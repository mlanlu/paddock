//! Running the `paddock` CLI and streaming its output.
//!
//! **Why the environment is rebuilt rather than inherited.** A bundled app
//! inherits `launchd`'s `PATH`, and paddock itself shells out to `docker`. So
//! resolving an absolute path to `paddock` is only half the problem: the child
//! still has to find `docker`, and it looks for it on the `PATH` this app hands
//! it. An app that resolves `paddock` correctly and then hands it an empty
//! `PATH` fails with paddock's exit code 3 — "Docker is not running" — which
//! sends the user off to restart a daemon that was never the problem.
//!
//! **Why stdout and stderr are merged in arrival order.** paddock writes
//! progress to stderr and results to stdout. Collecting them separately and
//! concatenating makes the log read as though every step happened after every
//! result. The channel below preserves the order the app actually saw.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

use crate::exit::Exit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub stream: Stream,
    pub text: String,
}

/// Directories put on the child's `PATH`, after the resolved binary's own.
/// Both Homebrew prefixes plus the system defaults: enough for `docker` to be
/// found wherever Docker Desktop or Colima put it.
const CHILD_PATH_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

/// Build the `PATH` handed to `paddock`, starting with the directory the
/// resolved binary lives in.
pub fn child_path(binary: &Path, home: Option<&Path>) -> String {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(dir) = binary.parent() {
        dirs.push(dir.to_path_buf());
    }
    if let Some(home) = home {
        dirs.push(home.join(".local").join("bin"));
    }
    dirs.extend(CHILD_PATH_DIRS.iter().map(PathBuf::from));

    let mut seen = Vec::new();
    for d in dirs {
        if !seen.contains(&d) {
            seen.push(d);
        }
    }
    seen.iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(":")
}

fn base_command(binary: &Path, args: &[String], home: Option<&Path>) -> Command {
    let mut cmd = Command::new(binary);
    cmd.args(args);
    cmd.env("PATH", child_path(binary, home));
    if let Some(home) = home {
        // paddock reads ~/.config/paddock and ~/.cache/paddock. Without HOME it
        // would resolve those against the wrong user and report no profiles.
        cmd.env("HOME", home);
    }
    cmd.stdin(Stdio::null());
    cmd
}

/// Run to completion, capturing stdout. For the `--json` commands, where the
/// output is a single value and there is nothing to stream.
pub struct Captured {
    pub exit: Exit,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_capture(
    binary: &Path,
    args: &[String],
    home: Option<&Path>,
) -> std::io::Result<Captured> {
    let out = base_command(binary, args, home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    Ok(Captured {
        exit: Exit::from_code(out.status.code().unwrap_or(-1)),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Run, calling `on_line` for each line of output as it arrives, then return
/// the exit status. Used for `up`, `stop`, `rm` and `reset`, where an image
/// build can take minutes and output that appears only at the end is useless.
pub fn run_streaming(
    binary: &Path,
    args: &[String],
    home: Option<&Path>,
    mut on_line: impl FnMut(Line),
) -> std::io::Result<Exit> {
    let mut child = base_command(binary, args, home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    let (tx, rx) = mpsc::channel::<Line>();
    let tx_err = tx.clone();

    let h_out = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(Line { stream: Stream::Stdout, text: line }).is_err() {
                break;
            }
        }
    });
    let h_err = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if tx_err.send(Line { stream: Stream::Stderr, text: line }).is_err() {
                break;
            }
        }
    });

    // Both senders are owned by the threads, so the channel closes on its own
    // once each has finished; this drains in arrival order until it does.
    for line in rx {
        on_line(line);
    }
    let _ = h_out.join();
    let _ = h_err.join();

    let status = child.wait()?;
    Ok(Exit::from_code(status.code().unwrap_or(-1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_binarys_own_directory_leads_the_path() {
        let path = child_path(Path::new("/opt/homebrew/bin/paddock"), None);
        assert!(path.starts_with("/opt/homebrew/bin:"), "{path}");
    }

    #[test]
    fn the_child_path_can_find_docker() {
        // The failure this guards against: paddock exits 3 "Docker is not
        // running" because the app handed it a PATH with no Homebrew in it.
        let path = child_path(Path::new("/Users/x/.local/bin/paddock"), None);
        assert!(path.contains("/opt/homebrew/bin"), "{path}");
        assert!(path.contains("/usr/local/bin"), "{path}");
    }

    #[test]
    fn the_path_has_no_duplicates() {
        let home = PathBuf::from("/Users/x");
        let path = child_path(Path::new("/usr/local/bin/paddock"), Some(&home));
        let dirs: Vec<&str> = path.split(':').collect();
        let mut unique = dirs.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(dirs.len(), unique.len(), "{path}");
    }

    #[test]
    fn home_local_bin_is_on_the_child_path() {
        let home = PathBuf::from("/Users/x");
        let path = child_path(Path::new("/usr/local/bin/paddock"), Some(&home));
        assert!(path.contains("/Users/x/.local/bin"), "{path}");
    }

    #[test]
    fn the_path_is_never_empty_even_with_a_bare_binary_name() {
        let path = child_path(Path::new("paddock"), None);
        assert!(path.contains("/usr/bin"), "{path}");
    }
}
