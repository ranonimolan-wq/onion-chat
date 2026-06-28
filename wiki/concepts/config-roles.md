---
title: Config & Roles System (v0.3)
created: 2026-06-28
updated: 2026-06-28
type: concept
tags: [config, roles, permissions, anonymity, rbac, json, rust-2024]
sources: [src/config.rs, src/roles.rs, src/commands.rs, src/ui.rs, src/main.rs]
confidence: high
---

# Config & Roles System (v0.3)

v0.3, OnionChat'ı topluluk odaklı bir anonim sohbet platformuna
dönüştürür. Merkezi `config.json`, rol-tabanlı erişim kontrolü (RBAC),
ve anonimlik koruması eklenir.

## Mimari

```
config.json (merkezi ayar)
    ↓
[load_or_create] → Config struct
    ↓
┌─────────────────────────────────────────────┐
│  Hub TUI                                    │
│  ├─ PeerRegistry (id, nick, role, muted)    │  ← roles.rs
│  ├─ Config (runtime ayarlar)                │  ← config.rs
│  └─ Komutlar (rol bazlı):                   │  ← commands.rs
│      /clear [N], /on_admin, /off_admin      │
│      /kick, /mute, /unmute, /role           │
│      /config, /config set <key> <value>     │
└─────────────────────────────────────────────┘
```

## config.json Yapısı

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

### Anonymity-First Defaults

- **history.enabled: false** — mesajlar kaydedilmez (anonymity)
- **anonymity.strip_metadata: true** — peer IP'leri gizli
- **anonymity.show_peer_addresses: false** — TUI'da `peer-<id>` gösterilir
- **permissions.*: false** — user rolü yönetim komutları kullanamaz

### Config Yolu Önceliği

1. `--config <path>` CLI bayrağı
2. `~/.onionchat/config.json` (default)
3. `./onionchat-config.json` (HOME yoksa fallback)

İlk başlatmada default config oluşturulur. `~` otomatik genişletilir.

## Rol Sistemi

```
Admin (hub operator) ← her zaman admin
  ↓ /on_admin <nick>
Admin (promoted peer)
  ↓ /off_admin <nick>
User (default peer)
  ↓ (ileride /mod)
Moderator
  ↓ (ileride)
Guest (read-only, placeholder)
```

### İzin Matrisi

| Komut | Admin | Moderator | User | Guest |
|-------|-------|-----------|------|-------|
| `/clear [N]` | ✓ | ✓ | config | ✗ |
| `/kick` | ✓ | ✓ | config | ✗ |
| `/mute` | ✓ | ✓ | config | ✗ |
| `/on_admin` | ✓ | ✗ | ✗ | ✗ |
| `/off_admin` | ✓ | ✗ | ✗ | ✗ |
| `/config set` | ✓ | ✗ | config | ✗ |
| `/config` (görüntüle) | ✓ | ✓ | ✓ | ✓ |
| `/role` | ✓ | ✓ | ✓ | ✓ |
| `/who` | ✓ | ✓ | ✓ | ✓ |

`config` = config.json'dan `permissions.allow_user_*` ile açılabilir.

## Anonimlik Modeli

Peer'lar gerçek IP yerine `peer-<id>` ile gösterilir:

```rust
pub fn display_name(&self, show_addr: bool) -> String {
    if let Some(nick) = &self.nick {
        nick.clone()
    } else if show_addr {
        format!("{}", self.addr)  // sadece debug için
    } else {
        format!("peer-{:x}", self.id)  // default anonim
    }
}
```

- Peer ID: artan sayaç (1, 2, 3, ...) — IP ifşa etmez
- Nick: `/nick` ile ayarlanır, opsiyonel
- `anonymity.show_peer_addresses: false` → IP asla gösterilmez
- Hub log'ları da `peer-<id>` kullanır

## Komutlar (v0.3)

### Temel
- `/help` — yardım
- `/quit` — çık
- `/clear` — tüm mesajları sil
- `/clear <N>` — son N mesajı sil
- `/nick <name>` — takma ad
- `/who` — peer listesi + roller
- `/role [nick]` — rol göster
- `/send <path>` — dosya gönder

### Yönetim
- `/on_admin <nick>` — admin ver (admin only)
- `/off_admin <nick>` — admin al (admin only)
- `/kick <nick>` — peer at (admin/mod)
- `/mute <nick>` — sustur (admin/mod)
- `/unmute <nick>` — susturmayı kaldır
- `/config` — config göster
- `/config set <key> <value>` — config değiştir (admin only)

## Config Set Anahtarları

```
history.enabled             bool
history.path                string
history.max_messages_loaded number
server.max_peers            number
server.name                 string
roles.enabled               bool
roles.first_user_is_admin   bool
anonymity.require_tor       bool
anonymity.strip_metadata    bool
anonymity.show_peer_addresses bool
permissions.allow_user_clear      bool
permissions.allow_user_kick       bool
permissions.allow_user_mute       bool
permissions.allow_user_change_config bool
```

## Test Kapsamı

### Unit Tests (94 yeni)

**`config.rs`** (35 test):
- Default değerler (history=false, strip_metadata=true)
- `parse_bool` varyantları (true/false/1/0/yes/no/evet/hayır)
- `expand_tilde` (~/, ~, /abs, relative)
- `save`/`load` round-trip
- `load_or_create` (oluştur + mevcut yükle)
- Partial config (eksik alanlar default)
- `set_field` tüm anahtarlar + hata durumları
- `to_pretty_json`
- Serde round-trip

**`roles.rs`** (35 test):
- `Role::as_str`/`from_str`/`Display`
- İzin matrisi (can_clear/kick/mute/grant/revoke/view)
- `PeerInfo::display_name` (nick, peer-id, addr)
- `PeerInfo::set_role`/`set_nick`/`set_muted`
- `PeerRegistry` (add/remove/get/find_by_nick/count_by_role)
- ID artımlı (reuse yok)

**`commands.rs`** (24 yeni test):
- `/clear` + `/clear <N>` + hatalı sayı
- `/on_admin`/`/off_admin` + alias (op, deop)
- `/kick`/`/mute`/`/unmute`
- `/role` + `/role <nick>`
- `/config` + `/config set` + multi-word value + Türkçe
- Help text yeni komutlar

### Canlı E2E Test (9 senaryo)

`scripts/test_config_roles_e2e.py`:

1. ✓ Config default oluşturulur (history=false)
2. ✓ Spoke bağlanır, hub mesaj alır
3. ✓ `/config` komutu JSON gösterir
4. ✓ `/config set history.enabled true` (dosyaya kaydedilir)
5. ✓ `/clear` history dosyasını temizler
6. ✓ `/clear 2` komutu işlenir
7. ✓ `/role` admin rolünü gösterir
8. ✓ `/who` peer listesi gösterir
9. ✓ Anonimlik — peer IP gizli

## Doğrulama

- `cargo check`: 0/0
- `cargo clippy --all-targets -- -D warnings`: 0 uyarı
- `cargo test`: **263/263** (169 önceki + 94 yeni)
- E2E: 9/9 senaryo başarılı

## İlgili Sayfalar

- [[architecture]] — modüler mimari (12 modül)
- [[v0.2-features]] — önceki sürüm özellikleri
- [[turkish-support]] — Türkçe karakter desteği
- [[chat-tui]] — sohbet TUI detayları
