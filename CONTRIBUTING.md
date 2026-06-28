# Contributing to OnionChat

First of all, **thank you** for considering a contribution! OnionChat is a community-driven project, and every contribution — whether it's a bug fix, a new feature, better docs, or just reporting an issue — makes the project stronger.

This document explains how to contribute, whether you're a human or an AI-assisted developer.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Project Philosophy](#project-philosophy)
- [Before You Start](#before-you-start)
- [Pull Request Process](#pull-request-process)
- [Code Style & Standards](#code-style--standards)
- [AI-Assisted Development](#ai-assisted-development)
  - [Mandatory: Read `readme-ai-ai-rules.md` First](#mandatory-read-readme-ai-ai-rulesmd-first)
  - [Ready-to-Use AI Prompts](#ready-to-use-ai-prompts)
  - [Wiki Maintenance](#wiki-maintenance)
  - [Memory-Bank Maintenance](#memory-bank-maintenance)
- [Testing Requirements](#testing-requirements)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)
- [License](#license)

---

## Code of Conduct

Be kind. Be patient. Assume good intent.

- **Respect privacy** — this is a privacy project. Don't dox contributors, don't share private conversations.
- **No harassment** — racist, sexist, homophobic, transphobic, or otherwise exclusionary behavior will not be tolerated.
- **Help newcomers** — everyone starts somewhere. If someone asks a "basic" question, answer it kindly or point them to docs.
- **Disagree on ideas, not people** — technical disagreement is healthy; personal attacks are not.

Violations: email the maintainer. Repeat offenders will be banned.

---

## Project Philosophy

OnionChat is built on these non-negotiable principles:

1. **Anonymity first.** Any feature that compromises user anonymity will be rejected. History is OFF by default. Peer addresses are hidden. Tor is a first-class citizen.
2. **No unsafe code.** The entire codebase is `unsafe`-free. If you think you need `unsafe`, find a safe alternative. (See `readme-ai-ai-rules.md` Rule 1.)
3. **Modular boundaries are absolute.** Each module has one responsibility. Do not merge modules or move logic between them unless explicitly approved. (Rule 7.)
4. **Documentation is mandatory.** Every new `pub` item gets a `///` doc comment. Every feature gets a wiki page. Every bug fix gets a wiki entry. (Rules 2 & 3.)
5. **Zero warnings.** `cargo check` and `cargo clippy -- -D warnings` must pass clean. Always.

If your contribution violates any of these, it will not be merged. This is not gatekeeping — it's how we keep the project trustworthy.

---

## Before You Start

### 1. Check existing issues

Before starting work, check [the issue tracker](https://github.com/yourname/onionchat/issues). Someone may already be working on what you have in mind. If not, open an issue describing what you want to do.

### 2. Fork & clone

```bash
# Fork on GitHub, then:
git clone https://github.com/YOUR_USERNAME/onionchat.git
cd onionchat
git remote add upstream https://github.com/yourname/onionchat.git
```

### 3. Create a branch

```bash
git checkout -b feature/my-awesome-feature
# or
git checkout -b fix/issue-123-backspace-bug
```

Branch naming:
- `feature/<short-description>` — new features
- `fix/<issue-number>-<short-description>` — bug fixes
- `docs/<short-description>` — documentation only
- `refactor/<short-description>` — code refactoring (no behavior change)

### 4. Build & test

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

All three must pass before you open a PR.

---

## Pull Request Process

### 1. Write tests

Every PR must include tests. No exceptions.

- **Bug fix**: add a test that fails before your fix and passes after.
- **New feature**: add unit tests for the new logic.
- **Refactor**: existing tests should still pass (if they don't, you changed behavior — that's a feature, not a refactor).

### 2. Update the wiki

If your PR adds a feature or fixes a bug, you **must** update the relevant wiki page(s). This is not optional (Rule 3 of `readme-ai-ai-rules.md`).

- New feature → add a page under `wiki/concepts/` or update an existing one.
- Bug fix → add an entry to `wiki/history/bug_fixes_YYYY-MM-DD.md`.
- Update `wiki/index.md` and `wiki/log.md` to reflect your changes.

### 3. Update `memory-bank/activeContext.md`

If you're an AI-assisted contributor (or even if you're not), update `memory-bank/activeContext.md` with:
- What you changed
- Why you changed it
- What's next

This keeps the next contributor (human or AI) in sync.

### 4. Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(chat): add /mute command for hub moderators
fix(ui): handle \x7f DEL character as backspace
docs(wiki): update architecture page for v0.3
refactor(config): derive Default instead of manual impl
test(chat): add Turkish character round-trip tests
```

### 5. Open the PR

```bash
git push origin feature/my-awesome-feature
```

Then open a PR on GitHub. The PR template will ask for:

- **What does this PR do?** — one paragraph summary
- **Why is this change needed?** — link to issue or explain motivation
- **How was it tested?** — list the tests you ran
- **Wiki updated?** — yes/no, which pages
- **Breaking changes?** — yes/no, what breaks

### 6. Code review

A maintainer will review your PR. Expect feedback. This is normal and good — it makes the code better.

**Review criteria:**
- Does it pass `cargo test` and `cargo clippy -- -D warnings`?
- Does it respect module boundaries (Rule 7)?
- Is it documented (Rule 2)?
- Is the wiki updated (Rule 3)?
- Does it compromise anonymity? (If yes, rejected.)
- Does it use `unsafe`? (If yes, rejected.)

### 7. Merge

Once approved, a maintainer will merge your PR. Congratulations — you're now a contributor! 🎉

---

## Code Style & Standards

### Rust style

- **Edition 2024** — use modern Rust features (let-chains, etc.).
- **`clippy` clean** — zero warnings with `-D warnings`.
- **No `unsafe`** — ever. Find a safe alternative.
- **Functions < 40 lines** — if longer, split it (Rule 6).
- **Doc comments** — every `pub` item gets `///` (Rule 2).
- **Bilingual comments OK** — Turkish/English mix is fine, but be technically clear.

### Module rules

- **One responsibility per module** (Rule 4).
- **No "God Objects"** (Rule 4).
- **Don't move logic between modules** without approval (Rule 7).
- **`main.rs` stays thin** — delegate to modules (Rule 4).

### Naming

- Files: `snake_case.rs`
- Types: `PascalCase`
- Functions/variables: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Wiki files: `kebab-case.md`

### Error handling

- Use `anyhow::Result` for application code.
- Use `thiserror` for library-level error types (if added).
- Never `unwrap()` in production code — use `?` or handle explicitly.
- `unwrap()` is OK in tests.

---

## AI-Assisted Development

OnionChat is **AI-friendly**. The project was largely built with AI assistance, and we actively support AI-assisted contributors. But there are rules.

### Mandatory: Read `readme-ai-ai-rules.md` First

Before doing anything with an AI assistant, both you and the AI **must** read [`readme-ai-ai-rules.md`](readme-ai-ai-rules.md). This file contains 7 rules that are non-negotiable:

1. **Zero Unsafe Policy** — no `unsafe` blocks.
2. **Mandatory Documentation** — `///` on every `pub` item.
3. **Persistent Knowledge Base** — update `wiki/` after every change.
4. **Modular Responsibility** — one responsibility per module.
5. **Verification Standards** — `cargo check` with zero errors and zero warnings.
6. **Vibe Coding Principles** — small iterations, frequent `cargo check`.
7. **Strict File Boundary Enforcement** — don't merge or move modules.

If your AI assistant hasn't read this file, stop and make it read it. Seriously.

### Ready-to-Use AI Prompts

Here are prompts you can paste into your AI assistant (Claude, GPT, Gemini, etc.) to contribute effectively.

---

#### Prompt 1: Starting a new feature

```
I'm working on OnionChat, a Rust P2P E2EE chat app.

IMPORTANT: Before doing anything, read these files in order:
1. readme-ai-ai-rules.md — the 7 rules you MUST follow
2. wiki/index.md — to find relevant pages
3. wiki/concepts/architecture.md — to understand the module structure
4. memory-bank/activeContext.md — to see what's been done recently

I want to add: [describe your feature here]

Rules:
- NO unsafe code (Rule 1)
- Every pub item gets /// doc comments (Rule 2)
- Update wiki/ after completing (Rule 3)
- Keep main.rs thin, respect module boundaries (Rules 4 & 7)
- Run cargo check + cargo clippy -- -D warnings + cargo test (Rule 5)
- Small iterations, frequent cargo check (Rule 6)

Start by reading the files above, then propose a plan. Don't write code yet.
```

---

#### Prompt 2: Fixing a bug

```
I'm working on OnionChat (Rust P2P E2EE chat).

Before starting, read:
1. readme-ai-ai-rules.md — the 7 rules
2. wiki/history/ — check if this bug was fixed before
3. wiki/concepts/architecture.md — module structure

Bug description: [describe the bug]

Reproduction:
1. [step 1]
2. [step 2]
Expected: [what should happen]
Actual: [what actually happens]

Rules:
- NO unsafe code
- Add a test that fails before the fix, passes after
- Update wiki/history/bug_fixes_YYYY-MM-DD.md with the fix
- Run cargo check + clippy + test (all must pass)
- Respect module boundaries — fix in the right module

Find the root cause, propose a fix, then implement it.
```

---

#### Prompt 3: After completing work (wiki + memory-bank update)

```
I just finished working on OnionChat. Here's what I did:
[summary of changes]

Now I need to update the wiki and memory-bank. Please:

1. Read wiki/index.md and wiki/log.md to see current state
2. Create or update the relevant wiki page(s):
   - If new feature: create wiki/concepts/<feature-name>.md
   - If bug fix: add entry to wiki/history/bug_fixes_YYYY-MM-DD.md
   - Update wiki/concepts/architecture.md if module structure changed
3. Update wiki/index.md with the new page(s)
4. Append to wiki/log.md with today's date and what changed
5. Update memory-bank/activeContext.md:
   - Current Focus
   - Recent Changes
   - Next Steps

Use the format from existing wiki pages (YAML frontmatter, [[wikilinks]], etc.).
```

---

#### Prompt 4: Code review with AI

```
Review this Rust code for OnionChat.

Check against readme-ai-ai-rules.md:
1. Any unsafe blocks? (must be zero)
2. Are all pub items documented with ///?
3. Does it respect module boundaries?
4. Will cargo clippy -- -D warnings pass?
5. Are there tests?

Also check:
- Error handling (no unwrap in prod, use ?)
- Function length (< 40 lines ideally)
- Naming conventions (snake_case for functions, PascalCase for types)
- Anonymity: does this leak peer addresses or metadata?

Code:
[paste code here]
```

---

#### Prompt 5: Adding tests

```
I need to add tests for [feature/module] in OnionChat.

Read:
1. readme-ai-ai-rules.md (Rule 5: verification standards)
2. src/[module].rs — the existing code and tests

Add tests for:
- [case 1]
- [case 2]
- Edge cases: [list them]

Requirements:
- Use #[test] for unit tests, #[tokio::test] for async
- Test names: snake_case, descriptive (e.g., backspace_turkish_char_2_bytes)
- Each test should verify ONE behavior
- Include Turkish character tests where relevant (ş ğ ü ö ç ı İ)
- Run cargo test — all must pass
- Run cargo clippy --all-targets -- -D warnings — zero warnings
```

---

### Wiki Maintenance

The `wiki/` directory is OnionChat's **persistent knowledge base**. It's how the project maintains continuity across contributors and AI sessions.

#### Structure

```
wiki/
├── SCHEMA.md              # Wiki conventions, tag taxonomy
├── index.md               # Page catalog (read this first!)
├── log.md                 # Chronological action log (append-only)
├── entities/              # People, organizations, concepts
│   └── onion-chat.md
├── concepts/              # How things work
│   ├── architecture.md
│   ├── chat-tui.md
│   ├── config-roles.md
│   ├── turkish-support.md
│   └── ...
├── comparisons/           # Side-by-side analyses (future)
├── queries/               # Open questions (future)
├── history/               # Bug fixes, change logs
│   ├── bug_fixes.md
│   └── bug_fixes_2026-06-28.md
└── raw/                   # Source materials (future)
```

#### Rules (from SCHEMA.md)

- **File names**: lowercase, hyphens, no spaces
- **Every page**: starts with YAML frontmatter (title, created, updated, type, tags, sources)
- **Use `[[wikilinks]]`** to link between pages (minimum 2 outbound links per page)
- **Update `index.md`** when adding a page
- **Append to `log.md`** for every action
- **Bump `updated` date** when modifying a page

#### When to update the wiki

| Change type | Action |
|-------------|--------|
| New feature | Create `wiki/concepts/<feature>.md` + update `index.md` + append `log.md` |
| Bug fix | Add entry to `wiki/history/bug_fixes_<date>.md` + append `log.md` |
| Refactor | Update affected `wiki/concepts/` pages + append `log.md` |
| New module | Add to `wiki/concepts/architecture.md` + create dedicated page + update `index.md` |
| Config change | Update `wiki/concepts/config-roles.md` + append `log.md` |

### Memory-Bank Maintenance

`memory-bank/activeContext.md` is the **session handoff document**. It tells the next contributor (human or AI) exactly where things stand.

#### Update it after every work session:

```markdown
# Active Context

- **Current Focus**: [what you're working on right now]
- **Recent Changes**:
  - [change 1]
  - [change 2]
- **Tooling**: [any new tools, scripts, or setup notes]
- **Next Steps**:
  1. [next thing to do]
  2. [the thing after that]
- **Open Questions**:
  - [unresolved question 1]
  - [unresolved question 2]
```

This is critical for AI-assisted development — the AI reads this file first to get context before reading code.

---

## Testing Requirements

### Unit tests

```bash
cargo test
```

Currently 281 tests. Every PR must maintain or increase this number.

### Clippy

```bash
cargo clippy --all-targets -- -D warnings
```

Zero warnings. Always.

### E2E tests (if applicable)

If your change affects the TUI or network behavior, run the E2E tests:

```bash
python3 scripts/test_chat_multi_e2e.py
python3 scripts/test_turkish_chars_e2e.py
python3 scripts/test_config_roles_e2e.py
python3 scripts/test_bugfix_e2e.py
```

### Test naming

- `fn test_<what_it_tests>` — descriptive, snake_case
- `fn backspace_turkish_char_2_bytes` — good
- `fn test1` — bad

### Test organization

- Tests live in the same file as the code, in a `#[cfg(test)] mod tests` block.
- E2E tests live in `scripts/` as Python files.

---

## Reporting Bugs

### Before reporting

1. Search [existing issues](https://github.com/yourname/onionchat/issues) — someone may have reported it.
2. Try to reproduce with the latest `main` branch.
3. Run with `RUST_LOG=debug` to get more output.

### Bug report template

```markdown
**Bug description**
[Clear description of the bug]

**To reproduce**
1. Start hub: `onionchat --listen 8080 --multi`
2. Connect peer: `onionchat --connect 127.0.0.1:8080`
3. Type: [what you typed]
4. See error: [what happened]

**Expected behavior**
[What you expected to happen]

**Actual behavior**
[What actually happened]

**Environment**
- OS: [e.g., Ubuntu 22.04]
- Rust version: [rustc --version]
- OnionChat version: [git commit hash or tag]
- Terminal: [e.g., gnome-terminal, kitty, iTerm2]

**Logs**
```
[paste RUST_LOG=debug output here]
```
```

### Security bugs

**Do NOT open a public issue for security bugs.** Email the maintainer directly. See [README.md → Security Disclosure](README.md#security-disclosure).

---

## Suggesting Features

We love feature ideas! But please:

1. **Check the philosophy** — does it compromise anonymity? If yes, it won't be accepted.
2. **Open an issue first** — describe the feature, why it's useful, and how you'd implement it. Get feedback before coding.
3. **Be patient** — maintainers are volunteers.

### Feature ideas we'd love to see

- **Mesh topology** — replace star topology with DHT-based mesh (no central hub)
- **Message search** — `/search <query>` in history
- **File transfer in TUI** — `/send` from hub mode
- **Nick persistence** — save nicknames across sessions
- **Sound notifications** — terminal bell on new message
- **Windows native support** — test and fix crossterm on Windows
- **i18n** — translate UI messages (currently Turkish/English mix)
- **Tor arti-client integration** — native Tor instead of SOCKS5
- **Message signing** — Ed25519 signatures for sender authentication
- **Forward secrecy** — rotate session keys periodically

---

## License

By contributing to OnionChat, you agree that your contributions will be licensed under the [GPL-3.0 License](LICENSE). You retain copyright to your contributions, but they become part of a GPL-3.0 project.

If you add new files, include the SPDX header:

```rust
// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.
```

---

## Questions?

- **Open an issue** with the `question` label
- **Read the wiki** — especially `wiki/concepts/architecture.md`
- **Read `readme-ai-ai-rules.md`** — it explains the project's DNA

---

<p align="center">
  <strong>Build privacy. Build community. Build OnionChat.</strong> 🧅
</p>
