# Task Instructions

**Scope:** This document provides guidance for executing individual coding tasks.
For process rules, planning, and architectural standards, refer to `AGENTS.md`.

## 1. Interaction Style

- **Be Concise:** Provide brief explanations unless asked for detail.
- **Be Explicit:** When suggesting changes, show the file path and the specific code edit.
- **No Hallucinations:** Do not reference files or code that do not exist without verifying first.

## 2. Coding Standards

(Task-specific rules)

- **Language:** Rust (2021 edition or later)
- **Formatting:** standard `rustfmt`.
- **Error Handling:** Use `thiserror` for libraries, `anyhow` for applications where appropriate, unless specified otherwise.
- **Testing:** Unit tests should be co-located with code; integration tests in `tests/` directory.

## 3. Output Formatting

- **File Edits:** Use strict markdown format for code blocks.
- **Terminal Commands:** ensure commands are compatible with `zsh` on macOS.

## 4. Response Protocol

1. **Analyze:** Understand the immediate task.
2. **Context:** Check `AGENTS.md` to see if a Plan is required.
3. **Execute:** Perform the code changes.
4. **Verify:** Ensure no breaking changes were introduced (run tests if applicable).

---

_Note: This file is for task execution. For project lifecycle, see [AGENTS.md](./AGENTS.md)._
