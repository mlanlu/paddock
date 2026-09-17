# WP-CODEX-1 — Codex image, isolation, and egress

## Review 1 — GPT-6 Astra, 2026-09-17, uncommitted diff

**F1** `paddock:636` — Switching agents on a running sandbox resolved the
profile policy again. A sandbox tightened with `firewall --policies ''` would
regain GitHub/npm access, or `open` if the profile allowed it.
→ **Acted.** `run` without policy flags now retains the live sets, extra
domains, and host ports while changing only the selected agent. A focused
check switched from Codex to Claude against an `open` profile and observed the
persisted strict policy remain strict.

## Decision review — GPT-6 Astra, 2026-09-17

**F1 incomplete** `paddock:639` — If provisioning had failed, switching agents
retained the strict final policy but derived the temporary provisioning policy
from the profile. An `open` profile could restore unrestricted egress during
the retry and leave it that way after another failure.
→ **Acted.** The temporary policy now widens the retained policy only by npm
and GitHub; `provision` uses the same path. Focused checks observed strict
egress after the agent switch and throughout both provisioning paths.
