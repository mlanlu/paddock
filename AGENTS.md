---
name: paddock-dev
description: "How to develop for paddock — YAGNI, colocated docs, minimal increments, ship/show/ask, and the review-agent protocol. Use for any change to the paddock CLI, image, policies, profiles, or the desktop app."
---

# Developing for paddock

paddock is a security tool. Its value is that the sandbox is actually closed —
default-deny egress, no host credentials, no Docker socket, policy the agent
cannot reach. Every change is measured against that first, and against
simplicity second. Nothing else ranks.

Read `PLAN.md` before starting. It holds the work packages, their status, and
the open questions. Update it as you go; it is the shared state between
sessions and between agents.

## This file

`AGENTS.md` is the canonical copy. `CLAUDE.md` and
`.claude/skills/paddock-dev/SKILL.md` are symlinks to it, so Claude Code and
Codex read the same guidance and it cannot drift. Edit `AGENTS.md`; never
replace a symlink with a copy. The YAML frontmatter at the top is what
registers this as a Claude skill — harmless as prose to every other reader.

Subsystems carry their own `AGENTS.md`, which both tools merge by proximity.
That is the same rule as the documentation one below: guidance lives next to
what it governs.

## Skills

Skills hold guidance needed only *sometimes* — product context behind a
decision, a release checklist, a specialised review. They live in
`.claude/skills/<name>/SKILL.md` with `name` and `description` frontmatter.

Claude Code discovers them by itself. **Codex has no skill mechanism** and
reads only what it is pointed at, so the index below is the pointer. Adding a
skill without adding its row makes it invisible to half the tools that need it.

| Skill | Read it when |
|---|---|
| `paddock-dev` | Any change to this repo. Same text as this file, already loaded via `CLAUDE.md` — re-read only if it is not in context. |

Guidance that must apply to *every* change belongs in this file, not in a
skill. Skills exist so this file stays short as they multiply.

## The repo

| Path | What it is |
|---|---|
| `paddock` | The whole CLI. One Python file, standard library only. |
| `image/` | Sandbox `Dockerfile` and the root-owned firewall and volume scripts. |
| `policies/` | Egress policy sets — one host per line, additive. |
| `profiles/` | `default.json` and the per-repo profiles merged over it. |
| `PLAN.md` | Work packages, status, open questions, decisions. |
| `docs/reviews/` | Review records, one file per work package. |
| `.claude/skills/` | On-demand skills, indexed above. |

Configuration and state live outside the repo, deliberately: profiles, env
files and policy overrides in `~/.config/paddock`, build and sandbox state in
`~/.cache/paddock`. The agent inside a sandbox cannot reach either.

## Principles

**YAGNI, without exception.** Build what the current work package needs and
nothing else. No configuration knobs for cases nobody has hit, no abstraction
layers with one implementation, no "we'll want this later." Later is when you
write it, and by then you will know what it should look like. If you catch
yourself adding a parameter that every caller passes the same value for, delete
it.

**Simplicity when stuck.** Small problems get solved, not escalated. Reach for
the plainest thing that works — a function over a class, a list over a
registry, a literal over a lookup table. If two designs both work, ship the one
that is easier to delete.

**Standard over novel — but informed.** Prefer libraries and patterns that are
widely used, well documented, and well tested. This is not a preference for old
technology; a two-year-old library with real adoption and good docs beats both
a fifteen-year-old unmaintained one and last month's state of the art. Judge on
adoption, documentation, test coverage, and maintenance, then decide. Write the
reasoning into the decision log in `PLAN.md` when the choice is not obvious.

**Escalate genuine uncertainty.** When a decision would be expensive to
reverse, changes the security posture, or you simply do not have the
information — stop and ask the human. Do not ask about small, reversible
things; solve those yourself.

## Documentation

Docs are read by humans and by agents. Both need the same thing: prose that
says what the subsystem is for, what the constraints are, and why it is shaped
this way. Write in full sentences. Do not write for one audience at the other's
expense.

**Documentation lives next to what it describes.** A subsystem's docs go in the
subsystem's directory, not in the root.

| Location | Documents |
|---|---|
| `README.md` | What paddock is, install, commands, the threat model. User-facing. |
| `image/` | The sandbox image and the firewall script. |
| `policies/` | What a policy set is, how to add one, why there is no deny list. |
| `profiles/` | The profile schema and how merging works. |
| `app/` | The desktop app: architecture, how it talks to the CLI. |
| `docs/` | Cross-cutting process artifacts — the JSON contracts, review records. |

A subsystem that needs *agents* to behave differently gets an `AGENTS.md` in
its directory as well; a subsystem that needs *readers* to understand it gets a
`README.md`. Most only ever need the second.

The root `README.md` links to them; it does not absorb them. Create a
subsystem's doc when the subsystem becomes non-obvious, not preemptively.

**A work package is not done until its docs are updated in the same commit.**
Docs that lag the code are worse than no docs, because they are believed.

## Code comments

The code should read clearly enough that a comment explaining *what* it does is
redundant. Delete those. Comments earn their place only by explaining *why* —
the constraint, the failure that motivated the shape, the thing a reader would
otherwise "fix" and break.

These are the standard, from the existing code:

```python
# paddock:505 — why this is an `if` and not a shorter idiom
# `if`, not `A && B || echo`: an `|| echo` at the end of an && chain
# would turn any earlier failure into exit 0.

# paddock:492 — why a git config is set at all
# sibling worktrees are not mounted and would look prunable
```

Both encode a bug that was actually hit. Neither restates the code.

When the "why" runs longer than a couple of lines, or explains a whole
subsystem rather than a line, it belongs in that directory's docs instead.
Prefer moving it. Module and class docstrings are the right home for
constraints that govern a whole unit — see the `Policy` docstring at
`paddock:209`.

## Working in increments

**Break work into the smallest shippable steps.** One coherent change per
commit, each leaving the tree working. If a work package cannot be split, that
is a signal — the design is probably tangled, or you do not understand it yet.
Say so rather than writing one large commit quietly.

Commits follow the existing convention: `type(scope): imperative summary`, e.g.
`feat(cli): ls --json`, `fix(firewall): survive DNS round-robin`, `docs: …`.
The summary says what changed and, where it fits, why.

## Ship / Show / Ask

Every work package is classified before work starts, and the class is recorded
in `PLAN.md`.

| Class | Meaning | Use when |
|---|---|---|
| **Ship** | Commit. No review. | Small, obvious, reversible. Docs, a contained fix, an additive flag with no security surface. |
| **Show** | Commit, then have a review agent look at it. Do not block on the review, but act on it before the next work package. | Confident but non-trivial. New commands, refactors, anything another program will depend on. |
| **Ask** | Review agent first, human if it escalates. Commit after. | Could not be broken into small increments; touches the firewall, policy resolution, credentials, or container privileges; or you are unsure. |

**Anything that changes the security posture is Ask, regardless of size.** That
means `image/init-firewall.sh`, `Policy` and `resolve_policy`, `install_policy`,
`create_container`'s mount and capability arguments, and anything touching
`/etc/paddock` or the env files.

## The review protocol

Reviews are done by a **review-class agent**. The review class is configured in
`PLAN.md` under *Review class*; keep that list as the single source of truth
rather than hardcoding model names here.

1. **Request the review.** Spawn a review-class agent with: the commit range or
   diff, the work package's goal from `PLAN.md`, and the principles above. Ask
   for findings, not prose — each finding a concrete defect or a concrete
   simplification, with a file and line.

2. **Decide on each finding.** Either act on it, or decline it with a reason.
   "Declined — YAGNI, no caller needs this" is a good reason. "Declined —
   disagree" is not. Record both the finding and your decision.

3. **Have the decisions reviewed.** Spawn a second review-class agent with the
   findings and your decisions, and ask whether any decline was wrong. This
   catches the case where the reviewer was right and you talked yourself out of
   it. Act on that verdict, then move on — do not open a third round. If the
   two disagree and the subject is security, escalate to the human.

4. **Record it**, then continue to the next work package.

### Recording a review

One file per work package, `docs/reviews/<WP-id>.md`, appended to across
rounds. Keep it short — it is a record, not an essay.

```markdown
# WP-M0-1 — paddock ls --json

## Review 1 — <reviewer>, <date>, commits abc1234..def5678

**F1** `paddock:690` — `ports` is emitted as a string, not a list.
→ **Acted.** Now `[{host, container}]`.

**F2** `paddock:684` — no test coverage.
→ **Declined.** Needs a running Docker daemon; see the testing note in PLAN.md.

## Decision review — <reviewer>, <date>

Agrees with both. No further action.
```

## Verification

There is no test suite, and most code paths need a running Docker daemon, so
work packages carry a manual verification checklist in `PLAN.md` instead. Run
it and say what you actually observed — not that you "verified" it. If a step
was skipped because the environment could not run it, say which and why.

Agent sessions often run inside a paddock sandbox with no Docker socket and no
Rust toolchain. Do not claim a build or a container test passed there. Write
the checklist, run what you can, and hand the rest to the human explicitly.

Revisit test infrastructure when the JSON contracts have a real consumer. Until
then it would be scaffolding for one caller — YAGNI.
