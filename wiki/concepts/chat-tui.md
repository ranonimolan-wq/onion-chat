---
title: Chat Module & TUI
created: 2026-06-28
updated: 2026-06-28
type: concept
tags: [chat, tui, ratatui, crossterm, message-format, e2ee, rust-2024]
sources: [src/chat.rs, src/ui.rs]
confidence: high
---

# Chat Module & TUI

OnionChat'ın sohbet modülü ve ratatui tabanlı TUI'si. Bu, OnionChat'ın
birincil özelliği — dosya transferi artık ek özellik olarak konumlandırıldı.

## Mimari Yerleşim

```
[Klavye] ───► [crossterm event poll (spawn_blocking)] ──► mpsc ──┐
                                                                 │
                                                                 ▼
                                                          [tokio::select!]
                                                                 │
[Ağ] ───► [Receiver Stream (tokio::spawn)] ──► mpsc ──────────►│
                                                                 ▼
                                                          [ChatState güncelle]
                                                                 │
                                                                 ▼
                                                          [terminal.draw()]
                                                                 │
[Sender] ◄── mpsc ◄── [chat::send_chat_split()] ◄── [Enter tuşu]
   │
   ▼
[arka plan yazma görevi: AES-GCM şifrele + TCP'ye yaz]
```

İki event kaynağı `tokio::select!` ile birleştirilir:
- **Ağ olayları**: `Receiver` (Stream) bir görevde okunur, mesajlar mpsc kanalına iletilir
- **Klavye olayları**: `crossterm::event::poll` (blocking) `spawn_blocking` ile ayrı bir thread'de çalışır

## Chat Protokolü

Her sohbet mesajı tek bir AES-GCM çerçevesi olarak gönderilir. İlk bayt
tip önekidir:

```text
[type=0x01][utf-8 metin]
```

Dosya transferi `0x02` (`file_transfer::PROTOCOL_VERSION`) ile başladığı
için, alıcı taraf ilk bayta göre modu anlayabilir. Bu sayede listen
tarafı peek ile tek mesaj okuyup chat mi dosya transferi mi olduğuna
karar verebilir.

## Mod Seçimi (Listen Tarafı)

Listen tarafı ECDHE el sıkışmasından sonra ilk mesajı peek eder:

```rust
let first_msg = conn.recv_message().await?;
let is_file_transfer = first_msg.len() == 1
    && first_msg[0] == file_transfer::PROTOCOL_VERSION;

if is_file_transfer {
    // Sürüm baytı zaten tüketildi — recv_file_after_version çağır
    file_transfer::recv_file_after_version(&mut conn, ...).await?;
} else {
    // Chat mesajı — decode et ve TUI'a ilk mesaj olarak geçir
    let text = chat::decode_chat(&first_msg)?.unwrap();
    ui::run_chat_tui(conn, UiMode::Listen, peer, Some(text)).await?;
}
```

Connect tarafı `--file` verilmişse dosya gönderir (eski davranış),
verilmemişse TUI'a girer ve kullanıcı mesaj yazana kadar bekler.

## TUI Layout

```text
┌─ OnionChat v0.1 — Tor P2P E2EE Chat ────────────┐
├─ Sohbet ────────────────────────────────────────┤
│ [12:34] < merhaba                                │
│ [12:34] > selam nasılsın                         │
│ [12:35] < iyiyim, teşekkürler                    │
│                                                  │
├─ Mesaj > _____________________________________ │
│ [listen] peer=127.0.0.1:42458 | bağlı           │
└──────────────────────────────────────────────────┘
```

### Bileşenler

- **Title bar** (3 satır): "OnionChat v0.1 — Tor P2P E2EE Chat" (cyan, bold)
- **Sohbet geçmişi** (flexible): zaman damgalı mesajlar
  - Sent: `> ` öneki (yeşil)
  - Received: `< ` öneki (cyan)
  - System: `* ` öneki (sarı, italik)
- **Mesaj kutusu** (3 satır): kullanıcının yazdığı metin, imleç gösterilir
- **Status bar** (1 satır): mod, peer adresi, bağlantı durumu, kısayollar

## Klavye Kısayolları

| Tuş | İşlev |
|-----|-------|
| `Enter` | Mesajı gönder |
| `Esc` veya `Ctrl+C` | Çık |
| `↑` / `↓` | Sohbet geçmişinde kaydır |
| `←` / `→` | İmleci hareket ettir |
| `Backspace` | İmleçten önceki karakteri sil |
| `Home` / `End` | İmleci satır başına / sonuna |

UTF-8 çok baytlı karakterler (örn. Türkçe `ş`, `ğ`, `ü`) doğru işlenir —
`cursor_pos` bayt offset olarak takip edilir ama `char` sınırlarında
kalır. Backspace ve ok tuşları `char::len_utf8()` ile doğru hareket eder.

## Bağlantı Kopması

Peer bağlantıyı kapatırsa:
1. `Receiver` stream'i biter (None döner)
2. Okuma görevi `net_tx.send(None)` gönderir
3. Ana loop `None` görür, `state.connected = false` yapar
4. Sistem mesajı ekler: "Peer bağlantıyı kapattı. Esc ile çıkın."
5. Kullanıcı Esc basana kadar TUI açık kalır

## Test Kapsamı

### `chat.rs` (8 test)
- `decode_chat_message_returns_text` — temel decode
- `decode_chat_message_with_utf8_turkish_chars` — Türkçe karakterler
- `decode_file_marker_returns_none` — dosya işaretçisi tanıma
- `decode_empty_message_errors`, `decode_unknown_type_byte_errors`,
  `decode_invalid_utf8_errors` — hata durumları
- `chat_msg_type_differs_from_file_version` — sabit değer güvenliği
- `chat_msg_type_is_0x01` — sabit değer testi

### `ui.rs` (21 test)
- `ChatState::insert_char`, `backspace`, `move_left`, `move_right`,
  `move_home`, `move_end`, `take_input` — input düzenleme
- UTF-8 çok baytlı karakter testleri
- `add_sent/received` scroll reset davranışı
- `scroll_up/down` sınır kontrolleri
- `visible_lines` scroll offset ile görünür pencere hesabı
- `format_timestamp` HH:MM formatı
- `line_to_spans` renkli span üretimi
- `ChatLine` factory constructor'ları

### E2E Test (Python PTY tabanlı)
`scripts/test_chat_e2e.py` iki proses başlatır (PTY üzerinden TUI
için TTY gerek), mesaj alışverişini doğrular:
- Connect → Listen: "merhaba" gönderildi ✓
- Listen → Connect: "selam" gönderildi ✓ (çift yönlü!)
- Connect → Listen: "uzun mesaj test" ✓

## İlgili Sayfalar

- [[architecture]] — güncel modül yapısı
- [[chunked-file-transfer]] — dosya transfer protokolü (ek özellik)
- [[socks5]] — anonimlik transportu
- [[bug-fixes-2026-06-28-v3]] — bu oturumun düzeltmeleri
