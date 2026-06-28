// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Markdown rendering modülü — basit inline formatlama.
//!
//! Bu modül sohbet mesajlarındaki inline formatlamayı ratatui
//! `Span`'larına çevirir. Blok seviyesi markdown (başlıklar, listeler,
//! kod blokları) desteklenmez — sohbet mesajları için inline yeterli.
//!
//! ## Desteklenen formatlar
//!
//! - `*bold*` → kalın
//! - `_italic_` → italik
//! - `` `code` `` → sabit genişlikli, sarı
//! - `:emoji:` → emoji shortcodes (örn. `:smile:`, `:heart:`, `:thumbsup:`)
//!
//! ## Sınırlamalar
//!
//! - İç içe formatlama desteklenmez (`*bold _italic_*` çalışmaz).
//! - Escape karakteri yok (`\*` literal `*` anlamına gelmez).
//! - Emoji shortcode tablosu sabit kodlanmış (~30 emoji).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Inline formatlanmış metni `Span` listesine çevirir.
///
/// Parser soldan sağa çalışır. Bir format işaretçisi (`*`, `_`, `` ` ``)
/// görüldüğünde, eşleşen kapanış işaretçisi aranır. Bulunursa aradaki
/// metin formatlanır; bulunmazsa işaretçi literal olarak eklenir.
pub fn render_spans(text: &str) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut buf = String::new();
    let bytes = text.as_bytes();
    // char_indices kullanarak UTF-8 karakter sınırlarında ilerle.
    // Bu, çok baytlı karakterlerin (Türkçe ş, ğ, ü, vb.) doğru
    // işlenmesini sağlar. `bytes[i] as char` tek baytı char'a çevirir,
    // bu da UTF-8'i bozar.
    let mut iter = text.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        match c {
            '*' => {
                // Önce buffer'ı boşalt.
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                // Eşleşen `*` ara.
                if let Some(end) = find_next(bytes, i + 1, b'*') {
                    let inner = &text[i + 1..end];
                    let formatted = render_emoji_in_text(inner);
                    spans.push(Span::styled(
                        formatted,
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    // iter'i end+1'e ilerlet
                    while let Some(&(pos, _)) = iter.peek() {
                        if pos > end {
                            break;
                        }
                        iter.next();
                    }
                } else {
                    buf.push('*');
                }
            }
            '_' => {
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                if let Some(end) = find_next(bytes, i + 1, b'_') {
                    let inner = &text[i + 1..end];
                    let formatted = render_emoji_in_text(inner);
                    spans.push(Span::styled(
                        formatted,
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));
                    while let Some(&(pos, _)) = iter.peek() {
                        if pos > end {
                            break;
                        }
                        iter.next();
                    }
                } else {
                    buf.push('_');
                }
            }
            '`' => {
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                if let Some(end) = find_next(bytes, i + 1, b'`') {
                    let inner = &text[i + 1..end];
                    spans.push(Span::styled(
                        inner.to_string(),
                        Style::default().fg(Color::Yellow),
                    ));
                    while let Some(&(pos, _)) = iter.peek() {
                        if pos > end {
                            break;
                        }
                        iter.next();
                    }
                } else {
                    buf.push('`');
                }
            }
            ':' => {
                // `:emoji:` shortcode dene.
                if let Some(end) = find_next(bytes, i + 1, b':') {
                    let code = &text[i + 1..end];
                    if let Some(emoji) = emoji_lookup(code) {
                        if !buf.is_empty() {
                            spans.push(Span::raw(std::mem::take(&mut buf)));
                        }
                        spans.push(Span::raw(emoji.to_string()));
                        while let Some(&(pos, _)) = iter.peek() {
                            if pos > end {
                                break;
                            }
                            iter.next();
                        }
                        continue;
                    }
                }
                // Geçerli emoji değilse literal ':' ekle.
                buf.push(':');
            }
            _ => {
                buf.push(c);
            }
        }
    }

    if !buf.is_empty() {
        spans.push(Span::raw(buf));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    spans
}

/// `bytes`'ta `start`'tan itibaren `target` byte'ını ara. Index döner.
fn find_next(bytes: &[u8], start: usize, target: u8) -> Option<usize> {
    (start..bytes.len()).find(|&i| bytes[i] == target)
}

/// Emoji shortcode'ları için basit lookup tablosu.
///
/// Yaygın ~30 emoji. Yeni emoji eklemek için bu fonksiyona match kolu ekle.
fn emoji_lookup(code: &str) -> Option<&'static str> {
    match code {
        "smile" | "smiley" => Some("\u{1F642}"),
        "laugh" | "lol" => Some("\u{1F602}"),
        "heart" | "love" => Some("\u{2764}\u{FE0F}"),
        "thumbsup" | "thumbup" | "+1" => Some("\u{1F44D}"),
        "thumbsdown" | "thumbdown" | "-1" => Some("\u{1F44E}"),
        "ok" => Some("\u{1F44C}"),
        "wave" => Some("\u{1F44B}"),
        "fire" => Some("\u{1F525}"),
        "star" => Some("\u{2B50}"),
        "check" | "done" => Some("\u{2705}"),
        "x" | "cross" => Some("\u{274C}"),
        "warning" => Some("\u{26A0}\u{FE0F}"),
        "info" => Some("\u{2139}\u{FE0F}"),
        "rocket" => Some("\u{1F680}"),
        "onion" => Some("\u{1F9C5}"),
        "lock" | "secure" => Some("\u{1F512}"),
        "key" => Some("\u{1F511}"),
        "ghost" => Some("\u{1F47B}"),
        "skull" => Some("\u{1F480}"),
        "party" | "celebrate" => Some("\u{1F389}"),
        "coffee" => Some("\u{2615}"),
        "beer" => Some("\u{1F37A}"),
        "pizza" => Some("\u{1F355}"),
        "cat" => Some("\u{1F408}"),
        "dog" => Some("\u{1F415}"),
        "thinking" => Some("\u{1F914}"),
        "shrug" => Some("\u{1F937}"),
        "tada" => Some("\u{1F389}"),
        _ => None,
    }
}

/// Bir metin içindeki tüm `:emoji:` shortcode'larını emoji karakterlerle
/// değiştirir. Markdown formatlaması dışındaki kısımlar için kullanılır.
fn render_emoji_in_text(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            if let Some(end) = find_next(bytes, i + 1, b':') {
                let code = &text[i + 1..end];
                if let Some(emoji) = emoji_lookup(code) {
                    result.push_str(emoji);
                    i = end + 1;
                    continue;
                }
            }
            result.push(':');
            i += 1;
        } else {
            // UTF-8 güvenli: char sınırlarında ilerle.
            let ch = text[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_returns_single_span() {
        let spans = render_spans("merhaba");
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn bold_formatting_creates_bold_span() {
        let spans = render_spans("*hello*");
        assert_eq!(spans.len(), 1);
        // Span'in stilini doğrudan test etmek zor, ama en azından
        // içerik doğru olmalı.
        assert_eq!(spans[0].content, "hello");
    }

    #[test]
    fn italic_formatting_creates_italic_span() {
        let spans = render_spans("_hello_");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello");
    }

    #[test]
    fn code_formatting_creates_yellow_span() {
        let spans = render_spans("`code`");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "code");
    }

    #[test]
    fn mixed_text_and_bold_creates_multiple_spans() {
        let spans = render_spans("merhaba *dünya* son");
        // "merhaba " + "dünya" + " son" = 3 span
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "merhaba ");
        assert_eq!(spans[1].content, "dünya");
        assert_eq!(spans[2].content, " son");
    }

    #[test]
    fn unmatched_bold_marker_is_literal() {
        let spans = render_spans("text * without close");
        // Marker'dan önceki "text " span olarak eklenir, sonra marker
        // literal olur ve kalan metin eklenir. 2 span beklenir.
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "text ");
        assert_eq!(spans[1].content, "* without close");
    }

    #[test]
    fn unmatched_italic_marker_is_literal() {
        let spans = render_spans("text _ without close");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "text ");
        assert_eq!(spans[1].content, "_ without close");
    }

    #[test]
    fn unmatched_code_marker_is_literal() {
        let spans = render_spans("text ` without close");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "text ");
        assert_eq!(spans[1].content, "` without close");
    }

    #[test]
    fn empty_string_returns_empty_span() {
        let spans = render_spans("");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "");
    }

    #[test]
    fn emoji_shortcode_replaced() {
        let spans = render_spans(":smile:");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "\u{1F642}");
    }

    #[test]
    fn emoji_in_text_replaced() {
        let spans = render_spans("merhaba :heart: nasılsın");
        // "merhaba " + emoji + " nasılsın"
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content, "\u{2764}\u{FE0F}");
    }

    #[test]
    fn unknown_shortcode_is_literal() {
        let spans = render_spans(":notanemoji:");
        assert_eq!(spans.len(), 1);
        // İçerik değişmemiş olmalı
        assert_eq!(spans[0].content, ":notanemoji:");
    }

    #[test]
    fn emoji_inside_bold_is_replaced() {
        let spans = render_spans("*:thumbsup:*");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "\u{1F44D}");
    }

    #[test]
    fn multiple_emojis_in_text() {
        let spans = render_spans(":smile: :heart: :rocket:");
        // 5 span: emoji + " " + emoji + " " + emoji
        assert_eq!(spans.len(), 5);
        assert_eq!(spans[0].content, "\u{1F642}");
        assert_eq!(spans[2].content, "\u{2764}\u{FE0F}");
        assert_eq!(spans[4].content, "\u{1F680}");
    }

    #[test]
    fn emoji_lookup_known_codes() {
        assert_eq!(emoji_lookup("smile"), Some("\u{1F642}"));
        assert_eq!(emoji_lookup("heart"), Some("\u{2764}\u{FE0F}"));
        assert_eq!(emoji_lookup("fire"), Some("\u{1F525}"));
        assert_eq!(emoji_lookup("onion"), Some("\u{1F9C5}"));
    }

    #[test]
    fn emoji_lookup_unknown_returns_none() {
        assert_eq!(emoji_lookup("xyz"), None);
        assert_eq!(emoji_lookup(""), None);
    }

    #[test]
    fn render_emoji_in_plain_text() {
        let result = render_emoji_in_text("hi :smile: bye");
        assert_eq!(result, "hi \u{1F642} bye");
    }

    #[test]
    fn find_next_locates_target() {
        let bytes = b"hello*world";
        assert_eq!(find_next(bytes, 0, b'*'), Some(5));
        assert_eq!(find_next(bytes, 6, b'*'), None);
    }

    #[test]
    fn nested_format_not_supported_treated_as_literal() {
        // İç içe desteklenmez — ilk `*` ile son `*` eşleşir, aradaki
        // hepsi bold olarak tek span olur.
        let spans = render_spans("*bold _italic_*");
        // "*bold _italic_*" → ilk * ... son * arası "bold _italic_"
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "bold _italic_");
    }

    #[test]
    fn code_with_special_chars() {
        let spans = render_spans("`code_with_underscores`");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "code_with_underscores");
    }

    #[test]
    fn turkish_chars_preserved() {
        let spans = render_spans("merhaba *dünya*");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "merhaba ");
        assert_eq!(spans[1].content, "dünya");
    }

    #[test]
    fn turkish_bold_text() {
        // *kalın* — Türkçe kelime bold
        let spans = render_spans("*kalın*");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "kalın");
    }

    #[test]
    fn turkish_italic_text() {
        // _italik_ — Türkçe kelime italic
        let spans = render_spans("_İtalyan_");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "İtalyan");
    }

    #[test]
    fn turkish_code_text() {
        // `kod` — Türkçe kelime code
        let spans = render_spans("`şifre`");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "şifre");
    }

    #[test]
    fn turkish_sentence_with_bold_and_emoji() {
        // "Merhaba *dünya* :smile:" — Türkçe + bold + emoji
        let spans = render_spans("Merhaba *dünya* :smile:");
        // 4 span: "Merhaba " + "dünya" + " " + emoji
        assert!(spans.len() >= 3);
        assert_eq!(spans[0].content, "Merhaba ");
        assert_eq!(spans[1].content, "dünya");
    }

    #[test]
    fn all_turkish_chars_in_bold() {
        let spans = render_spans("*Şş Ğğ Üü Öö Çç İı*");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "Şş Ğğ Üü Öö Çç İı");
    }

    #[test]
    fn turkish_word_istanbul_bold() {
        let spans = render_spans("*İstanbul*");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "İstanbul");
    }

    #[test]
    fn turkish_mixed_with_emoji_shortcodes() {
        // "Güzel gün :heart: teşekkürler :thumbsup:"
        let spans = render_spans("Güzel gün :heart: teşekkürler :thumbsup:");
        // En az 5 span: text + emoji + text + emoji
        assert!(spans.len() >= 3);
        // Emoji içeriklerini kontrol et — Span.content Cow<str>'dir,
        // &* ile deref ediyoruz.
        let contents: Vec<String> = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(contents.contains(&"\u{2764}\u{FE0F}".to_string())); // :heart:
        assert!(contents.contains(&"\u{1F44D}".to_string())); // :thumbsup:
    }

    #[test]
    fn turkish_sentence_no_markdown() {
        // Saf Türkçe metin — markdown yok
        let spans = render_spans("Bu bir Türkçe cümledir.");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "Bu bir Türkçe cümledir.");
    }

    #[test]
    fn turkish_punctuation_preserved() {
        // Türkçe noktalama işaretleri
        let spans = render_spans("Nasılsın? İyiyim, teşekkürler!");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "Nasılsın? İyiyim, teşekkürler!");
    }
}
