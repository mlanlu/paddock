//! Finding the `paddock` binary from inside a bundled application.
//!
//! A `.app` launched from Finder inherits `launchd`'s environment, not a
//! shell's, so `PATH` is roughly `/usr/bin:/bin:/usr/sbin:/sbin` and `paddock`
//! — installed to `~/.local/bin` or Homebrew — is simply absent. Every command
//! the app runs therefore starts by resolving an absolute path itself.
//!
//! Two rules govern this module, and both are security properties rather than
//! conveniences:
//!
//! 1. **Never fall back to a bare name or a relative path.** Handing
//!    `Command::new("paddock")` to the OS resolves against the inherited `PATH`
//!    and, for a relative path, against the process working directory. paddock
//!    exists to keep an agent inside a sandbox; executing whatever `paddock`
//!    some other directory happens to contain gives away exactly that.
//! 2. **An override that does not resolve is an error, never a fallback.** If
//!    the user pointed the app at a specific binary and it is missing, silently
//!    running a different one is the worst outcome available — it looks like it
//!    worked.
//!
//! The login shell is the last rung because it is the expensive one: it runs
//! the user's own rc files to reconstruct their `PATH`. That is their
//! configuration, not untrusted input, but its *output* is still validated as
//! an absolute path to an executable file before it is used.

use std::fmt;
use std::path::{Path, PathBuf};

/// Which rung of the ladder produced a binary. Carried so the UI can tell the
/// user where the thing it is about to run came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An explicit path configured by the user.
    Override,
    /// `~/.local/bin` — where paddock's own README installs it.
    HomeLocalBin,
    /// `/opt/homebrew/bin` — Homebrew on Apple Silicon.
    Homebrew,
    /// `/usr/local/bin` — Homebrew on Intel, and the common manual install.
    UsrLocalBin,
    /// Reconstructed by running the user's login shell.
    LoginShell,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Override => "configured override",
            Source::HomeLocalBin => "~/.local/bin",
            Source::Homebrew => "/opt/homebrew/bin",
            Source::UsrLocalBin => "/usr/local/bin",
            Source::LoginShell => "login shell",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub path: PathBuf,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The user configured an override that is not an executable file. Never
    /// downgraded to a search — see rule 2 in the module docs.
    OverrideUnusable { path: PathBuf },
    /// The override was not absolute. Relative paths resolve against the
    /// process working directory, which is not something the user can see.
    OverrideNotAbsolute { path: PathBuf },
    /// Nothing on any rung. `looked` is every path actually probed, so the
    /// error message can tell the user where to install it or what to set.
    NotFound { binary: String, looked: Vec<PathBuf> },
    /// The login shell answered, but with something unusable. Kept distinct
    /// from `NotFound` because it means the user's shell config is the thing
    /// to look at.
    LoginShellUnusable { binary: String, got: String },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::OverrideUnusable { path } => write!(
                f,
                "the configured paddock path is not an executable file: {}",
                path.display()
            ),
            ResolveError::OverrideNotAbsolute { path } => write!(
                f,
                "the configured paddock path must be absolute, got: {}",
                path.display()
            ),
            ResolveError::NotFound { binary, looked } => {
                write!(f, "could not find `{binary}`. Looked in: ")?;
                for (i, p) in looked.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p.display())?;
                }
                write!(f, ", and the login shell")
            }
            ResolveError::LoginShellUnusable { binary, got } => write!(
                f,
                "the login shell located `{binary}` at {got:?}, which is not an absolute path to an executable file"
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

/// The effects `resolve` needs from the outside world. Two methods, both
/// injected so the ladder itself can be tested without a filesystem, a shell,
/// or macOS — none of which an agent developing paddock inside a paddock
/// sandbox has.
pub trait Probe {
    /// True when `path` is an existing regular file with an execute bit.
    fn is_executable_file(&self, path: &Path) -> bool;
    /// Ask the user's login shell where `binary` is, as they would see it.
    /// Returns the raw, untrusted answer; `resolve` validates it.
    fn login_shell_lookup(&self, binary: &str) -> Option<String>;
}

/// The fixed directories searched, in order, after any override.
///
/// Order is deliberate: `~/.local/bin` first because that is where paddock's
/// README installs it and a user-local copy should win over a system one, then
/// Homebrew's two prefixes. The process `PATH` is *not* consulted at any point.
fn search_dirs(home: Option<&Path>) -> Vec<(Source, PathBuf)> {
    let mut dirs = Vec::with_capacity(3);
    if let Some(home) = home {
        dirs.push((Source::HomeLocalBin, home.join(".local").join("bin")));
    }
    dirs.push((Source::Homebrew, PathBuf::from("/opt/homebrew/bin")));
    dirs.push((Source::UsrLocalBin, PathBuf::from("/usr/local/bin")));
    dirs
}

/// Resolve `binary` to an absolute path, or explain what was tried.
///
/// `override_path` is the user's configured location and short-circuits the
/// search entirely — successfully or not.
pub fn resolve(
    binary: &str,
    override_path: Option<&Path>,
    home: Option<&Path>,
    probe: &dyn Probe,
) -> Result<Resolved, ResolveError> {
    if let Some(path) = override_path {
        if !path.is_absolute() {
            return Err(ResolveError::OverrideNotAbsolute { path: path.to_path_buf() });
        }
        if probe.is_executable_file(path) {
            return Ok(Resolved { path: path.to_path_buf(), source: Source::Override });
        }
        return Err(ResolveError::OverrideUnusable { path: path.to_path_buf() });
    }

    let mut looked = Vec::new();
    for (source, dir) in search_dirs(home) {
        let candidate = dir.join(binary);
        if probe.is_executable_file(&candidate) {
            return Ok(Resolved { path: candidate, source });
        }
        looked.push(candidate);
    }

    match probe.login_shell_lookup(binary) {
        None => Err(ResolveError::NotFound { binary: binary.to_string(), looked }),
        Some(answer) => {
            // The shell prints a path; anything else — a relative path, an
            // alias, a "not found" message, several lines — is refused rather
            // than executed. See rule 1 in the module docs.
            let first = answer.lines().next().unwrap_or("").trim();
            let path = Path::new(first);
            if !first.is_empty() && path.is_absolute() && probe.is_executable_file(path) {
                Ok(Resolved { path: path.to_path_buf(), source: Source::LoginShell })
            } else {
                Err(ResolveError::LoginShellUnusable {
                    binary: binary.to_string(),
                    got: answer,
                })
            }
        }
    }
}

/// The real `Probe`: the filesystem, and the user's login shell.
pub struct SystemProbe;

impl SystemProbe {
    /// `$HOME` as the app sees it. Separate from `Probe` because it is a plain
    /// lookup with nothing to fake.
    pub fn home() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_absolute())
    }
}

impl Probe for SystemProbe {
    fn is_executable_file(&self, path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        // Follows symlinks deliberately: `~/.local/bin/paddock` is usually a
        // symlink into a checkout, and the target is what gets executed.
        match std::fs::metadata(path) {
            Ok(m) => m.is_file() && m.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }

    fn login_shell_lookup(&self, binary: &str) -> Option<String> {
        // `binary` reaches a shell command line. Today every caller passes a
        // literal, but a name that is not a bare word must never be
        // interpolated into a shell string, so it is refused outright rather
        // than quoted — there is no legitimate binary name this rejects.
        if binary.is_empty()
            || !binary.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }
        let shell = std::env::var("SHELL").ok()?;
        // `-l` runs the login files, which is where `PATH` is actually set;
        // `-i` because rc files routinely guard their `PATH` edits on being
        // interactive, and without it the answer is the same empty `PATH` the
        // app started with.
        let out = std::process::Command::new(shell)
            .args(["-lic", &format!("command -v {binary}")])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let answer = String::from_utf8_lossy(&out.stdout).to_string();
        if answer.trim().is_empty() {
            None
        } else {
            Some(answer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A probe over a fixed set of "executable" paths and a canned shell answer.
    struct FakeProbe {
        executables: Vec<PathBuf>,
        shell: Option<String>,
    }

    impl FakeProbe {
        fn new(executables: &[&str]) -> Self {
            Self {
                executables: executables.iter().map(PathBuf::from).collect(),
                shell: None,
            }
        }
        fn with_shell(mut self, answer: &str) -> Self {
            self.shell = Some(answer.to_string());
            self
        }
    }

    impl Probe for FakeProbe {
        fn is_executable_file(&self, path: &Path) -> bool {
            self.executables.iter().any(|p| p == path)
        }
        fn login_shell_lookup(&self, _binary: &str) -> Option<String> {
            self.shell.clone()
        }
    }

    fn home() -> PathBuf {
        PathBuf::from("/Users/someone")
    }

    #[test]
    fn override_wins_over_every_other_rung() {
        let probe = FakeProbe::new(&["/custom/paddock", "/opt/homebrew/bin/paddock"]);
        let got = resolve("paddock", Some(Path::new("/custom/paddock")), Some(&home()), &probe).unwrap();
        assert_eq!(got.path, PathBuf::from("/custom/paddock"));
        assert_eq!(got.source, Source::Override);
    }

    #[test]
    fn a_missing_override_is_an_error_not_a_fallback() {
        // The binary exists on a later rung; the override must still fail
        // loudly rather than quietly running a different binary.
        let probe = FakeProbe::new(&["/opt/homebrew/bin/paddock"]);
        let err = resolve("paddock", Some(Path::new("/custom/paddock")), Some(&home()), &probe).unwrap_err();
        assert_eq!(err, ResolveError::OverrideUnusable { path: PathBuf::from("/custom/paddock") });
    }

    #[test]
    fn a_relative_override_is_refused() {
        let probe = FakeProbe::new(&["paddock"]);
        let err = resolve("paddock", Some(Path::new("paddock")), Some(&home()), &probe).unwrap_err();
        assert_eq!(err, ResolveError::OverrideNotAbsolute { path: PathBuf::from("paddock") });
    }

    #[test]
    fn home_local_bin_is_searched_first() {
        let probe = FakeProbe::new(&[
            "/Users/someone/.local/bin/paddock",
            "/opt/homebrew/bin/paddock",
            "/usr/local/bin/paddock",
        ]);
        let got = resolve("paddock", None, Some(&home()), &probe).unwrap();
        assert_eq!(got.path, PathBuf::from("/Users/someone/.local/bin/paddock"));
        assert_eq!(got.source, Source::HomeLocalBin);
    }

    #[test]
    fn homebrew_beats_usr_local() {
        let probe = FakeProbe::new(&["/opt/homebrew/bin/paddock", "/usr/local/bin/paddock"]);
        let got = resolve("paddock", None, Some(&home()), &probe).unwrap();
        assert_eq!(got.source, Source::Homebrew);
    }

    #[test]
    fn usr_local_is_the_last_fixed_rung() {
        let probe = FakeProbe::new(&["/usr/local/bin/paddock"]);
        let got = resolve("paddock", None, Some(&home()), &probe).unwrap();
        assert_eq!(got.path, PathBuf::from("/usr/local/bin/paddock"));
        assert_eq!(got.source, Source::UsrLocalBin);
    }

    #[test]
    fn without_a_home_the_search_still_covers_the_system_dirs() {
        let probe = FakeProbe::new(&["/opt/homebrew/bin/paddock"]);
        let got = resolve("paddock", None, None, &probe).unwrap();
        assert_eq!(got.source, Source::Homebrew);
    }

    #[test]
    fn the_login_shell_is_the_last_resort() {
        let probe = FakeProbe::new(&["/weird/prefix/paddock"]).with_shell("/weird/prefix/paddock\n");
        let got = resolve("paddock", None, Some(&home()), &probe).unwrap();
        assert_eq!(got.path, PathBuf::from("/weird/prefix/paddock"));
        assert_eq!(got.source, Source::LoginShell);
    }

    #[test]
    fn a_relative_answer_from_the_login_shell_is_refused() {
        // `command -v` prints the bare name for a shell builtin or a function,
        // and prints an alias definition for an alias. None of those are a
        // binary this app may execute.
        let probe = FakeProbe::new(&["paddock"]).with_shell("paddock\n");
        let err = resolve("paddock", None, Some(&home()), &probe).unwrap_err();
        assert!(matches!(err, ResolveError::LoginShellUnusable { .. }));
    }

    #[test]
    fn a_login_shell_answer_that_is_not_executable_is_refused() {
        let probe = FakeProbe::new(&[]).with_shell("/weird/prefix/paddock\n");
        let err = resolve("paddock", None, Some(&home()), &probe).unwrap_err();
        assert!(matches!(err, ResolveError::LoginShellUnusable { .. }));
    }

    #[test]
    fn noise_before_the_path_is_refused_rather_than_trimmed() {
        // An rc file that prints a banner makes the first line not-a-path. The
        // shell's answer is a whole, and a partially-parsed one is not trusted.
        let probe = FakeProbe::new(&["/usr/bin/paddock"]).with_shell("welcome!\n/usr/bin/paddock\n");
        let err = resolve("paddock", None, Some(&home()), &probe).unwrap_err();
        assert!(matches!(err, ResolveError::LoginShellUnusable { .. }));
    }

    #[test]
    fn not_found_lists_every_path_it_probed() {
        let probe = FakeProbe::new(&[]);
        let err = resolve("paddock", None, Some(&home()), &probe).unwrap_err();
        match err {
            ResolveError::NotFound { binary, looked } => {
                assert_eq!(binary, "paddock");
                assert_eq!(
                    looked,
                    vec![
                        PathBuf::from("/Users/someone/.local/bin/paddock"),
                        PathBuf::from("/opt/homebrew/bin/paddock"),
                        PathBuf::from("/usr/local/bin/paddock"),
                    ]
                );
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn the_error_message_names_where_it_looked() {
        let probe = FakeProbe::new(&[]);
        let err = resolve("paddock", None, Some(&home()), &probe).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("/opt/homebrew/bin/paddock"), "{msg}");
        assert!(msg.contains("login shell"), "{msg}");
    }
}
