# paddock

An enclosure for coding agents. Run `claude --dangerously-skip-permissions` inside a Docker sandbox that only sees one workspace directory, has default-deny egress, carries no host credentials, and has no Docker socket. Policy lives outside the repo, so the agent cannot loosen its own sandbox.

One sandbox per workspace directory. Want several agents in parallel? Give each its own directory — git worktrees are one convenient way to do that, but a plain clone works exactly the same, and nothing forces you into worktrees.

```
paddock run ~/code/myrepo            # build (first time), start, drop into claude --dangerously-skip-permissions
paddock run ~/code/myrepo-feature    # a second sandbox, e.g. for a worktree, with its own ports
paddock ls
```

## What the agent gets

| | |
|---|---|
| **Filesystem** | The workspace directory, bind-mounted at its real path. For a linked git worktree, also the repo's shared `.git` (found via `git rev-parse --git-common-dir`, mounted at its real path so the worktree pointer resolves). Nothing else from the host. |
| **Network** | Default-deny egress (iptables/ipset). Allowed: GitHub's published IP ranges, the base list in [`image/base-domains.txt`](image/base-domains.txt) (Anthropic API, npm), plus your profile's `domains`. Baked into the image root-owned — the agent can't widen it; you rebuild. |
| **Host** | `host.docker.internal` only on the TCP ranges in the profile's `host_ports` (e.g. a local Supabase / Postgres). No Docker socket. |
| **Credentials** | Only what's in `~/.config/paddock/env/<profile>.env` (a fine-grained GitHub PAT, optional Claude token, project keys). `~/.claude`, `~/.config/gh`, `~/.ssh` are never mounted. Claude's login persists on a per-sandbox volume. |
| **Privileges** | Runs as `node`. `sudo` works for exactly two root-owned scripts: re-applying the firewall and fixing volume ownership. |
| **Tooling** | Node (version from the profile), corepack, `gh`, `claude`, git + delta, ripgrep, zsh. |

## Install

Requires Docker (daemon running) and Python 3.8+. Nothing else on the host — no Node, no devcontainer CLI.

```bash
git clone https://github.com/mlanlu/paddock ~/.local/share/paddock
ln -s ~/.local/share/paddock/paddock ~/.local/bin/paddock   # or anywhere on PATH
```

## Setup for a repo

```bash
cd ~/code/myrepo
paddock init          # writes ~/.config/paddock/profiles/<repo>.json + env/<repo>.env, prints what it detected
```

Fill in the env file (at least `GH_TOKEN`, a fine-grained PAT scoped to that repo) and adjust the profile. Then `paddock run`.

The profile name defaults to the repo's directory name (for worktrees: the directory that owns the real `.git`, so all worktrees share one profile). Override with `--profile`.

### Profile

`~/.config/paddock/profiles/<name>.json`, merged over [`profiles/default.json`](profiles/default.json). Profiles shipped in this repo's `profiles/` are used when you don't have your own. Example, [`profiles/openmatch.json`](profiles/openmatch.json):

```jsonc
{
  "domains": ["fonts.googleapis.com", "fonts.gstatic.com"],   // extra egress
  "host_ports": ["54321:54329"],                               // host.docker.internal, TCP
  "ports": [3000, 3001],                                       // container ports to publish on localhost
  "env": {
    "SUPABASE_URL": "http://${host}:54321",                    // ${host} = host.docker.internal
    "NEXT_PUBLIC_API_URL": "http://localhost:${port:3001}"     // ${port:N} = the host port N got
  },
  "install": "auto",        // or a command, or null. auto: pnpm/yarn/npm from the lockfile
  "node_modules": "auto",   // shadow every workspace package's node_modules with a per-sandbox volume
  "volumes": [],            // extra relative paths to shadow the same way
  "mounts": [],             // extra host dirs, "src[:dst][:ro|rw]" (see below)
  "claude_dirs": ["skills", "agents", "commands"],   // ~/.claude subdirs, read-only
  "claude_settings": true,  // copy ~/.claude/settings.json in, paths rewritten
  "node": "22",
  "claude_version": "latest"
}
```

**Ports.** Each sandbox publishes the profile's `ports` on `127.0.0.1`. The first sandbox of a profile gets them 1:1 (`3000→3000`), the next gets `+10` (`3010→3000`), and so on, so two worktrees can both run `next dev` on 3000. `${port:N}` in `env` resolves to the host port, which is what browser-side URLs need. `paddock ls` shows the mapping.

**Why volumes over `node_modules`?** The workspace is bind-mounted from macOS/Windows; Linux binaries must not land in your host `node_modules` and vice versa. Per-sandbox volumes shadow those directories. The pnpm store is one shared volume. `paddock reset` drops a sandbox's volumes.

## Skills, agents, hooks and extra directories

The sandbox never mounts `~/.claude` as a whole — it holds credentials, history and memory. Instead:

- **`claude_dirs`** (default `["skills", "agents", "commands"]`): each `~/.claude/<dir>` is mounted **read-only** at `/home/node/.claude/<dir>`. The agent can use your skills and subagents but not rewrite them. Add `"hooks"` (and whatever they depend on, e.g. `"gsd-core"`) if your hooks should run inside too.
- **`claude_settings`** (default `true`): your `~/.claude/settings.json` is *copied* into the sandbox at first start with `~/.claude` rewritten to `/home/node/.claude` and absolute `…/bin/node` paths reduced to `node`, so hook commands resolve inside the container. It's a copy: the agent may edit its own settings, your host file is untouched. `paddock reset` regenerates it.
- **`mounts`**: any other directories, `"~/notes"`, `"~/notes:/mnt/notes"`, `"~/scratch:rw"` — read-only unless you say `rw`.

Project-level `.claude/` (commands, settings) comes along with the checkout as usual.

## Commands

```
paddock up      [PATH] [-p PROFILE] [--rebuild]   build image if needed, create/start
paddock run     [PATH] [-p PROFILE] [-- args]     up + claude --dangerously-skip-permissions
paddock shell   [PATH]                            up + zsh
paddock exec    [PATH] -- <cmd>                   up + run a command inside
paddock ls                                        sandboxes, state, ports, workspaces
paddock stop    [PATH]
paddock rm      [PATH]                            remove container, keep volumes
paddock reset   [PATH]                            remove container + volumes (node_modules, Claude login)
paddock firewall [PATH]                           re-apply the egress rules
paddock init    [PATH] [-p PROFILE] [--force]     write profile + env skeleton
```

`PATH` defaults to the current directory; any directory inside a checkout works.

## How it works

- **Image per profile**, tagged `paddock/<profile>`, rebuilt automatically when the Dockerfile, scripts, or the profile's policy change (content hash in a label). `--rebuild` forces a no-cache build.
- **Container per workspace**, named `paddock-<repo>[-<dir>]`, labelled with workspace, profile and port map. `up` is idempotent; a stopped sandbox is restarted and the firewall re-applied.
- **First start** provisions: chowns the volume mountpoints, sets `safe.directory` and `gc.worktreePruneExpire=never` (so an agent's `git worktree prune` can't drop your unmounted sibling worktrees), activates the `packageManager` from `package.json` via corepack, runs the install command, and wires `gh` as git's credential helper if `GH_TOKEN` is set.
- **Every start** applies [`image/init-firewall.sh`](image/init-firewall.sh): flush, allow DNS/loopback, resolve the allowlist into an ipset, allow the host on the configured ports, default DROP, verify (`example.com` must fail, `api.github.com` must succeed).

## Limits and caveats

- Anything that needs Docker (e.g. `supabase start`, testcontainers) has to run **on the host**; expose it to the sandbox with `host_ports`. Mounting the Docker socket would hand the agent root on your machine.
- The allowlist is resolved to IPs at container start. CDN-backed hosts rotate; if something on the list stops resolving, `paddock firewall` re-resolves.
- Sibling worktrees are not mounted, so `git worktree list` inside shows them as *prunable*. `gc.worktreePruneExpire=never` protects them from a plain `prune`; `--expire now` would still remove them (recover with `git worktree repair` on the host).
- Docker Desktop on macOS: `host.docker.internal` is the VM's view of your Mac — the container reaches host services, but the browser on your Mac uses `localhost` and the published ports.
- No GPU, no Linux namespaces beyond what Docker gives you; this is a container, not a VM. For a stronger boundary, run Docker with a microVM runtime.

## Credits

The image and firewall are adapted from the [reference Claude Code devcontainer](https://github.com/anthropics/claude-code/tree/main/.devcontainer).
