# WP-M0-3 — distinct exit codes (Ask)

## Review 1 — opus, 2026-09-09, working-tree diff before commit 17e8ffc

Confirmed `install_policy` unchanged apart from the code argument; no path that
exited non-zero now exits zero; moving `docker_ok()` into `main` is right for
every subcommand and `init` correctly excluded.

**F1** `paddock:477` — **security.** `github_ranges()` was called mid-way
through writing `/etc/paddock` and *before* the firewall ran. A fetch failure
with no cache died with `1`, and on a freshly created container — which has no
rules until the script has run once — that left a running sandbox with open
egress, reported as an ordinary error. Pre-existing, but the new codes would
have taught the app that exit codes describe firewall state.
→ **Acted, beyond the minimal.** Ranges are fetched first; `github_ranges`
returns `None` instead of dying; the firewall runs with an empty ranges file,
the sandbox is marked CLOSED and paddock exits `6`. Observed: with the fetch
blocked, example.com and github.com unreachable, npm reachable, state file
`closed: true`, next `up` re-applies. This changes when the firewall runs on a
fetch failure (always, now) — a posture change in the fail-closed direction.
**Flagged to the human** in the session summary as an Ask-class decision made
autonomously.

**F2** `paddock:652` — `exec` without a command ran `ensure_up` first.
→ **Acted.** Check moved above it; observed no container created.

**F3** doc, code `5` — sandbox is left on the *provisioning* policy (profile +
npm + github), wider than the profile.
→ **Acted.** Row says so.

**F4** `paddock:186,406,408,430` — profile *content* errors exited `1` while
absence exited `4`; same button.
→ **Acted.** Invalid profile JSON, bad `${…}` template, missing `mounts`
source → `4`. `load_json` takes a code so the cache readers keep `1`.

**F5** style — unnamed `2`, tuple unpack hides `EXIT_CLOSED =` from grep.
→ **Declined.** Style; the comment above the tuple explains `2`.

## Decision review — opus, 2026-09-09

F1 did not need the human first: it was a defect, fail-closed, one line to
revert, reviewed before commit. But:

**DR1** — `--policies open,github` with a failed fetch ran the firewall OPEN and
still marked the sandbox CLOSED. → **Acted.** Ranges are not fetched in open
mode. Observed: open,github with the fetch blocked → exit 0, `closed: false`.

**DR2** — `closed` docs, the `ls` label and README still said "firewall
failed"; the die message overstated. → **Acted.** Label is now `CLOSED (policy
not fully applied)`; docs, README and message reworded; code-6 row notes the
provisioning-widened case.

**DR3** — `github_ranges()` returned `[]` on a parseable but empty response,
silently producing the state the change was meant to make loud.
→ **Acted.** Returns `None` when no usable CIDRs.

**DR4** — residual: `write_root_file` failure on a fresh container still dies
`1` before the firewall runs. → **Recorded** in PLAN.md under WP-M0-3.

**DR5** — PLAN.md contradicted the code. → **Acted.** WP body corrected, D6
added.

F2 note (no Docker → `3` even for a usage error) → **Acted** in the doc row.
