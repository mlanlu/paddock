import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  cliInfo,
  info,
  listSandboxes,
  runStreamed,
  type Info,
  type Lifecycle,
  type Policy,
  type Sandbox,
} from "./api";

const $ = <T extends HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`missing element: ${sel}`);
  return el;
};

const list = $("#sandboxes");
const logPanel = $("#log-panel");
const logHead = $("#log-head");
const logEl = $("#log");
const pickButton = $<HTMLButtonElement>("#pick");
const cliLabel = $("#cli");

/** Nothing from the CLI is ever interpolated as HTML: container names come from
 *  directory names, and a workspace path is arbitrary user input. */
function el(tag: string, className?: string, text?: string): HTMLElement {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

let busy = false;

function setBusy(next: boolean): void {
  busy = next;
  for (const b of document.querySelectorAll<HTMLButtonElement>("button")) {
    b.disabled = next;
  }
}

// ---------------------------------------------------------------- log panel

function log(text: string, kind: "out" | "err" | "app" = "out"): void {
  logPanel.hidden = false;
  const line = el("span", kind === "out" ? "" : kind, text + "\n");
  logEl.append(line);
  // Only follow the tail when the user has not scrolled away from it.
  const atBottom = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < 40;
  if (atBottom) logEl.scrollTop = logEl.scrollHeight;
}

function startLog(title: string): void {
  logEl.textContent = "";
  logHead.textContent = title;
  logPanel.hidden = false;
}

// ------------------------------------------------------------------- policy

/** The policy cell. `open` means no firewall at all, which is the one state
 *  that must never read as just another label. */
function policyCell(policy: Policy | null, closed: boolean): HTMLElement {
  const wrap = el("span");
  if (closed) {
    wrap.append(el("span", "bad", "CLOSED"));
    wrap.append(
      el("span", "muted", policy ? ` (wanted ${policy.describe})` : " (policy not applied)"),
    );
    return wrap;
  }
  if (!policy) return el("span", "muted", "—");
  if (policy.sets.includes("open")) {
    wrap.append(el("span", "bad", "open — no firewall"));
    return wrap;
  }
  wrap.append(el("span", "", policy.describe));
  return wrap;
}

// -------------------------------------------------------------------- table

async function refresh(): Promise<void> {
  list.textContent = "";
  let sandboxes: Sandbox[];
  try {
    sandboxes = await listSandboxes();
  } catch (e) {
    list.append(el("p", "bad", String(e)));
    return;
  }

  if (sandboxes.length === 0) {
    list.append(el("p", "muted", "No sandboxes yet. Open a folder to make one."));
    return;
  }

  for (const s of sandboxes) {
    const row = el("article", "row");

    const head = el("div", "row-head");
    head.append(el("strong", "", s.container));
    head.append(el("span", s.state === "running" ? "ok" : "muted", s.state));
    if (s.closed) head.append(el("span", "bad", "policy not fully applied"));
    row.append(head);

    const meta = el("div", "row-meta");
    meta.append(el("span", "muted", s.workspace));
    meta.append(el("span", "muted", `profile ${s.profile}`));
    meta.append(policyCell(s.policy, s.closed));
    if (s.ports.length > 0) {
      meta.append(
        el("span", "muted", s.ports.map((p) => `${p.host}→${p.container}`).join(" ")),
      );
    }
    row.append(meta);

    const actions = el("div", "row-actions");
    if (s.state === "running") {
      actions.append(action("Stop", () => lifecycle("stop", s.workspace, s.container)));
    } else {
      actions.append(action("Start", () => lifecycle("up", s.workspace, s.container)));
    }
    actions.append(action("Remove", () => lifecycle("rm", s.workspace, s.container)));
    actions.append(action("Reset…", () => confirmReset(s)));
    row.append(actions);

    list.append(row);
  }
}

function action(label: string, onClick: () => void): HTMLButtonElement {
  const b = document.createElement("button");
  b.type = "button";
  b.textContent = label;
  b.disabled = busy;
  b.addEventListener("click", onClick);
  return b;
}

// --------------------------------------------------------------- lifecycle

async function lifecycle(command: Lifecycle, path: string, container: string): Promise<void> {
  if (busy) return;
  setBusy(true);
  startLog(`paddock ${command} — ${container}`);
  try {
    const result = await runStreamed(command, path);
    if (result.ok) {
      log(`\n✓ ${command} succeeded`, "app");
    } else {
      log(`\n✗ ${command} ${result.meaning} (exit ${result.code})`, "err");
      if (result.misstates_policy) {
        // Exit 5 and 6 both leave a sandbox running in a state the user did not
        // ask for. Saying only "it failed" would hide the thing paddock exists
        // to be honest about.
        log("The sandbox is running, but not with the policy its profile asks for.", "err");
      }
      if (result.remedy) log(result.remedy, "app");
    }
  } catch (e) {
    log(String(e), "err");
  } finally {
    setBusy(false);
    await refresh();
  }
}

async function confirmReset(s: Sandbox): Promise<void> {
  // `reset` destroys the volumes: node_modules and, more painfully, the Claude
  // login. `rm` does not, and does not ask.
  const ok = window.confirm(
    `Reset ${s.container}?\n\n` +
      `This removes the container AND its volumes — node_modules and the Claude ` +
      `login inside the sandbox. You will have to log in again.\n\n` +
      `To keep them, use Remove instead.`,
  );
  if (ok) await lifecycle("reset", s.workspace, s.container);
}

// ------------------------------------------------------------------ picker

async function pick(): Promise<void> {
  const chosen = await open({ directory: true, multiple: false, title: "Choose a folder" });
  if (typeof chosen !== "string") return;

  let resolved: Info;
  try {
    resolved = await info(chosen);
  } catch (e) {
    startLog(chosen);
    log(String(e), "err");
    return;
  }
  showPlan(resolved);
}

/** What `up` *would* do, before anything runs. `info` has no side effects, so
 *  this is free to show and is the whole reason the CLI has that command. */
function showPlan(i: Info): void {
  list.textContent = "";
  const card = el("article", "row plan");

  const head = el("div", "row-head");
  head.append(el("strong", "", i.container));
  head.append(el("span", "muted", i.state ?? "not created yet"));
  card.append(head);

  const meta = el("div", "row-meta");
  meta.append(el("span", "muted", i.root));
  meta.append(el("span", "muted", `profile ${i.profile}`));
  meta.append(policyCell(i.policy, false));
  card.append(meta);

  if (i.profile_path === null) {
    // Documented cue: only profiles/default.json applies.
    card.append(
      el(
        "p",
        "muted",
        "No profile of its own — the default profile applies. `paddock init` writes one.",
      ),
    );
  }
  if (i.common_git !== null) {
    card.append(el("p", "muted", `Linked worktree of ${i.common_git}`));
  }

  const actions = el("div", "row-actions");
  actions.append(action(i.state === "running" ? "Restart" : "Start sandbox", () =>
    lifecycle("up", i.root, i.container),
  ));
  actions.append(action("Back to list", () => void refresh()));
  card.append(actions);

  list.append(card);
}

// -------------------------------------------------------------------- boot

async function main(): Promise<void> {
  await listen<{ stream: string; text: string }>("paddock://log", (e) => {
    log(e.payload.text, e.payload.stream === "stderr" ? "err" : "out");
  });

  await listen<string>("paddock://fatal", (e) => {
    // Without the CLI there is no app, so this replaces the list rather than
    // appearing next to it.
    list.textContent = "";
    list.append(el("p", "bad", e.payload));
    pickButton.disabled = true;
  });

  pickButton.addEventListener("click", () => void pick());

  try {
    const cli = await cliInfo();
    cliLabel.textContent = `${cli.path} (${cli.source})`;
  } catch {
    cliLabel.textContent = "";
  }

  await refresh();
}

void main();
