// Typed mirror of the Rust commands in `src-tauri/src/lib.rs`, which in turn
// mirror `docs/cli-json.md`. Kept in one file so a contract change has one
// place to land on this side.
import { invoke } from "@tauri-apps/api/core";

export interface Port {
  host: number;
  container: number;
}

export interface Policy {
  sets: string[];
  extra: string[];
  describe: string;
}

export interface Sandbox {
  container: string;
  state: string;
  repo: string;
  profile: string;
  workspace: string;
  closed: boolean;
  policy: Policy | null;
  ports: Port[];
}

export interface Info {
  root: string;
  repo: string;
  name: string;
  container: string;
  common_git: string | null;
  profile: string;
  profile_path: string | null;
  env_file: string | null;
  state: string | null;
  provisioned: boolean | null;
  ports: Port[];
  policy: Policy | null;
}

export interface RunResult {
  code: number;
  ok: boolean;
  meaning: string;
  remedy: string | null;
  misstates_policy: boolean;
}

export interface CliInfo {
  path: string;
  source: string;
}

/** The lifecycle verbs the backend will accept. Mirrors its allow-list. */
export type Lifecycle = "up" | "stop" | "rm" | "reset";

export const cliInfo = () => invoke<CliInfo>("cli_info");
export const listSandboxes = () => invoke<Sandbox[]>("list_sandboxes");
export const info = (path: string) => invoke<Info>("info", { path });
export const runStreamed = (command: Lifecycle, path: string) =>
  invoke<RunResult>("run_streamed", { command, path });
