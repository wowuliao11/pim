# AGENTS.md

## 1. Purpose & Authority

This document acts as the **primary constitution** for all AI agents and human developers working on this project. It defines the core process rules, lifecycle management, and architectural standards that MUST be followed.

**Status:**

- This file is the **Source of Truth** for project process.
- If this file conflicts with `INSTRUCTIONS.md` or `README.md`, this file takes precedence.
- Only human maintainers or extensive planning sessions may authorize changes to this file.

## 2. Documentation Architecture

### Documentation Map

| Document             | Purpose                                                   | Maintainer | Update Frequency                         |
| -------------------- | --------------------------------------------------------- | ---------- | ---------------------------------------- |
| `AGENTS.md`          | Process rules, lifecycle methodology (This file)          | Human      | Rarely (process changes)                 |
| `INSTRUCTIONS.md`    | Task-level guidance, style, formatting                    | Human      | As needed                                |
| `README.md`          | Entry point, high-level overview                          | Human/AI   | Major milestones                         |
| `/docs/decisions/`   | **Architecture Decision Records (ADRs)** — see §2.1       | AI + Human | One per significant architectural choice |
| `/docs/design.md`    | **Current** accepted system design                        | AI + Human | After implementation stabilizes          |
| `/docs/prompts/`     | Reusable workflows/prompts                                | AI + Human | Optional, on discovery                   |
| `/plans/` (optional) | Implementation scratchpads for multi-session work — §2.2  | AI + Human | Temporary; deleted once work lands       |

### 2.1 Roles of Specific Directories

#### `/docs/decisions/` — Architecture Decision Records

- **Definition:** The durable log of *why* the system looks the way it does. Every non-trivial architectural choice (identity provider, transport, config model, observability story, release automation, monorepo layout, etc.) MUST be captured as an ADR.
- **Authority:** ADRs are the primary architectural source of truth. `docs/design.md` describes *what* the system is; `docs/decisions/` explains *why*.
- **Format:** Every ADR MUST follow `docs/decisions/0000-template.md` and include, at minimum:
  - Status (`Proposed` | `Accepted` | `Deprecated` | `Superseded by ADR-NNNN`)
  - Date, Deciders
  - Context (what forces are at play, with citations)
  - Decision (the actual choice, stated as a single sentence plus supporting detail)
  - Consequences (Positive / Negative / Locked-in / Follow-up)
  - Alternatives considered (at least one rejected option with a concrete reason)
  - References — MUST cite `path:line` for every claim about the codebase, and MUST include an `Originated from` line if the decision grew out of a plan or PR
- **Lifecycle:** `Proposed` → `Accepted` → (optionally) `Deprecated` or `Superseded by ADR-NNNN`. ADRs are **append-only**: when a decision changes, write a new ADR and set the old one to `Superseded`. Do not edit the historical body.
- **Numbering:** Strictly sequential, zero-padded to four digits. Never reuse a slot, even if an ADR is abandoned before acceptance.
- **Surfacing:** Per ADR-0002, every PR that touches decided subsystems MUST link the relevant ADRs in its description so reviewers see the rationale inline.

#### `/docs/design.md`

- **Definition:** The definitive record of how the system currently works or _will_ work in the immediate agreed-upon future.
- **Rule:** Update this file only after a decision has been recorded in `/docs/decisions/` **and** the implementation has stabilized. `design.md` restates the current ADR-blessed architecture in readable prose; it is not where new decisions get proposed.
- **Constraint:** Future code generation MUST follow the patterns defined here. If code deviates, fix the ADR (new or superseding) first, then update `design.md`.

#### `/docs/prompts/`

- **Definition:** A library of useful prompts for specific tasks (e.g., "Generate a new CRUD module").
- **Rule:** These are helpers, not strict rules. They do not override `AGENTS.md`.

### 2.2 `/plans/` — Optional Implementation Scratchpad

`/plans/` is **not** the record of architectural decisions. It is an optional, temporary scratchpad for breaking multi-session implementation work into phases and tracking progress. Plans are disposable; ADRs are permanent.

- **When to create a plan:** Only when the trigger conditions in §3.1 fire *and* the work needs multi-session phase tracking that a PR description cannot carry. Most changes — including most single architectural decisions — should skip plans entirely and go directly to an ADR plus a PR.
- **Format:** Context/Goal, phased steps with checkboxes, acceptance criteria per phase, status.
- **Relationship to ADRs:** If a plan surfaces an architectural decision, extract that decision into an ADR as soon as it is confirmed. The plan retains only the task list; the *why* lives in the ADR with `Originated from: plans/NNN-*.md` in References.
- **End of life:** When all phases land, delete the plan file. Do not keep completed plans as historical artifacts — the ADRs and git history already serve that purpose.

## 3. Process Rules

### 3.1 When to Record What

The decision flow for any non-trivial change:

1. **Is there an architectural choice** (identity, transport, config shape, observability, release, monorepo layout, dependency substitution, API contract)? → Write an **ADR** in `/docs/decisions/` following §2.1. This is required *regardless* of whether a plan also exists.
2. **Does the work need multi-session phase tracking** that a PR description cannot carry? → Optionally add a **plan** in `/plans/` following §2.2. Extract any decisions it contains into ADRs as soon as they are confirmed.
3. **Neither of the above?** → Ship a single PR with a clear description using `.github/pull_request_template.md`. No ADR, no plan.

#### When an ADR IS required

An ADR MUST be drafted and committed (usually in the same PR as the code change, or immediately before it) when **any** of the following is true:

- **Architecture:** The change introduces or modifies a cross-cutting pattern (auth, transport, observability, release, build, configuration model, monorepo layout).
- **Public contract:** The change modifies a public API surface in `libs/` or a proto contract in `rpc-proto` in a non-trivial way.
- **Dependency substitution:** The change replaces or adopts a significant external service or tool (identity provider, release automation, database, message bus).
- **Reversal:** The change reverses a previous ADR. (Write a new ADR whose Status is `Accepted` and mark the old one `Superseded by ADR-NNNN`.)

#### When an ADR is NOT required

- Single-PR bug fixes, refactors, small feature additions confined to one crate with no cross-cutting impact.
- Dependency version bumps that do not change behaviour.
- CI tweaks, lint fixes, doc edits, test additions, config-value tuning.
- Anything that does not change *how* the system is built or *why* it is built that way.

#### When a plan IS required

A plan MUST be drafted, confirmed by the user, and committed before code when **any** of the following is true **and** a PR description is insufficient:

- **Scope:** The work cannot reasonably fit in a single PR (estimated > ~400 lines of diff or > ~2 working days of effort).
- **Surface area:** The change touches **3 or more crates**, or modifies a public API surface in `libs/`, or alters a proto contract in `rpc-proto`.
- **Migration:** The change requires a coordinated migration across data shape, config shape, proto contract, or a CI/release gate.
- **Multi-session:** The user explicitly frames the request as multi-session or multi-phase, or asks for a plan.

Even when these triggers fire, prefer to ship without a plan if the work is really one PR with careful staging. A plan is the right tool when the phase list itself carries information a PR description cannot.

#### Procedure when a plan IS required

1. Create `/plans/NNN-<short-slug>.md` with Context/Goal, phased steps, acceptance criteria per phase, status.
2. Ask the user to confirm the plan before writing code.
3. As soon as any architectural decision in the plan is confirmed, extract it into an ADR under `/docs/decisions/`. Do not let decisions live only in the plan.
4. Update the plan's Status as phases complete.
5. Delete the plan when all phases land.

> **Heuristic:** If you are unsure whether a change needs an ADR, err toward writing one — it is cheap, append-only, and compounds over time. If you are unsure whether a change needs a plan, err toward *not* writing one and over-communicate in the PR description instead.

### 3.2 Design Evolution

1. **Trigger:** An ADR moves to `Accepted` **and** the implementation lands on `main`.
2. **Action:** Update `/docs/design.md` to reflect the new reality, with a link back to the governing ADR(s).
3. **Goal:** Ensure `design.md` always represents the _actual_ system state + immediate committed changes, preventing "documentation drift".

### 3.3 Reminder Protocol

- The Agent SHOULD remind the user to update documentation if:
  - A significant architectural decision was made in chat but no ADR was written.
  - An ADR is `Accepted` and the implementation has landed, but `docs/design.md` is untouched.
  - A plan phase completed but the corresponding ADR is missing or stale.

## 4. Maintenance Standards

- **Maintainer:** Both AI Agents and Humans are responsible for keeping these documents in sync.
- **Forbidden:** Do NOT create random documentation files outside this structure without updating the `AGENTS.md` map.

## 5. Language Policy

- **Conversational Responses:** For conversational responses to the user, the agent **MUST** respond in Chinese.
- **Repository Mutations:** For any action that mutates the repository (creating/editing files, writing documentation, modifying code, proposing commit messages or PR descriptions), the agent **MUST** use English.
- **Invariance:** This rule applies regardless of the user's input language.
- **Ambiguity:** If the agent is unsure whether an action constitutes a repository mutation, it **MUST** assume that English is required.
