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
