# The CLI's JSON contracts

`paddock` is the engine behind the desktop app, and the app must never parse
the human-readable output. Every command the app depends on has a `--json`
flag whose shape is documented here. Changing a shape is a contract change:
update this file in the same commit and say so in the commit message.

All JSON goes to stdout. Progress lines (`paddock: …`) go to stderr as always,
so a consumer can pipe stdout straight into a parser.

## `paddock ls --json`

A list with one object per sandbox, sorted by container name. An empty list
when there are none (the table prints `no sandboxes`).

```json
[
  {
    "container": "paddock-openmatch-openmatch-web",
    "state": "running",
    "repo": "openmatch",
    "profile": "openmatch",
    "workspace": "/Users/you/Documents/openmatch-web",
    "policy": {
      "sets": ["github", "npm"],
      "extra": ["fonts.googleapis.com"],
      "describe": "github,npm +1",
      "closed": false
    },
    "ports": [{"host": 3010, "container": 3000}]
  }
]
```

| Field | Meaning |
|---|---|
| `container` | Docker container name, `paddock-<repo>[-<dir>]`. |
| `state` | Docker's state string: `running`, `exited`, `created`, … |
| `repo` | Name of the directory that owns the real `.git`; shared by every worktree of a repo. |
| `profile` | Profile the sandbox was created with. |
| `workspace` | Absolute host path of the sandboxed directory. |
| `policy` | The egress policy last applied by paddock, or `null` if paddock has not applied one yet (no state file — the sandbox predates policy persistence). |
| `policy.sets` | Policy sets in effect, e.g. `["github", "npm"]`. `["open"]` means no firewall. |
| `policy.extra` | Extra domains from the profile and `--allow`. |
| `policy.describe` | The string the table shows, e.g. `github,npm +1`, `strict`, `open`. |
| `policy.closed` | `true` when the last firewall run failed and the sandbox is DNS-only. The table shows `CLOSED (firewall failed)`. |
| `ports` | Published ports as pairs. `host` is on `127.0.0.1`; `container` is the port inside. Empty list when the profile publishes none. |

## `paddock info PATH --json`

Resolves `PATH` exactly as `paddock up PATH` would — repo, container name,
profile, policy, ports — and reports what it found without creating,
starting or writing anything. It is how the app shows what *will* happen
before the user commits, and how it knows to offer `paddock init` when there is
no profile. It needs the Docker daemon, to look up the container.

```json
{
  "root": "/Users/you/Documents/openmatch-web",
  "repo": "openmatch",
  "name": "openmatch-web",
  "container": "paddock-openmatch-openmatch-web",
  "common_git": "/Users/you/Documents/openmatch/.git",
  "profile": "openmatch",
  "profile_path": "/Users/you/.config/paddock/profiles/openmatch.json",
  "env_file": "/Users/you/.config/paddock/env/openmatch.env",
  "state": "exited",
  "provisioned": null,
  "ports": [{"host": 3010, "container": 3000}],
  "policy": {"sets": ["github", "npm"], "extra": [], "describe": "github,npm"}
}
```

| Field | Meaning |
|---|---|
| `root` | The directory that would be sandboxed: the git toplevel of `PATH`, or `PATH` itself outside a repo. |
| `repo` | Name of the directory that owns the real `.git`; the default profile name. |
| `name` | Basename of `root`. Equals `repo` unless this is a linked worktree or a differently named clone. |
| `container` | The container name `up` would use. |
| `common_git` | Absolute path of the shared `.git` when `root` is a linked worktree, else `null`. |
| `profile` | Profile name that would apply: `--profile`, else `repo`, else `default`. |
| `profile_path` | File the profile was read from, or `null` when only `profiles/default.json` applies. `null` is the cue to offer `paddock init`. |
| `env_file` | `~/.config/paddock/env/<profile>.env` if it exists, else `null`. |
| `state` | Docker's state string for the container, or `null` when it does not exist. |
| `provisioned` | Whether first-start provisioning has run. Only knowable for a running container (it is a `docker exec`); `null` otherwise. |
| `ports` | Published port pairs, as in `ls`. For an existing container, the actual mapping; otherwise the mapping `up` would assign right now, which may differ by the time `up` runs. |
| `policy` | The profile's egress policy, as `up` applies it on a fresh start. `--policies`/`--allow` are not consulted; a running sandbox may carry a different live policy, which `ls --json` reports. |

## Exit codes

Every command exits `0` on success. Failures the app wants to react to
differently get their own code; everything else is `1`.

| Code | Meaning | What a caller can do |
|---|---|---|
| `1` | Any other failure. The message on stderr says what. | Show the message. |
| `2` | Usage error: unknown command or flag, `exec` without a command. argparse owns this code. | Bug in the caller. |
| `3` | Docker is not on `PATH` or the daemon is not running. Every command except `init` checks this first. | Offer to start Docker Desktop. |
| `4` | Configuration missing or wrong: a `--profile` that does not exist, a policy set named by the profile or `--policies` that does not exist, invalid JSON in a profile, a bad `${…}` template in its `env`, or a `mounts` source that does not exist. | Offer `paddock init` or open the config directory. |
| `5` | Provisioning failed (dependency install, corepack, …). The sandbox is running on the **provisioning** policy, which is the profile policy widened by `npm` and `github`, and stays that way until `paddock provision` succeeds. | Offer `paddock provision` after the network is fixed; show the sandbox as wider than its profile until then. |
| `6` | The policy could not be fully applied and the sandbox is CLOSED: either the firewall script failed and egress is DNS only, or GitHub's IP ranges could not be fetched and the firewall was applied without them. `ls --json` reports `closed: true`. | Offer `paddock firewall` once the network is back. |
| `7` | The command needs a running sandbox (`firewall`, `provision`) and there is none. | Offer `paddock up`. |

`run`, `shell` and `exec` exit with the exit code of the command they ran
inside the sandbox once it is up. A failure *before* that uses the codes above.
