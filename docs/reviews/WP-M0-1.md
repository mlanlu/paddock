# WP-M0-1 — paddock ls --json

## Review 1 — opus, 2026-09-09, commit c48e2f6

**F1** `paddock:715` — table not byte-identical for a state file with
`closed: true` and no `policy` key: was `CLOSED (firewall failed)`, now `?`.
No paddock version writes that state (both keys are saved together), but the
code now branches on it.
→ **Acted**, via F2.

**F2** `paddock:695`, `docs/cli-json.md` — `closed` lives inside the nullable
`policy` object, so the contract cannot say CLOSED for a sandbox without a
persisted policy. `closed` is a property of the sandbox, not the policy.
→ **Acted.** Hoisted to the row's top level. Deviates from the shape in
PLAN.md, which was wrong on this point; PLAN.md updated.

**F3** `paddock:694` — `ls` constructs a `Policy`, which reads every set file
and dies if one is missing, so one sandbox whose persisted sets no longer exist
kills the whole listing. `describe()` needs none of that I/O.
→ **Acted.** `describe_policy(sets, extra)` is a plain function; `Policy` and
both callers use it.

**F4** doc, `policy.sets` — "in effect" is wrong when closed, and the always-on
`claude` set is not listed.
→ **Acted.** Reworded.

**F5** doc, `policy.describe` — "the string the table shows" is false when
closed.
→ **Acted.** Reworded.

**F6** doc — `["open"]` means no firewall` is imprecise; `open` anywhere in the
list does.
→ **Acted.** Reworded.

**F7** doc — `host_ports` is persisted but omitted from `policy`, and the doc
implies completeness.
→ **Declined the field** — YAGNI, no consumer asks for it. **Acted on the
wording**: the doc now says the object is the domain policy and host-port
access is not included.

**F8** `paddock:696-697`, `712-719` — port pair parsed twice; `r` is a dict and
then a list in the same function.
→ **Acted.** `port_pairs(label)` helper, shared with `cmd_info`; renamed.

Follow-up commit: 179f2db.

## Decision review — opus, 2026-09-09

No decline wrong. **D1**: the reworded `closed` row said DNS-only, but since
17e8ffc `closed` is also set when GitHub's ranges were unavailable and
everything else applied. → **Acted.** Row now matches the exit-6 wording.
**D2**: `policy: null` cause over-specified. → **Acted.** **D3**: two more
label parses in `reserved_host_ports` and `ensure_up`. → **Acted.** Both use
`port_pairs`; observed ports line unchanged on `up` and table identical.
