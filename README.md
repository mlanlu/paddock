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
| **Network** | Default-deny egress (iptables/ipset), fail-closed. What's allowed is the union of the profile's **policy sets** (`github`, `npm`, `pypi`, …; the Anthropic API is always on), its extra `domains`, and ad-hoc `--allow` hosts. Written into the root-owned `/etc/paddock` by paddock at every start — the agent can't touch it; you can retighten or loosen a running sandbox with `paddock firewall`. |
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
  "policies": ["github", "npm"],                               // egress policy sets (see below)
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

### Egress policy: modes that compose

Policy sets are plain files in [`policies/`](policies/) (override or add your own in `~/.config/paddock/policies/`):

| set | allows | note |
|---|---|---|
| `claude` | api.anthropic.com, sentry, statsig | always on — Claude Code can't run without it |
| `github` | GitHub's published IP ranges + release asset hosts | git over HTTPS, `gh`, release downloads |
| `npm` | registry.npmjs.org, registry.yarnpkg.com | npm / pnpm / yarn / corepack |
| `pypi` | pypi.org, files.pythonhosted.org | pip / uv |
| `apt` | deb.debian.org, security.debian.org | apt-get inside the sandbox |
| `vscode` | marketplace + server download | VS Code *Attach to Running Container* |
| `open` | everything | no firewall; for trusted tasks |

The effective policy is **additive**: the union of the profile's `policies`, its `domains`, and any `--allow` hosts. To tighten, leave a set out. A policy set with flags stays in force while the sandbox runs; a fresh start returns to the profile's. `paddock ls` shows what each sandbox currently has (or `CLOSED` if the last firewall run failed). Some useful modes:

```bash
paddock run                             # profile policy, e.g. github + npm
paddock run --policies ''               # strict: Anthropic API only. The agent edits; you review and push from the host
paddock run --policies npm              # installs allowed, no GitHub — no git push, no curl | bash from raw.githubusercontent.com
paddock run --allow api.stripe.com      # one-off extra host
paddock firewall --policies github,npm  # re-tighten/loosen a RUNNING sandbox, no restart; sticks until the next stop/start
paddock run --policies open             # no firewall
```

**Why no deny list?** The firewall matches IPs, and hosts share them: `raw.githubusercontent.com` and `gist.github.com` sit in the same ranges as `github.com` and `api.github.com`. A sandbox that can `git push` can also fetch a script from a raw URL — you can't allow one and deny the other at this layer. So GitHub is all-or-nothing, and paddock doesn't offer a `deny` that would only work for hosts with dedicated IPs. Exact per-hostname control needs an L7 filtering proxy in front of the sandbox; that's the natural next mode.

**Dependency installs** during provisioning run with the profile policy widened by `npm` + `github` (never the open internet), then the real policy is applied. Provisioning happens once per **container**: a stopped sandbox restarts without it, but `paddock rm` followed by `paddock up` builds a fresh container and provisions that one too (cheap — the `node_modules` volumes survive). `paddock provision` re-runs it on demand.

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
                (up/run/shell/exec also take --policies and --allow)
paddock ls                                        sandboxes, state, ports, workspaces
paddock stop    [PATH]
paddock rm      [PATH]                            remove container, keep volumes
paddock reset   [PATH]                            remove container + volumes (node_modules, Claude login)
paddock firewall [PATH] [--policies ..] [--allow ..]   re-apply egress policy to a running sandbox
paddock provision [PATH]                          re-run first-start provisioning (deps install)
paddock init    [PATH] [-p PROFILE] [--force]     write profile + env skeleton
```

`PATH` defaults to the current directory; any directory inside a checkout works.

## How it works

- **One image** (`paddock/sandbox:node<version>`), rebuilt automatically when the Dockerfile or scripts change (content hash in a label). `--rebuild` forces a no-cache build. Policy is not in the image.
- **Container per workspace**, named `paddock-<repo>[-<dir>]`, labelled with workspace, profile and port map. `up` is idempotent; a stopped sandbox is restarted and the firewall re-applied.
- **A container's first start** provisions: chowns the volume mountpoints, sets `safe.directory` and `gc.worktreePruneExpire=never` (so an agent's `git worktree prune` can't drop your unmounted sibling worktrees), activates the `packageManager` from `package.json` via corepack, runs the install command, and wires `gh` as git's credential helper if `GH_TOKEN` is set.
- **Every start** paddock writes the resolved policy (mode, domains, GitHub flag, host ports) into the root-owned `/etc/paddock` via `docker exec -u root`, then runs [`image/init-firewall.sh`](image/init-firewall.sh): DROP policies and the fixed rules first, then resolve the allowlist into an ipset, then verify that `example.com` is unreachable. **Fail-closed**: if resolution fails (no network), the sandbox is left with DNS only and paddock tells you to `paddock firewall` once the network is back.

## Limits and caveats

- Anything that needs Docker (e.g. `supabase start`, testcontainers) has to run **on the host**; expose it to the sandbox with `host_ports`. Mounting the Docker socket would hand the agent root on your machine.
- The allowlist is resolved to IPs at container start. CDN-backed hosts rotate; if something on the list stops resolving, `paddock firewall` re-resolves.
- **Docker Desktop behind a VPN** (ProtonVPN, WireGuard, …) often has broken container egress: DNS works but TLS/apt/npm time out (MTU: the tunnel is ~1380, the VM assumes 1500). Symptoms: image build fails at `apt-get`, provisioning fails at `corepack`, the firewall reports it could not fetch GitHub ranges. Fix on the Docker side: Settings → Resources → Network, lower the MTU, or exclude Docker from the VPN (split tunneling), or update Docker Desktop.
- Sibling worktrees are not mounted, so `git worktree list` inside shows them as *prunable*. `gc.worktreePruneExpire=never` protects them from a plain `prune`; `--expire now` would still remove them (recover with `git worktree repair` on the host).
- Docker Desktop on macOS: `host.docker.internal` is the VM's view of your Mac — the container reaches host services, but the browser on your Mac uses `localhost` and the published ports.
- No GPU, no Linux namespaces beyond what Docker gives you; this is a container, not a VM. For a stronger boundary, run Docker with a microVM runtime.

## Credits

The image and firewall are adapted from the [reference Claude Code devcontainer](https://github.com/anthropics/claude-code/tree/main/.devcontainer).
