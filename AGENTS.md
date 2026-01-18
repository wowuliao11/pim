# AGENTS.md

## 1. Purpose & Authority
This document acts as the **primary constitution** for all AI agents and human developers working on this project. It defines the core process rules, lifecycle management, and architectural standards that MUST be followed.

**Status:**
- This file is the **Source of Truth** for project process.
- If this file conflicts with `INSTRUCTIONS.md` or `README.md`, this file takes precedence.
- Only human maintainers or extensive planning sessions may authorize changes to this file.

## 2. Documentation Architecture

### Documentation Map

| Document | Purpose | Maintainer | Update Frequency |
|----------|---------|------------|------------------|
| `AGENTS.md` | Process rules, lifecycle methodology (This file) | Human | Rarely (Process changes) |
| `INSTRUCTIONS.md` | Task-level guidance, style, formatting | Human | As needed |
| `README.md` | entry point, high-level overview | Human/AI | Major milestones |
| `/plans/` | Feature/capability roadmaps and phased delivery | AI + Human | Start of any multi-session work |
| `/docs/design.md` | **Current** accepted system design | AI + Human | After implementation stabilizes |
| `/docs/prompts/` | Reusable workflows/prompts | AI + Human | Optional, on discovery |

### Roles of Specific Directories

#### `/plans/`
- **Definition:**  A place for tracking long-term intent.
- **Requirement:** Any request that spans multiple coding sessions or involves complex architectural changes MUST start with a plan file in this directory.
- **Format:** Plans MUST include:
    - Context/Goal
    - Phased implementation steps (Phase 1, Phase 2, etc.)
    - Acceptance criteria for each phase
    - Status tracking
- **Evolution:** Plans are mutable. They evolve as we learn. They are NOT the final documentation.

#### `/docs/design.md`
- **Definition:** The definitive record of how the system currently works or *will* work in the immediate agreed-upon future.
- **Rule:** This file should ONLY be updated when a design has been agreed upon or a Phase of a plan has been stabilized/completed.
- **Constraint:** Future code generation MUST follow the patterns defined here. If code deviates, this document must be updated first (or concurrently).

#### `/docs/prompts/`
- **Definition:** A library of useful prompts for specific tasks (e.g., "Generate a new CRUD module").
- **Rule:** These are helpers, not strict rules. They do not override `AGENTS.md`.

## 3. Process Rules

### 3.1 Long-Term Requirements & Planning
1. **Trigger:** When a user request is identified as "complex" or "multi-step" (cannot be finished in one response).
2. **Action:** The Agent MUST Create a new file in `/plans/` (e.g., `/plans/001-user-auth.md`).
3. **Content:** Draft a phased approach.
4. **Validation:** Ask the user to confirm the plan before writing code.

### 3.2 Design Evolution
1. **Trigger:** When a Plan Phase is marked "Complete" or "Stable".
2. **Action:** The Agent MUST update `/docs/design.md` to reflect the new reality.
3. **Goal:** Ensure `design.md` always represents the *actual* system state + immediate committed changes, preventing "documentation drift".

### 3.3 Reminder Protocol
- The Agent SHOULD remind the user to update documentation if:
    - A significant architectural decision was made in chat but not recorded.
    - A plan phase is completed but `design.md` is untouched.

## 4. Maintenance Standards
- **Maintainer:** Both AI Agents and Humans are responsible for keeping these documents in sync.
- **Forbidden:** Do NOT create random documentation files outside this structure without updating the `AGENTS.md` map.

## 5. Language Policy

- **Conversational Responses:** For conversational responses to the user, the agent **MUST** respond in Chinese.
- **Repository Mutations:** For any action that mutates the repository (creating/editing files, writing documentation, modifying code, proposing commit messages or PR descriptions), the agent **MUST** use English.
- **Invariance:** This rule applies regardless of the user's input language.
- **Ambiguity:** If the agent is unsure whether an action constitutes a repository mutation, it **MUST** assume that English is required.
