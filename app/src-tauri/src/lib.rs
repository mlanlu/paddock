//! Tauri wrappers over `paddock_core`.
//!
//! Everything here should be a thin translation between the frontend and the
//! core: resolve arguments, call the core, map errors to strings the UI can
//! show. Logic that is worth testing belongs in `core/`, which compiles without
//! WebKit and therefore inside a paddock sandbox.

use std::path::PathBuf;

use paddock_core::{contract, exit::Exit, proc, resolve};
use serde::Serialize;
use tauri::{Emitter, Manager};

/// The resolved CLI, worked out once at startup. Resolution can run the user's
/// login shell, which is far too slow to repeat per click.
struct Cli {
    binary: PathBuf,
    home: Option<PathBuf>,
    source: resolve::Source,
}

#[derive(Serialize)]
struct CliInfo {
    path: String,
    source: String,
}

/// A line of streamed output, as the frontend receives it.
#[derive(Clone, Serialize)]
struct LogLine {
    stream: &'static str,
    text: String,
}

/// The result of a streamed command.
#[derive(Serialize)]
struct RunResult {
    code: i32,
    ok: bool,
    meaning: String,
    remedy: Option<String>,
    misstates_policy: bool,
}

impl From<Exit> for RunResult {
    fn from(e: Exit) -> Self {
        let code = match e {
            Exit::Ok => 0,
            Exit::Failure => 1,
            Exit::Usage => 2,
            Exit::NoDocker => 3,
            Exit::Config => 4,
            Exit::Provisioning => 5,
            Exit::Closed => 6,
            Exit::NoSandbox => 7,
            Exit::Other(c) => c,
        };
        RunResult {
            code,
            ok: e.is_ok(),
            meaning: e.meaning().to_string(),
            remedy: e.remedy().map(str::to_string),
            misstates_policy: e.misstates_policy(),
        }
    }
}

#[tauri::command]
fn cli_info(cli: tauri::State<'_, Cli>) -> CliInfo {
    CliInfo {
        path: cli.binary.to_string_lossy().into_owned(),
        source: cli.source.as_str().to_string(),
    }
}

#[tauri::command]
fn list_sandboxes(cli: tauri::State<'_, Cli>) -> Result<Vec<contract::Sandbox>, String> {
    let args = vec!["ls".to_string(), "--json".to_string()];
    let out = proc::run_capture(&cli.binary, &args, cli.home.as_deref())
        .map_err(|e| format!("could not run paddock: {e}"))?;
    if !out.exit.is_ok() {
        return Err(describe_failure(out.exit, &out.stderr));
    }
    contract::parse_sandboxes(&out.stdout).map_err(|e| e.to_string())
}

#[tauri::command]
fn info(cli: tauri::State<'_, Cli>, path: String) -> Result<contract::Info, String> {
    let args = vec!["info".to_string(), path, "--json".to_string()];
    let out = proc::run_capture(&cli.binary, &args, cli.home.as_deref())
        .map_err(|e| format!("could not run paddock: {e}"))?;
    if !out.exit.is_ok() {
        return Err(describe_failure(out.exit, &out.stderr));
    }
    contract::parse_info(&out.stdout).map_err(|e| e.to_string())
}

/// Run a lifecycle command, streaming each line to the frontend as it arrives.
#[tauri::command]
async fn run_streamed(
    app: tauri::AppHandle,
    command: String,
    path: String,
) -> Result<RunResult, String> {
    // Only the lifecycle verbs. The frontend cannot ask for an arbitrary
    // subcommand, so a compromised WebView cannot turn this into a way to run
    // `exec -- <anything>` on the host.
    let allowed = ["up", "stop", "rm", "reset"];
    if !allowed.contains(&command.as_str()) {
        return Err(format!("refusing to run `paddock {command}`"));
    }

    let cli = app.state::<Cli>();
    let binary = cli.binary.clone();
    let home = cli.home.clone();
    let args = vec![command, path];

    let handle = app.clone();
    let exit = tauri::async_runtime::spawn_blocking(move || {
        proc::run_streaming(&binary, &args, home.as_deref(), |line| {
            let payload = LogLine {
                stream: match line.stream {
                    proc::Stream::Stdout => "stdout",
                    proc::Stream::Stderr => "stderr",
                },
                text: line.text,
            };
            let _ = handle.emit("paddock://log", payload);
        })
    })
    .await
    .map_err(|e| format!("the command could not be run: {e}"))?
    .map_err(|e| format!("could not run paddock: {e}"))?;

    Ok(RunResult::from(exit))
}

/// Turn a failed run into one sentence the user can act on: what paddock said,
/// then what to do about it.
fn describe_failure(exit: Exit, stderr: &str) -> String {
    let detail = stderr.trim().lines().last().unwrap_or("").trim();
    let mut msg = exit.meaning().to_string();
    if !detail.is_empty() {
        msg = format!("{msg}: {detail}");
    }
    if let Some(remedy) = exit.remedy() {
        msg = format!("{msg} — {remedy}");
    }
    msg
}

pub fn run() {
    let probe = resolve::SystemProbe;
    let home = resolve::SystemProbe::home();
    // No override plumbed yet: there is no settings surface to set one, and a
    // knob no caller can reach is the kind of thing this repo deletes.
    let resolved = resolve::resolve("paddock", None, home.as_deref(), &probe);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            match &resolved {
                Ok(r) => {
                    app.manage(Cli {
                        binary: r.path.clone(),
                        home: home.clone(),
                        source: r.source,
                    });
                }
                Err(e) => {
                    // Without the CLI there is no app. Surfacing this as a
                    // startup error beats every command failing separately
                    // with the same cause.
                    let _ = app.emit("paddock://fatal", e.to_string());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cli_info,
            list_sandboxes,
            info,
            run_streamed
        ])
        .run(tauri::generate_context!())
        .expect("error while running the paddock app");
}
