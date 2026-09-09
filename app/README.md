# The paddock desktop app

A desktop front end for the `paddock` CLI: pick a folder, watch its sandbox come
up, and — from M2 — get a terminal on `claude` inside it. Tauri, so a Rust core
and the system WebView rather than a bundled browser.

The CLI stays the engine. This app shells out to `paddock` and never talks to
Docker directly. paddock's value is its security model, and a second
implementation in Rust would be a second thing that can disagree with the first.

## Shape

```
app/
  src/            TypeScript frontend. No framework — the app is a list, a log
                  panel and six buttons, and M2's xterm.js is framework-agnostic.
  src-tauri/
    core/         Plain Rust. No `tauri` dependency. All the logic, unit-tested.
    src/          Tauri wrappers: resolve arguments, call core, map errors.
```

**The split between `core/` and `src/` is the important thing here.** Agents
develop paddock from inside a paddock sandbox, which has no macOS and no WebKit,
so a crate that depends on `tauri` cannot be compiled there at all. `core/` can:

```sh
cd src-tauri && cargo test -p paddock-core
```

That runs without Docker, without macOS and without the GTK/WebKit stack. It is
the only verification available from inside a sandbox, so anything worth
checking belongs in `core/` and the Tauri layer stays thin enough to read.

## Building

Needs macOS with Xcode command line tools, Node 20+, and a Rust toolchain.

```sh
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

cd app
npm install
npm run tauri icon src-tauri/icons/icon.png   # once: platform-native formats
npm run tauri dev
```

`npm run tauri build` produces the bundle. It is **unsigned** — there is no
Apple Developer account (PLAN.md, D6) — so macOS quarantines it on first open.
Right-click → Open, or:

```sh
xattr -dr com.apple.quarantine /Applications/paddock.app
```

That is expected, not a bug. Notarization is a documented gap, not a missing
feature.

## Why binary resolution has its own module

A `.app` launched from Finder inherits `launchd`'s environment, not a shell's.
`PATH` is roughly `/usr/bin:/bin:/usr/sbin:/sbin`, so `paddock` — installed to
`~/.local/bin` or Homebrew — is not on it. This is the failure the whole
milestone exists to get right, and it has two halves:

1. The app resolves an absolute path to `paddock` itself, never relying on the
   inherited `PATH` and never falling back to a bare name. See
   `core/src/resolve.rs`, which documents why an override that does not resolve
   is an error rather than a fallback.
2. The app hands the child a `PATH` that can find `docker`, because paddock
   shells out to it. Getting (1) right and (2) wrong produces paddock's exit
   code 3 — "Docker is not running" — and sends the user off to restart a daemon
   that was never the problem. See `core/src/proc.rs`.
