# paddock — plan

Shared state for the desktop app work. Sessions and agents read this first and
update it as they go. Process rules live in `AGENTS.md`; this file holds the
work, its status, and the decisions.

**Status:** `TODO` · `WIP` · `REVIEW` · `DONE` · `BLOCKED`

**Review class:** `opus` — the agents allowed to review work packages and to
review decisions about findings. Keep this list here; do not hardcode it
elsewhere. `sol` is intended to join the class; it has no known invocation yet,
so reviews run on `opus` alone until it does.

---

## Goal

A desktop app that starts a sandboxed agent session in one click: pick a folder,
watch it come up, get a terminal on `claude` inside the sandbox — without giving
up anything the CLI guarantees.

**Tauri** (Rust core, system WebView), macOS Apple Silicon and Linux, terminals
embedded in the app with popping out to a real terminal as an option.

The CLI stays the engine. The app shells out to `paddock`; it never talks to
Docker directly. paddock's value is its security model — additive policy
resolution, root-owned `/etc/paddock`, a fail-closed firewall — and a second
implementation in Rust would be a second thing that can disagree with the first.

---

## M0 — CLI seams · `REVIEW`

Machine-readable output so the app has something to consume. Ships useful on its
own: `ls --json` is worth having with or without an app.

### WP-M0-1 — `paddock ls --json` · Show · `REVIEW`

The app must not parse the table. Split `cmd_ls` into one function that builds
the rows and one that renders them, then render either the table or JSON.

Shape, per sandbox: `container`, `state`, `repo`, `profile`, `workspace`,
`closed`, `policy` (`sets`, `extra`, `describe`), `ports` (`[{host, container}]`).
`closed` is a fact about the sandbox, not the policy, so it sits at the top
level where it survives `policy` being `null` (moved there in review).
Ports as a list of pairs, not the `"3010->3000"` display string.

- Touches: `paddock` (`cmd_ls`), `docs/cli-json.md` (new), `README.md` command list.
- Done when: `paddock ls` output is byte-identical to today's, `paddock ls --json`
  parses and matches the documented shape, and the shape is documented.
- Verify: run both with two sandboxes up, one stopped, one `CLOSED`; confirm the
  table is unchanged and the JSON round-trips through `python3 -m json.tool`.
  **Needs Docker.**
- Done 2026-09-09 (c48e2f6). Observed: table byte-identical against the
  pre-change binary with three sandboxes (one running, two stopped); JSON parses;
  a CLOSED state (faked in the state file of a scratch sandbox) renders in both.

### WP-M0-2 — `paddock info PATH --json` · Show · `REVIEW`

Resolve a path with no side effects, so the app can show what *will* happen
before the user commits to it, and offer `paddock init` when there is no profile.

Shape: `root`, `repo`, `name`, `container`, `common_git`, `profile`,
`profile_path` (null when none exists), `env_file`, `state` (null when no
container), `ports`, `policy`.

Note: `provisioned()` runs `docker exec`, so it only answers for a running
sandbox. Report `null` otherwise rather than guessing or starting anything.

- Touches: `paddock` (new `cmd_info`), `docs/cli-json.md`, `README.md`.
- Done when: it works on a plain directory, a git repo, and a linked worktree,
  in all three cases with and without a container, and never mutates anything.
- Verify: run in a repo with no profile (expect `profile_path: null`), in a
  worktree (expect `common_git` set and the repo's profile), and against a
  running sandbox. Confirm `docker ps -a` is unchanged after each.
- Done 2026-09-09 (a9709a1). Observed on a plain directory with no profile
  (`profile_path: null`, `state: null`, ports previewed via `assign_ports`), on
  the openmatch main repo without a container, on the `openmatch-web` linked
  worktree with a stopped container (`common_git` set, repo's profile and
  ports), and on a running scratch sandbox (`provisioned: true`). `docker ps -a`
  identical before and after. Adds a `provisioned` field the PLAN shape lacked —
  the app needs it to show "will provision on start"; it is the one thing only a
  running sandbox can answer, hence nullable.

### WP-M0-3 — distinct exit codes · Ask · `REVIEW`

`die()` exits `1` for everything, so a caller cannot tell "Docker is not
running" from "provisioning failed" from "the firewall is CLOSED" — and each of
those wants a different button in the UI.

Shipped: `3` docker unavailable · `4` configuration missing (no profile, no
policy set) · `5` provisioning failed · `6` firewall closed · `7` sandbox not
running. `1` stays the catch-all, `2` stays argparse's usage error — see D5.

**Ask** because it touches `install_policy`, which is on the security list in the
skill. Intended to only make existing failures more legible; review found one
failure that had to change shape as well — see D6.

- Touches: `paddock` (`die` call sites), `docs/cli-json.md`, `README.md`.
- Done when: each code is reachable and documented, and no failure path that
  previously exited non-zero now exits zero.
- Verify: docker off `PATH` → expect `3`. Unknown `--policies` value →
  expect `4`. Confirm success paths still exit `0`.
- Done 2026-09-09. Observed: `PATH` without docker → `3` for `ls` and `stop`
  (`ls` previously died with a `FileNotFoundError` traceback, since it never
  called `docker_ok()`; the check now runs in `main` for every command but
  `init`). Unknown `--policies` → `4`, unknown `-p` → `4`, `firewall` on a
  stopped sandbox → `7`, unknown subcommand and `exec` without a command → `2`,
  a profile with `"install": "false"` → `5` from `paddock provision`.
  `ls`/`up`/`info` success → `0`. `6` observed by blocking the host's GitHub
  fetch: firewall applied, example.com and github.com unreachable, npm
  reachable, state `closed: true`. The firewall-script-failure route to `6`
  was not observed (needs a broken network inside the VM).
- Residual (WP-M0-3 decision review): `install_policy` can still die with `1`
  before the firewall runs if a `write_root_file` docker exec fails on a
  container `up` has just started. `ensure_up` dies before handing out a
  shell, so it needs an out-of-band attach. Stop the fresh container in that
  case if it ever bites.

---

## Fixes found along the way

### WP-FIX-1 — provisioning marker outlived the container · Show · `DONE`

`provisioned()` checked `/commandhistory/.paddock-provisioned`, but
`/commandhistory` is a volume that `paddock rm` deliberately keeps, while most
of what `provision()` does — `safe.directory`, `gc.worktreePruneExpire`,
corepack activation — lives in the container filesystem. (The `settings.json`
copy goes to the `claude` volume and survives `rm`, like the old marker did.)
So `paddock rm` followed by `paddock up` produced a container that
reported itself provisioned and was not.

Found in this repo's own sandbox: the marker was present, `~/.gitconfig` was
absent, and `git add` failed intermittently with `detected dubious ownership`
because without `safe.directory` git ownership-checks a virtiofs bind mount
that reports uid inconsistently.

Marker moved to `/etc/paddock/provisioned` — container filesystem, root-owned,
so the agent can neither forge nor delete it, and it dies with the container it
describes. Existing sandboxes re-provision once on their next `up`, which
repairs exactly this state.

- Touches: `paddock` (`PROVISION_MARKER`, `provisioned`, `provision`), `README.md`.
- Done when: `rm` + `up` re-provisions; stop/start does not; `paddock provision`
  still works on a running sandbox.
- Verify: **needs Docker.** `paddock up` → `paddock rm` → `paddock up`, then
  confirm `~/.gitconfig` exists inside and `git config --global --get-all
  safe.directory` returns `*`. Then `paddock stop` → `paddock up` and confirm
  provisioning is *skipped*.
- Done 2026-09-09 (fd86e8a, ed17f8e). Observed on a scratch workspace: `rm` +
  `up` re-provisioned (`~/.gitconfig` present, `safe.directory = *`), `stop` +
  `up` skipped provisioning, the `node` user cannot remove or create files in
  `/etc/paddock`, `paddock provision` still works.
- Review: `docs/reviews/WP-FIX-1.md` — two comment findings, both acted on;
  decision review agrees.

---

## M1 — app skeleton, no terminal · `TODO`

Tauri scaffold in `app/`. Native folder picker, sandbox list from `ls --json`,
`paddock up` with its output streamed into a log panel, stop/rm/reset.

Deliberately terminal-free. This milestone exists to prove the spawn → stream →
render chain works *from inside a bundled `.app`*, which is where the usual
failure lives: a GUI app on macOS does not inherit the shell's `PATH`, so
`paddock` and `docker` are not found. Resolve binaries explicitly — config
override, then `~/.local/bin`, then `/opt/homebrew/bin`, then a login shell.

Break into work packages when M0 closes.

## M2 — embedded terminal · `TODO`

`portable-pty` + xterm.js with the webgl addon; Claude streams fast enough that
the DOM renderer stutters. Tabs, resize, and "open in Terminal" as the escape
hatch.

Carries an open design decision — see *tmux durability* below — that must be
settled before the transport is written, because it decides whether the terminal
owns a process or attaches to a session.

## M3 — polish and ship · `TODO`

Policy sets as checkboxes wired to `paddock firewall` (live retightening is one
of paddock's better tricks and is invisible in the CLI). Recent workspaces.
Ports as clickable links. `.dmg` plus notarization, Linux AppImage/`.deb`, CI
release.

---

## Open questions

1. **Adding `sol` to the review class.** `opus` maps to the Agent tool's
   `model: opus`; `sol` maps to nothing this session can see. Reviews run on
   `opus` alone until someone says what `sol` is. Not blocking.
2. **tmux durability.** `paddock run` is a `docker exec`: quit the app and the
   agent dies mid-task. Running `claude` under `tmux` inside the container makes
   sessions survive, lets the app re-attach with scrollback, and makes "open in
   Terminal" attach *the same* session instead of a rival one. Costs one word in
   the image's apt list and a small `tmux.conf` so its keybindings stay clear of
   Claude's. **Decide before M2 starts.**
3. **Apple Developer account** ($99/yr) for notarization. Without it every
   install trips Gatekeeper. Decide before M3, not during.
4. **Rust toolchain for agent sessions.** Agents run inside a paddock sandbox
   with no `cargo`, so Rust would be written blind from M1 on. A `rust` policy
   set (`static.rust-lang.org`, `crates.io`, `index.crates.io`) plus Rust in the
   image would let an agent at least `cargo check`. Worth adding to the repo
   regardless.
5. **Test infrastructure.** No tests exist, and most paths need a running Docker
   daemon. Revisit when the JSON contracts have a real consumer; until then it
   is scaffolding for one caller.
6. **Container name collisions.** Containers are addressed by name only, and
   `ensure_up` reuses any container with that name without checking its
   `paddock.workspace` label. Two clones with the same repo and directory
   basename under different parents collide; `info` then reports the other
   workspace's state. Pre-existing; surfaced in the WP-M0-2 decision review.
   Fix when someone hits it — probably `die` on a label mismatch.
7. **Splitting `paddock-dev`.** `AGENTS.md` is both the always-on guidance and,
   via symlink, the `paddock-dev` skill — so invoking the skill re-loads text
   Claude already has. Harmless at 226 lines, and with one skill in existence
   any split would be guesswork. **Revisit when the product skill lands:** if
   `AGENTS.md` has grown, the review protocol is the natural first thing to move
   into `paddock-dev` as a genuinely on-demand skill, leaving the principles
   behind. If it has not, leave it alone.

---

## Decisions

**D1 — Tauri over Electron, SwiftUI, or a local web UI.** A signed `.dmg` makes
the runtime an implementation detail, which removes the argument for keeping the
UI in Python. Electron bundles its own Chromium and is heavy; SwiftUI is lighter
still but macOS-only. Tauri gives a real app in the dock, the system WebView, and
Linux for close to free.

**D2 — the app shells out to the CLI.** Reimplementing policy resolution in Rust
would create two implementations of the security model that can drift. Process
spawn cost is irrelevant at this interaction rate.

**D3 — Unix only: macOS Apple Silicon and Linux.** Windows is not merely
untested: `create_container` bind-mounts the workspace at its literal path
(`-v /Users/you/repo:/Users/you/repo`), which has no direct `C:\` equivalent.
Supporting it is CLI work, independent of the UI, and nobody needs it.

**D5 — exit codes start at 3.** The plan proposed `2` for "Docker unavailable",
but argparse exits `2` on any usage error and that is not worth overriding. A
caller that gets `2` has a bug in its own invocation; everything paddock itself
diagnoses starts at `3`. `exec` without a command also uses `2`, since it is a
usage error argparse cannot see.

**D6 — a missing GitHub range fetch closes the sandbox rather than aborting.**
Before WP-M0-3, `github_ranges()` died mid-way through writing `/etc/paddock`
and before the firewall script ran. On a freshly created container that left
a running sandbox with no iptables rules at all, reported as exit `1`. Now the
ranges are fetched first; on failure the firewall runs with an empty ranges
file (the script fails closed; empty means fewer ipset entries), the sandbox is
marked `closed` and paddock exits `6`. Not fetched at all in `open` mode, so
an open sandbox is never labelled CLOSED. Made autonomously in an agent session
after two review rounds; revert is one line if the human disagrees.

**D4 — stream text for progress, not a structured event protocol.** `info()`
already emits readable `paddock: …` lines. A porcelain event stream would be an
API invented ahead of its first real requirement. Add one only if the progress
panel actually feels bad.
