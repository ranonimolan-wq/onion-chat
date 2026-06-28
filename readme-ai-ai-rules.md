# AI Guidelines for ShadowShare Development
>
> [!IMPORTANT]
> All AI assistants contributing to this repository MUST read and adhere to these guidelines before proposing or executing any code changes.
>
## 1. Zero Unsafe Policy
Memory safety is the highest priority in ShadowShare.
- **Rule**: Do NOT use `unsafe` blocks under any circumstances.
- **Rationale**: We prioritize safety and stability over micro-optimizations that require bypassing Rust's borrow checker. If a task seems to require `unsafe`, find a safe alternative using higher-level abstractions or libraries.
>
## 2. Mandatory Documentation (///)
All new code must be self-documenting for both humans and future AI contexts.
- **Rule**: Every new `pub mod`, `pub struct`, `pub enum`, and `pub fn` MUST include triple-slash (`///`) doc-comments.
- **Format**: Turkish/English bilingual comments are allowed, but the technical explanation must be clear and technically descriptive.
>
## 3. Persistent Knowledge Base (Wiki)
To maintain architectural continuity across different sessions and AI assistants:
- **Rule**: All new features and system improvements (small/large) must be immediately added to the wiki under `wiki/entities/` or `wiki/concepts/` as appropriate.
- **Rule**: All bug fixes, crash resolutions, stability patches, and corrections must be immediately added to the wiki under `wiki/history/bug_fixes.md` (create if not exists).
- **Rule**: This is NOT a suggestion, it's a REQUIREMENT! After completing a task, you must update the relevant wiki page(s) according to the type of change; otherwise, you have violated the rule.
- **Rule**: For every significant feature or refactor, a corresponding `.md` file must be created or updated in the `wiki/` directory.
- **Rule**: If you modify existing code structures, you MUST update the related documentation in the `wiki/` folder to reflect the changes immediately.
- **Rule**: Before starting work, the AI must search and read relevant files in the `wiki/` directory (especially those inside `wiki/history/`) to understand the existing logic, patterns, and resolved issues.
>
## 4. Modular Responsibility
ShadowShare follows a strict modular architecture.
- **Rule**: Keep `main.rs` thin.
- **Rule**: Delegate logic to specialized modules (e.g., `crypto.rs`, `network.rs`, `file_transfer.rs`, `ui.rs`).
- **Rule**: Do not create "God Objects" that manage multiple disconnected responsibilities.
>
## 5. Verification Standards
- **Rule**: Always run `cargo check` after any structural change to ensure zero errors and zero warnings.
- **Rule**: Avoid placeholders. If an asset is needed, generate or use a real representative file.
>
## 6. Vibe Coding Principles
We embrace rapid, iterative development while maintaining quality.
- **Rule**: Start with a working prototype, then refine through successive iterations.
- **Rule**: Use temporal coupling: write code that works now, improve it later.
- **Rule**: Focus on getting immediate feedback; compile frequently (`cargo check`).
- **Rule**: Keep functions small and focused; aim for < 40 lines per function when possible.
- **Rule**: Embrace change: refactor mercilessly when you learn more, but always keep tests passing (if any) and documentation updated.
>
## 7. Strict File Boundary Enforcement
- **Rule**: Existing module boundaries are ABSOLUTE and MUST NOT be changed.
- **Rule**: DO NOT merge files, collapse modules, or move logic between modules unless explicitly instructed.
- **Rule**: Each module has a SINGLE responsibility and must remain isolated.
- **Rule**: If a task requires changes across multiple modules, modify them individually — NEVER combine them.
- **Rule**: If you must create a new module, ensure it has a clear, single purpose and follows the same documentation and safety standards.
>
---
*Derived from Aeon Engine AI Constitutional Guidelines (v2026) and adapted for ShadowShare.*