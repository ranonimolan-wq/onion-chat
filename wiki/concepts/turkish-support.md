---
title: Turkish Character Support
created: 2026-06-28
updated: 2026-06-28
type: concept
tags: [utf-8, i18n, turkish, charset, rust-2024]
sources: [src/chat.rs, src/ui.rs, src/markdown.rs, src/commands.rs, src/history.rs]
confidence: high
---

# Turkish Character Support

OnionChat tam UTF-8 desteği sunar — Türkçe karakterler (ş, ğ, ü, ö, ç, ı,
İ, Ş, Ğ, Ü, Ö, Ç) tüm katmanlarda doğru çalışır: ağ transferi, TUI
input editing, markdown rendering, komut parsing, ve history persistansı.

## UTF-8 Modeli

Rust `String` ve `&str` tipleri UTF-8 kodlanmış. Her Türkçe karakter
2 bayt:

| Karakter | Unicode | UTF-8 Bayt |
|----------|---------|------------|
| ş | U+015F | C5 9F |
| ğ | U+011F | C4 9F |
| ü | U+00FC | C3 BC |
| ö | U+00F6 | C3 B6 |
| ç | U+00E7 | C3 A7 |
| ı | U+0131 | C4 B1 |
| İ | U+0130 | C4 B0 |
| Ş | U+015E | C5 9E |
| Ğ | U+011E | C4 9E |
| Ü | U+00DC | C3 9C |
| Ö | U+00D6 | C3 96 |
| Ç | U+00C7 | C3 87 |

## Katman Katman Destek

### 1. Ağ Transferi (`chat.rs`)

`send_chat` metinleri UTF-8 baytları olarak gönderir. `decode_chat`
`String::from_utf8` ile çözer. Hiçbir karakter dönüşümü yok — raw UTF-8.

```rust
let text = String::from_utf8(msg[1..].to_vec())?;
```

### 2. TUI Input Editing (`ui.rs`)

`ChatState` imleç pozisyonunu **bayt offset** olarak takip eder, ama
hareketleri `char::len_utf8()` ile yapar:

```rust
pub fn backspace(&mut self) {
    if self.cursor_pos > 0 {
        let prev = self.input[..self.cursor_pos].chars().last().unwrap();
        let prev_len = prev.len_utf8();
        self.cursor_pos -= prev_len;
        self.input.replace_range(self.cursor_pos..self.cursor_pos + prev_len, "");
    }
}
```

Bu sayede `ş` (2 bayt) silinince cursor 2 geri gider, `a` (1 bayt)
silinince 1 geri. Karakter sınırları korunur — asla yarım UTF-8 sequence
oluşmaz.

### 3. Markdown Rendering (`markdown.rs`)

`render_spans` eskiden `bytes[i] as char` kullanıyordu — bu **hatalıydı**.
Tek baytı char'a çevirince çok baytlı karakterler bozuluyordu (`ş` → `ÅŸ`).

Düzeltme: `text.char_indices()` ile karakter sınırlarında ilerle. Bu
sayede Türkçe karakterler markdown formatlamasında korunur.

### 4. Komut Parsing (`commands.rs`)

`/nick Şükrü` gibi komutlar `split_whitespace` ile ayrılır — bu UTF-8
güvenli. Türkçe nickler, dosya yolları, ve argümanlar doğru parse edilir.

### 5. History Persistansı (`history.rs`)

JSON Lines formatında `serde_json` kullanır — serde UTF-8 destekli.
Türkçe mesajlar `~/.onionchat/history.jsonl` dosyasına raw UTF-8 olarak
yazılır, geri yüklemede birebir aynı gelir.

## Test Kapsamı

### Unit Testler (59 yeni test)

**`chat.rs`** (13 Türkçe test):
- `decode_all_turkish_lowercase_chars` — ş ğ ü ö ç ı
- `decode_all_turkish_uppercase_chars` — Ş Ğ Ü Ö Ç I
- `decode_turkish_sentence_with_punctuation` — noktalama
- `decode_turkish_with_numbers_and_mixed`
- `decode_turkish_long_message` — çok baytlı + uzun
- `decode_only_turkish_special_chars`
- `decode_turkish_with_emoji_mixed`
- 6 adet `send_and_decode_*_roundtrip` — gerçek `Connection` üzerinden

**`ui.rs`** (24 Türkçe test):
- `insert_char_turkish_lowercase_chars`
- `insert_char_turkish_uppercase_chars`
- `insert_char_turkish_mixed_with_ascii`
- `insert_char_turkish_word_istanbul`
- `backspace_turkish_s_cedilla`
- `backspace_turkish_g_breve`
- `backspace_turkish_i_dotless`
- `backspace_turkish_capital_i_with_dot`
- `backspace_mixed_turkish_and_ascii`
- `backspace_all_turkish_special_chars_one_by_one`
- `move_left_turkish_char_2_bytes`
- `move_right_turkish_char_2_bytes`
- `move_left_right_through_all_turkish_chars`
- `move_left_at_zero_with_turkish_input`
- `move_right_at_end_with_turkish_input`
- `home_end_with_turkish_input`
- `take_input_turkish_text`
- `take_input_all_turkish_chars`

**`markdown.rs`** (11 Türkçe test):
- `turkish_chars_preserved`
- `turkish_bold_text`
- `turkish_italic_text`
- `turkish_code_text`
- `turkish_sentence_with_bold_and_emoji`
- `all_turkish_chars_in_bold`
- `turkish_word_istanbul_bold`
- `turkish_mixed_with_emoji_shortcodes`
- `turkish_sentence_no_markdown`
- `turkish_punctuation_preserved`

**`commands.rs`** (8 Türkçe test):
- `nick_command_turkish_single_word`
- `nick_command_turkish_full_name`
- `nick_command_all_turkish_chars`
- `nick_command_istanbul`
- `send_command_turkish_filename`
- `send_command_turkish_relative_path`
- `unknown_command_with_turkish_args`
- `non_command_turkish_text_returns_none`

**`history.rs`** (8 Türkçe test):
- `append_and_load_turkish_text`
- `append_and_load_all_turkish_chars`
- `append_and_load_long_turkish_message`
- `append_and_load_turkish_with_emoji_in_text`
- `append_and_load_mixed_turkish_and_emoji_chars`
- `load_recent_turkish_messages`
- `history_entry_turkish_text_serde_roundtrip`
- `history_entry_all_turkish_chars_serde`

### Canlı E2E Test (11 senaryo)

`scripts/test_turkish_chars_e2e.py` 3 proses (hub + 2 spoke) başlatır:

1. ✓ `merhaba dünya` (ü karakteri)
2. ✓ Tüm Türkçe karakterler (Şş Ğğ Üü Öö Çç İı)
3. ✓ Türkçe cümle + noktalama
4. ✓ Türkçe + emoji karışık
5. ✓ Türkçe markdown `*kalın*`
6. ✓ `İstanbul` (İ başta, büyük)
7. ✓ `/nick Şükrü` (Türkçe nick)
8. ✓ `/help` komutu
9. ✓ Çift yönlü Türkçe mesaj
10. ✓ History'de Türkçe kalıcılık (JSON Lines)
11. ✓ Backspace Türkçe karakter silme

## Bilinen Sınırlamalar

- **İç içe markdown**: `*bold _italic_*` çalışmaz (ilk `*` ile son `*`
  eşleşir, içerdikleri literal olur). Bu kabul edilebilir bir sınırlama.
- **Klavye layout**: Crossterm terminalin klavye layout'unu kullanır.
  Türkçe Q veya F klavye layout'unda Türkçe karakterler doğru gelir.
- **Bazı terminal emülatörleri**: Eski terminal emülatörleri UTF-8'i
  yanlış render edebilir. Modern terminaler (gnome-terminal, kitty,
  alacritty, iTerm2) sorun yok.

## Düzeltmeler (Bu Oturum)

### Kritik Bug: `markdown.rs` UTF-8 Bozulması

**Sorun**: `render_spans` eskiden `bytes[i] as char` kullanıyordu. Bu
tek baytı char'a çeviriyordu, bu da çok baytlı UTF-8 karakterleri
bozuyordu. `ş` (C5 9F) → `Å` (C5) + `Ÿ` (9F) olarak render ediliyordu.

**Çözüm**: `text.char_indices().peekable()` ile karakter sınırlarında
ilerle. Her iterasyon bir tam Unicode karakter döner.

**Etki**: Tüm Türkçe karakterler artık markdown rendering'de doğru
görünüyor.

## İlgili Sayfalar

- [[architecture]] — modüler mimari
- [[chat-tui]] — sohbet TUI detayları
- [[v0.2-features]] — v0.2 özellik seti
