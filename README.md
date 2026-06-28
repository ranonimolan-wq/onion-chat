# OnionChat

**Tor-supported P2P end-to-end encrypted multi-user chat with file transfer.**

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/Rust-1.96+-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/Tests-281%20passing-brightgreen.svg)](#testing)
[![Platforms](https://img.shields.io/badge/Platforms-Linux%20%7C%20macOS%20%7C%20WSL-lightgrey.svg)](#platform-support)

OnionChat is a terminal-based, anonymity-first chat application built in Rust. It connects peers directly over TCP (optionally tunneled through Tor SOCKS5), encrypts every message with X25519 ECDHE + AES-256-GCM, and supports multi-user star-topology hubs where the hub operator acts as admin.

---

## Table of Contents

- [Why OnionChat?](#why-onionchat)
- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration (`config.json`)](#configuration-configjson)
- [Roles & Permissions](#roles--permissions)
- [Slash Commands](#slash-commands)
- [Markdown & Emoji](#markdown--emoji)
- [Anonymity Model](#anonymity-model)
- [Architecture](#architecture)
- [Testing](#testing)
- [Building from Source](#building-from-source)
- [Tor Hidden Service Setup](#tor-hidden-service-setup)
- [Platform Support](#platform-support)
- [Security Disclosure](#security-disclosure)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgements](#acknowledgements)

---

## Why OnionChat?

Most chat apps trust a central server with your metadata (who talked to whom, when, from which IP). OnionChat is built on a different premise:

- **Anonymity by default.** Peer IP addresses are never displayed. The hub sees a random `peer-<id>` identifier, not a real address.
- **End-to-end encrypted.** Every message is encrypted with ephemeral X25519 keys and AES-256-GCM. The transport (TCP or Tor) never sees plaintext.
- **No phone number, no email, no account.** Just run the binary and connect.
- **Self-hostable.** Anyone can run a hub. The hub operator is the admin and can promote others.
- **Terminal-native.** No browser, no Electron, no telemetry. Just `ratatui` + `crossterm`.
- **Configurable.** Everything (history, roles, anonymity, permissions) lives in one `config.json` file.

OnionChat is for communities that value privacy: journalists, activists, researchers, security teams, and anyone who wants a no-frills encrypted chat that respects their metadata.

---

## Features

### Core
- **E2EE chat** — X25519 ECDHE key exchange + AES-256-GCM authenticated encryption
- **Multi-peer hub** — star topology, hub broadcasts messages to all connected peers
- **Tor support** — connect through Tor SOCKS5 (`--anon` or `--socks5`), or expose a Tor hidden service (`--hidden-service`)
- **File transfer** — chunked v2 protocol, 64 KiB chunks, path-traversal protection, configurable max size
- **SOCKS5 client** — RFC 1928 compliant, IPv4/IPv6, no-auth method

### UX
- **Ratatui TUI** — message history, input box, status bar, scrollable, cursor movement
- **UTF-8 / Turkish support** — full multi-byte character handling (ş, ğ, ü, ö, ç, ı, İ, Ş, Ğ, Ü, Ö, Ç)
- **Markdown rendering** — `*bold*`, `_italic_`, `` `code` ``, `:emoji:` shortcodes (30+ emojis)
- **Slash commands** — `/help`, `/clear [N]`, `/nick`, `/who`, `/role`, `/config`, etc.
- **Message history** — optional, off by default (anonymity), JSON Lines format, configurable path
- **Backspace works** — handles both `\x7f` (DEL) and `\x08` (BS) terminal conventions

### Admin
- **Role-based access control** — Admin / Moderator / User / Guest
- **`/on_admin <nick>`** — promote a peer to admin
- **`/kick`, `/mute`, `/unmute`** — moderate the hub
- **`/config set <key> <value>`** — change settings at runtime, persisted to `config.json`
- **`/clear` broadcast** — when the hub clears the screen, all peers clear too (synced)

### Privacy
- **History default OFF** — no messages are written to disk unless you explicitly enable it
- **`strip_metadata: true`** — peer addresses never shown in TUI
- **`require_tor: false`** (configurable) — force Tor for all connections
- **No telemetry, no analytics, no phone-home** — the binary never contacts any server other than the peer you specify

---

## Quick Start

```bash
# Build
git clone https://github.com/yourname/onionchat.git
cd onionchat
cargo build --release

# Terminal 1: start a hub
./target/release/onionchat --listen 8080 --multi

# Terminal 2: connect as a peer
./target/release/onionchat --connect 127.0.0.1:8080

# Terminal 3: connect another peer
./target/release/onionchat --connect 127.0.0.1:8080
```

Type messages and press Enter. Type `/help` for commands. Press `Esc` to quit.

---

## Installation

### From source (recommended)

```bash
git clone https://github.com/yourname/onionchat.git
cd onionchat
cargo build --release
# Binary: target/release/onionchat
```

### Prerequisites

- **Rust 1.96+** (stable) — install via [rustup](https://rustup.rs)
- **Linux / macOS / WSL** — Windows native not yet tested (use WSL)
- **Tor** (optional) — only needed for `--anon`, `--socks5`, or `--hidden-service`

### Verify the build

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Both should pass with zero warnings.

---

## Usage

### Start a hub (multi-peer)

```bash
# Listen on port 8080, accept multiple peers
onionchat --listen 8080 --multi

# Listen on a specific address
onionchat --listen 127.0.0.1:8080 --multi

# With Tor hidden service (requires Tor running)
onionchat --listen 8080 --multi --hidden-service
```

### Connect to a hub

```bash
# Direct TCP
onionchat --connect 1.2.3.4:8080

# Through Tor (default SOCKS5 at 127.0.0.1:9050)
onionchat --connect abcdef.onion:80 --anon

# Custom SOCKS5 proxy
onionchat --connect 1.2.3.4:8080 --socks5 127.0.0.1:1080

# Send a file (single-peer mode)
onionchat --connect 1.2.3.4:8080 --file ./secret.txt
```

### Address shortcuts

OnionChat accepts short forms for addresses:

| Input | Expanded |
|-------|----------|
| `8080` | `0.0.0.0:8080` |
| `:8080` | `0.0.0.0:8080` |
| `127.0.0.1:8080` | as-is |
| `0.0.0.0:8080` | as-is |

### Default mode (no arguments)

```bash
onionchat
```

Prints a help message. You must specify `--listen` or `--connect`.

---

## Configuration (`config.json`)

OnionChat is driven by a single JSON config file. On first run, a default config is created at `~/.onionchat/config.json`.

```json
{
  "history": {
    "enabled": false,
    "path": "~/.onionchat/history.jsonl",
    "max_messages_loaded": 50
  },
  "server": {
    "max_peers": 100,
    "name": "OnionChat Hub"
  },
  "roles": {
    "enabled": true,
    "first_user_is_admin": false
  },
  "anonymity": {
    "require_tor": false,
    "strip_metadata": true,
    "show_peer_addresses": false
  },
  "permissions": {
    "allow_user_clear": false,
    "allow_user_kick": false,
    "allow_user_mute": false,
    "allow_user_change_config": false
  }
}
```

### Anonymity-first defaults

- `history.enabled: false` — messages are NOT saved to disk by default
- `anonymity.strip_metadata: true` — peer IPs are hidden, shown as `peer-<id>`
- `anonymity.show_peer_addresses: false` — never display real addresses in TUI
- `permissions.*: false` — regular users cannot run admin commands

### Change config at runtime

Inside the hub TUI:

```
/config set history.enabled true
/config set server.max_peers 500
/config set anonymity.show_peer_addresses true
```

Changes are persisted to `config.json` immediately.

### Custom config path

```bash
onionchat --listen 8080 --multi --config /path/to/my-config.json
```

### Config keys reference

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `history.enabled` | bool | `false` | Save messages to disk |
| `history.path` | string | `~/.onionchat/history.jsonl` | History file path |
| `history.max_messages_loaded` | number | `50` | Messages loaded on startup |
| `server.max_peers` | number | `100` | Max concurrent peers |
| `server.name` | string | `"OnionChat Hub"` | Hub display name |
| `roles.enabled` | bool | `true` | Enable role system |
| `roles.first_user_is_admin` | bool | `false` | First peer auto-admin |
| `anonymity.require_tor` | bool | `false` | Force Tor for all connections |
| `anonymity.strip_metadata` | bool | `true` | Hide peer addresses |
| `anonymity.show_peer_addresses` | bool | `false` | Show real IPs (debug only) |
| `permissions.allow_user_clear` | bool | `false` | Users can `/clear` |
| `permissions.allow_user_kick` | bool | `false` | Users can `/kick` |
| `permissions.allow_user_mute` | bool | `false` | Users can `/mute` |
| `permissions.allow_user_change_config` | bool | `false` | Users can `/config set` |

---

## Roles & Permissions

| Role | Description | How to get |
|------|-------------|------------|
| **Admin** | Full access: kick, mute, config, promote others | Start the hub, or `/on_admin` by an admin |
| **Moderator** | Kick, mute, clear | (Future: `/mod` by admin) |
| **User** | Default. Chat, nick, who, role | Connect to hub |
| **Guest** | Read-only (placeholder) | (Future) |

### Permission matrix

| Command | Admin | Moderator | User | Guest |
|---------|:-----:|:---------:|:----:|:-----:|
| `/clear [N]` | ✅ | ✅ | config | ❌ |
| `/kick` | ✅ | ✅ | config | ❌ |
| `/mute` | ✅ | ✅ | config | ❌ |
| `/on_admin` | ✅ | ❌ | ❌ | ❌ |
| `/off_admin` | ✅ | ❌ | ❌ | ❌ |
| `/config set` | ✅ | ❌ | config | ❌ |
| `/config` (view) | ✅ | ✅ | ✅ | ✅ |
| `/role` | ✅ | ✅ | ✅ | ✅ |
| `/who` | ✅ | ✅ | ✅ | ✅ |

---

## Slash Commands

### Basic
```
/help                    Show this help
/quit                    Quit (Esc also works)
/clear [N]               Clear screen (N = last N messages)
/nick <name>             Set your nickname
/who                     List connected peers + roles
/role [nick]             Show role (yours or someone's)
/send <path>             Send a file (single-peer mode)
```

### Admin / Moderator
```
/on_admin <nick>         Promote to admin (admin only)
/off_admin <nick>        Demote from admin (admin only)
/kick <nick>             Kick a peer (admin/mod)
/mute <nick>             Mute a peer (admin/mod)
/unmute <nick>           Unmute a peer (admin/mod)
/config                  Show current config
/config set <key> <val>  Change a config value (admin only)
```

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Esc` / `Ctrl+C` | Quit |
| `↑` / `↓` | Scroll history |
| `←` / `→` | Move cursor |
| `Backspace` | Delete char before cursor |
| `Home` / `End` | Jump to start / end of input |

---

## Markdown & Emoji

OnionChat renders inline markdown in messages:

| Syntax | Result |
|--------|--------|
| `*bold*` | **bold** |
| `_italic_` | *italic* |
| `` `code` `` | `code` (yellow) |
| `:smile:` | 🙂 |
| `:heart:` | ❤️ |
| `:onion:` | 🧅 |
| `:rocket:` | 🚀 |

### Supported emojis

`:smile:` `:laugh:` `:heart:` `:thumbsup:` `:thumbsdown:` `:ok:` `:wave:` `:fire:` `:star:` `:check:` `:x:` `:warning:` `:info:` `:rocket:` `:onion:` `:lock:` `:key:` `:ghost:` `:skull:` `:party:` `:coffee:` `:beer:` `:pizza:` `:cat:` `:dog:` `:thinking:` `:shrug:` `:tada:`

### Turkish character support

Full UTF-8 support. All Turkish characters work in messages, nicks, file paths, and commands: `ş ğ ü ö ç ı İ Ş Ğ Ü Ö Ç`.

---

## Anonymity Model

### What OnionChat protects

- **Message content** — E2EE with AES-256-GCM. Even the hub cannot read messages between two peers (in single-peer mode). In hub mode, the hub sees plaintext (it broadcasts), but the transport is still encrypted.
- **Peer identity** — peers are identified by random `peer-<id>`, not IP. The hub operator can see the connecting IP (for troubleshooting), but it's hidden by default (`strip_metadata: true`).
- **Metadata on disk** — history is OFF by default. No logs, no databases, no analytics.

### What OnionChat does NOT protect

- **The hub operator sees your IP** — when you connect to a hub, the hub's `TcpListener::accept` sees your real source address. Use Tor (`--anon` or `--socks5 127.0.0.1:9050`) to hide your IP from the hub.
- **Timing analysis** — OnionChat does not delay or batch messages. An observer correlating traffic timing can infer who is talking.
- **Hub operator trust** — in hub mode, the hub sees all messages (it must, to broadcast). For truly private 1:1 chat, use single-peer mode (`--connect` without `--multi`).

### Tor integration

```bash
# As a peer: route your connection through Tor
onionchat --connect abcdef.onion:80 --anon

# As a hub: expose a Tor hidden service
onionchat --listen 8080 --multi --hidden-service
```

For the hidden service, Tor must be running with `ControlPort 9051` and `CookieAuthentication 1` in your `torrc`.

---

## Architecture

OnionChat is modular. Each module has a single responsibility.

```
onionchat/
├── src/
│   ├── main.rs            # CLI entry point, arg parsing
│   ├── crypto.rs          # X25519 ECDH + AES-256-GCM
│   ├── network.rs         # TCP transport, ECDHE handshake, length-prefixed framing
│   ├── socks5.rs          # RFC 1928 SOCKS5 client (for Tor)
│   ├── tor_control.rs     # Tor control port (ADD_ONION for hidden services)
│   ├── chat.rs            # Chat message protocol ([0x01][utf-8], [0x03] clear)
│   ├── file_transfer.rs   # Chunked v2 file transfer protocol
│   ├── ui.rs              # ratatui TUI (hub + single-peer)
│   ├── config.rs          # config.json load/save/set
│   ├── roles.rs           # Role enum, PeerInfo, PeerRegistry
│   ├── commands.rs        # Slash command parser
│   ├── history.rs         # JSON Lines message persistence
│   └── markdown.rs        # Inline markdown + emoji rendering
├── wiki/                  # Architectural documentation (read this!)
├── memory-bank/           # Active context for AI sessions
├── scripts/               # E2E test scripts (Python)
├── Cargo.toml
└── README.md
```

### Module boundaries (strict!)

Module boundaries are **absolute**. Each module has a single responsibility and must not be merged with another. See [`readme-ai-ai-rules.md`](readme-ai-ai-rules.md) for the full AI guidelines.

### Protocol

```
Client (connect)                          Server (listen)
─────────────────                          ────────────────
TCP connection ───────────────────────────►
pub_key[32] send ──────────────────────►
                    pub_key[32] read ◄──── pub_key[32] send
pub_key[32] read ◄────────────────────────
AES key derived (both sides)
Connection ready
send_message(text) ──[len|nonce|ct]─────►
```

Every message is encrypted with AES-256-GCM and framed as `[len: u32 BE][nonce|ciphertext]`.

---

## Testing

OnionChat has **281 unit tests** and **multiple E2E test suites**.

### Unit tests

```bash
cargo test
```

Covers: crypto round-trips, network framing, chunked file transfer, SOCKS5 handshake, chat protocol, UTF-8/Turkish chars, markdown rendering, command parsing, config load/save, role permissions, history persistence, UI state (cursor, backspace, scroll).

### Clippy (zero warnings policy)

```bash
cargo clippy --all-targets -- -D warnings
```

### E2E tests (Python, PTY-based)

```bash
# Multi-peer chat test
python3 scripts/test_chat_multi_e2e.py

# Turkish character test
python3 scripts/test_turkish_chars_e2e.py

# Config + roles test
python3 scripts/test_config_roles_e2e.py

# Bug fix test (backspace + clear broadcast)
python3 scripts/test_bugfix_e2e.py
```

These tests spawn real `onionchat` processes via PTY, send keystrokes, and verify the TUI output.

---

## Building from Source

### Requirements

- Rust 1.96+ (stable)
- Linux, macOS, or WSL
- (Optional) Tor for anonymity features

### Build

```bash
git clone https://github.com/yourname/onionchat.git
cd onionchat
cargo build --release
```

Binary: `target/release/onionchat`

### Run tests

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

### Cross-compile (optional)

```bash
# For ARM64 (e.g., Raspberry Pi)
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

---

## Tor Hidden Service Setup

### 1. Install Tor

```bash
# Debian/Ubuntu
sudo apt install tor

# macOS
brew install tor
```

### 2. Configure Tor

Edit `/etc/tor/torrc` (or `~/.torrc`):

```
ControlPort 9051
CookieAuthentication 1
SocksPort 9050
```

Restart Tor:

```bash
sudo systemctl restart tor
```

### 3. Start OnionChat with hidden service

```bash
onionchat --listen 8080 --multi --hidden-service
```

OnionChat will print an onion address like `abcdef1234567890.onion`. Peers connect with:

```bash
onionchat --connect abcdef1234567890.onion:80 --anon
```

### Custom Tor control port / cookie

```bash
onionchat --listen 8080 --multi --hidden-service \
          --tor-control 127.0.0.1:9051 \
          --tor-cookie /var/run/tor/control.authcookie
```

---

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux | ✅ Fully supported | Primary development platform |
| macOS | ✅ Should work | Not extensively tested, please report issues |
| WSL | ✅ Works | Windows Terminal recommended |
| Windows native | ❌ Not tested | Use WSL |
| BSD | ❓ Unknown | Should work, untested |

---

## Security Disclosure

**Do not open public GitHub issues for security vulnerabilities.**

If you discover a security issue:

1. Email the maintainer directly (see GitHub profile)
2. Include a clear description and proof of concept
3. Allow 90 days for a fix before public disclosure

### Security properties

- **Encryption**: X25519 ECDHE (ephemeral keys) + AES-256-GCM (authenticated)
- **No unsafe code**: The entire codebase is `unsafe`-free (Rule 1 of `readme-ai-ai-rules.md`)
- **Memory safety**: Rust's ownership model prevents buffer overflows, use-after-free, etc.
- **Nonce uniqueness**: Each AES-GCM encryption uses a random 12-byte nonce from `OsRng`

### Known limitations

- Hub mode is not E2EE between peers (the hub decrypts to broadcast)
- No forward secrecy beyond the ephemeral ECDHE (session key is reused for the connection lifetime)
- No message integrity verification across reconnects (future: SHA-256 manifest)

---

## Contributing

OnionChat is **open source** and welcomes contributions! See [`CONTRIBUTING.md`](CONTRIBUTING.md) for:

- Pull request process
- Code style guidelines
- AI-assisted development workflow (with ready-to-use prompts)
- Wiki and memory-bank maintenance
- Testing requirements

### Quick contribution ideas

- **Windows native support** — test and fix `crossterm` issues on Windows
- **Mesh topology** — replace star topology with DHT-based mesh
- **Message search** — `/search <query>` in history
- **File transfer in TUI** — `/send` from hub mode
- **Nick persistence** — save nicknames in config
- **Sound notifications** — bell on new message

---

## License

OnionChat is licensed under the **GNU General Public License v3.0**.

```
SPDX-License-Identifier: GPL-3.0-only
Copyright (c) 2024 OnionChat Developers. All rights reserved.
```

See [`LICENSE`](LICENSE) for the full text.

---

## Acknowledgements

OnionChat builds on the work of many open-source projects:

- [Rust](https://www.rust-lang.org/) — language
- [Tokio](https://tokio.rs/) — async runtime
- [ratatui](https://ratatui.rs/) — terminal UI
- [crossterm](https://github.com/crossterm-rs/crossterm) — terminal abstraction
- [x25519-dalek](https://github.com/dalek-cryptography/curve25519-dalek) — ECDH
- [aes-gcm](https://github.com/RustCrypto/AEADs) — authenticated encryption
- [clap](https://docs.rs/clap/) — CLI parsing
- [serde](https://serde.rs/) — serialization
- [Tor Project](https://www.torproject.org/) — anonymity network

### Derived from

OnionChat's AI constitutional guidelines are derived from the Aeon Engine AI Constitutional Guidelines (v2024), adapted for this project. See [`readme-ai-ai-rules.md`](readme-ai-ai-rules.md).

---

<p align="center">
  <strong>Anonymity is not a feature. It's the foundation.</strong>
</p>

<p align="center">
  Made with 🧅 by the OnionChat community
</p>
