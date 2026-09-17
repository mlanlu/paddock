# WP-CODEX-2 — app contract and preview

## Review 1 — GPT-6 Astra, 2026-09-17, commit eea7ad5

**F1** `app/src/main.ts:214` — A running Claude sandbox's folder preview showed
Codex, but its “Restart” button called `paddock up`, which retains the running
Claude agent. The preview promised a different agent from the resulting state.
→ **Acted.** The preview now labels the policy as a fresh start, explains that
`up` retains a running sandbox's live agent and policy, and calls the action
“Ensure running.”

## Decision review — GPT-6 Astra, 2026-09-17

Agrees that the fresh-start label, running-state explanation, and “Ensure
running” action resolve F1. No further findings.
