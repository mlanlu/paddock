# WP-FIX-1 — provisioning marker outlived the container

## Review 1 — opus, 2026-09-09, commit e42f668

**F1** `paddock:35-37` — the comment claims the settings copy lives in the
container filesystem; it goes to the `claude` volume and survives `rm`.
→ **Acted.** Comment and the PLAN.md paragraph corrected (0306198).

**F2** `paddock:35-37` — comment can be shorter and still carry the why.
→ **Acted.** Two-sentence version; now also records the hardening the
reviewer noted: the old marker was on a `node`-owned volume, so the agent could
delete it and make the next `up` re-provision with egress widened by
`PROVISION_POLICIES`. Root-owned closes that.

Clean: `/etc/paddock` exists in the image before the first `touch`;
`install_policy` precedes `provision` at both call sites; no leftover
references; firewall script never globs the directory.

## Decision review — opus, 2026-09-09

Agrees with both actions. Confirmed the hardening claim: the old marker sat on a
`node`-owned volume, and a deleted marker made the next `up` run
`install_policy` widened by `PROVISION_POLICIES`. Checked `init-volumes.sh`
(sudo-able, reads a node-writable target list) cannot reach `/etc/paddock`
because it guards every chown with `mountpoint -q`. No further action.
