# WP-M0-2 — paddock info PATH --json

## Review 1 — opus, 2026-09-09, commit d5882fb

Mutation audit clean: no `github_ranges`, no state write, no docker create or
start; `host_port_free` binds and closes a socket, nothing more.

**F1** doc — `info`'s `policy` lacks `closed`, and the `ls` section documented
it as part of the policy object.
→ **Acted.** `closed` is now a top-level `ls` row field (WP-M0-1 F2); the `info`
row says explicitly it is absent there and why.

**F2** doc — `profile` and `env_file` are what a *fresh* start would use; an
existing container keeps the profile it was created with.
→ **Acted** on the docs. Declined reading the label instead: `ls --json`
already reports the container's profile, and `info` is about what `up` would
resolve.

**F3** `paddock:695`, `730` — duplicated port label parse.
→ **Acted.** `port_pairs()`.

**F4** `paddock:725` — `docker_ok()` means `info` can show nothing without
Docker, including the profile lookup that needs none.
→ **Accepted as is, documented.** It exits `3` (WP-M0-3) and the app handles
that first. Emitting partial JSON would be a second shape for one caller.

**F5** `paddock:738` — a failed `docker exec` reads as `provisioned: false`, not
"unknown".
→ **Declined.** `false` is the safe direction, and the trigger (workspace path
gone inside a running container) is not one anybody has hit.

**F6** nit — human output prints `None`/`True`.
→ **Declined.** It is a debugging aid; the app reads `--json`.

## Decision review — opus, 2026-09-09

No decline wrong. **D1**: the F2 doc sentence claimed the label and resolved
profile differ only on `-p`; they also differ after `paddock init` on a repo
that already had a sandbox, or after a profile file is removed.
→ **Acted.** Sentence corrected. Also noted a pre-existing container-name
collision (two clones with the same basename) as PLAN.md open question 6.
