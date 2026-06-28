// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Sohbet modülü — `network::Connection` üzerinden metin mesajlaşma.
//!
//! Bu modül sohbet mesajlarının serileştirilmesinden ve çözümlenmesinden
//! sorumludur. Ağ ve kriptografi detaylarına dokunmaz; dosya transferi
//! `file_transfer` modülünün sorumluluğundadır.
//!
//! ## Mesaj formatı
//!
//! Her sohbet mesajı `network::Connection::send_message` ile tek bir
//! AES-GCM çerçevesi olarak gönderilir. İlk bayt tip önekidir:
//!
//! ```text
//! [type=0x01][utf-8 metin]
//! ```
//!
//! Dosya transferi `0x02` (`file_transfer::PROTOCOL_VERSION`) ile
//! başladığı için, alıcı taraf ilk bayta göre modu anlayabilir:
//! - `0x01` → sohbet mesajı
//! - `0x02` → dosya transferi başlangıcı

use anyhow::{anyhow, Result};

use crate::network::{Connection, Sender};

/// Sohbet mesajı tip öneki. Dosya transferi `0x02` kullandığı için
/// çakışma yoktur.
pub const CHAT_MSG_TYPE: u8 = 0x01;

/// "Ekranı temizle" komutu tip öneki. Hub `/clear` yapınca tüm peer'lara
/// bu mesaj gönderilir, peer'lar kendi ekranlarını temizler.
pub const CLEAR_MSG_TYPE: u8 = 0x03;

/// Bir sohbet mesajını peer'a gönderir. Wire formatı: `[0x01][utf-8]`.
///
/// `&mut Connection` alır — sıralı (tek-görev) senaryolar için uygundur.
/// Birden fazla görev aynı bağlantıyı paylaşacaksa `send_chat_split`
/// kullanın.
#[allow(dead_code)]
pub async fn send_chat(conn: &mut Connection, text: &str) -> Result<()> {
    let mut buf = Vec::with_capacity(1 + text.len());
    buf.push(CHAT_MSG_TYPE);
    buf.extend_from_slice(text.as_bytes());
    conn.send_message(&buf).await
}

/// `Sender` (yazma yarısı) üzerinden bir sohbet mesajı gönderir.
/// `Connection::split` ile üretilen `Sender` ile kullanılır.
///
/// TUI event loop'unda ana görev okuma yaparken arka plan yazma
/// görevi mesajı TCP'ye yazar.
pub async fn send_chat_split(sender: &Sender, text: &str) -> Result<()> {
    let mut buf = Vec::with_capacity(1 + text.len());
    buf.push(CHAT_MSG_TYPE);
    buf.extend_from_slice(text.as_bytes());
    sender.send(buf).await
}

/// `Sender` üzerinden "ekranı temizle" komutu gönderir. Hub `/clear`
/// yapınca tüm peer'lara bu mesaj broadcast edilir. Peer'lar bu mesajı
/// alınca kendi `state.messages.clear()`'ını çağırır.
///
/// Wire formatı: tek bayt `[0x03]`.
pub async fn send_clear_split(sender: &Sender) -> Result<()> {
    sender.send(vec![CLEAR_MSG_TYPE]).await
}

/// Bir mesajın "clear" komutu olup olmadığını kontrol eder.
pub fn is_clear_message(msg: &[u8]) -> bool {
    msg.len() == 1 && msg[0] == CLEAR_MSG_TYPE
}

/// Alınan bir mesajın ilk baytından tipini çıkarır.
///
/// # Dönüş değerleri
/// - `Ok(Some(text))` — sohbet mesajı, UTF-8 metni döner.
/// - `Ok(None)` — dosya transferi başlangıcı (ilk bayt `0x02`).
///   Çağıran `file_transfer::recv_file_after_version` çağırmalıdır.
/// - `Err(...)` — geçersiz mesaj (boş, bilinmeyen tip, veya UTF-8 hatası).
///
/// Not: "clear" komutu (`0x03`) için önce `is_clear_message` kontrol edin.
pub fn decode_chat(msg: &[u8]) -> Result<Option<String>> {
    if msg.is_empty() {
        return Err(anyhow!("empty message"));
    }
    match msg[0] {
        CHAT_MSG_TYPE => {
            let text = String::from_utf8(msg[1..].to_vec())
                .map_err(|e| anyhow!("invalid UTF-8 in chat message: {}", e))?;
            Ok(Some(text))
        }
        crate::file_transfer::PROTOCOL_VERSION => Ok(None),
        CLEAR_MSG_TYPE => Ok(None), // clear komutu — çağıran is_clear_message ile kontrol etmeli
        other => Err(anyhow!("unknown message type byte: 0x{:02x}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_chat_message_returns_text() {
        let msg = [CHAT_MSG_TYPE, b'h', b'i'];
        assert_eq!(decode_chat(&msg).unwrap(), Some("hi".to_string()));
    }

    #[test]
    fn decode_chat_message_with_utf8_turkish_chars() {
        // Türkçe karakterler UTF-8'de çok baytlı; bu test onların doğru
        // çözümlendiğini doğrular.
        let text = "merhaba dünya";
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_all_turkish_lowercase_chars() {
        // Türkçe alfabenin tüm küçük harfleri: ş ğ ü ö ç ı
        let text = "şğöüçİ"; // 'İ' büyük, diğerleri küçük — hepsini test et
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_all_turkish_uppercase_chars() {
        // Büyük harfler: Ş Ğ Ü Ö Ç I
        let text = "ŞĞÜÖÇI";
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_turkish_sentence_with_punctuation() {
        // Noktalı işaretlerle Türkçe cümle
        let text = "Merhaba! Nasılsın? İyi, teşekkürler.";
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_turkish_with_numbers_and_mixed() {
        // Türkçe + sayılar + ASCII karışık
        let text = "3 tane şeker, 5 lira, İstanbul'da";
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_turkish_long_message() {
        // Uzun Türkçe mesaj — çok baytlı karakterlerin çoğunun testi
        let text = "Çanakkale'de Şehitler Günü. Öğretmenler için özel gün. \
                    Güzellik her yerde — aç gözünü, gör dünyayı!";
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_only_turkish_special_chars() {
        // Sadece Türkçe özel karakterler
        let text = "şşşğğğüüööççıı";
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_turkish_with_emoji_mixed() {
        // Türkçe + emoji karışık
        let text = "Merhaba dünya :smile: güzel gün :heart:";
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_empty_turkish_string_after_type_byte() {
        // Tip baytından sonra boş string (sadece tip)
        let msg = vec![CHAT_MSG_TYPE];
        let result = decode_chat(&msg).unwrap();
        assert_eq!(result, Some("".to_string()));
    }

    #[test]
    fn decode_turkish_with_newlines_and_tabs() {
        // Türkçe + yeni satır + tab
        let text = "Satır 1: Merhaba\nSatır 2: Dünya\tSon";
        let mut msg = vec![CHAT_MSG_TYPE];
        msg.extend_from_slice(text.as_bytes());
        assert_eq!(decode_chat(&msg).unwrap(), Some(text.to_string()));
    }

    #[test]
    fn decode_file_marker_returns_none() {
        let msg = [crate::file_transfer::PROTOCOL_VERSION];
        assert_eq!(decode_chat(&msg).unwrap(), None);
    }

    #[test]
    fn decode_empty_message_errors() {
        assert!(decode_chat(&[]).is_err());
    }

    #[test]
    fn decode_unknown_type_byte_errors() {
        let msg = [0xFF];
        assert!(decode_chat(&msg).is_err());
    }

    #[test]
    fn decode_invalid_utf8_errors() {
        // 0xFF, 0xFE geçerli UTF-8 değil.
        let msg = [CHAT_MSG_TYPE, 0xFF, 0xFE];
        assert!(decode_chat(&msg).is_err());
    }

    #[test]
    fn chat_msg_type_differs_from_file_version() {
        // Bu iki sabit farklı OLMALI; aksi halde protokol çakışması olur.
        assert_ne!(CHAT_MSG_TYPE, crate::file_transfer::PROTOCOL_VERSION);
    }

    #[test]
    fn chat_msg_type_is_0x01() {
        // Sabit değer testi — değişiklik bilinçli olmalı.
        assert_eq!(CHAT_MSG_TYPE, 0x01);
    }

    // ===== Clear komutu testleri =====

    #[test]
    fn clear_msg_type_is_0x03() {
        assert_eq!(CLEAR_MSG_TYPE, 0x03);
    }

    #[test]
    fn clear_msg_type_differs_from_chat_and_file() {
        assert_ne!(CLEAR_MSG_TYPE, CHAT_MSG_TYPE);
        assert_ne!(CLEAR_MSG_TYPE, crate::file_transfer::PROTOCOL_VERSION);
    }

    #[test]
    fn is_clear_message_true_for_clear_byte() {
        assert!(is_clear_message(&[CLEAR_MSG_TYPE]));
    }

    #[test]
    fn is_clear_message_false_for_chat_message() {
        assert!(!is_clear_message(&[CHAT_MSG_TYPE, b'h', b'i']));
    }

    #[test]
    fn is_clear_message_false_for_file_marker() {
        assert!(!is_clear_message(&[crate::file_transfer::PROTOCOL_VERSION]));
    }

    #[test]
    fn is_clear_message_false_for_empty() {
        assert!(!is_clear_message(&[]));
    }

    #[test]
    fn is_clear_message_false_for_unknown_type() {
        assert!(!is_clear_message(&[0xFF]));
    }

    #[test]
    fn is_clear_message_false_for_clear_with_extra_bytes() {
        // clear komutu tek bayt olmalı — ekstra bayt varsa clear değildir
        assert!(!is_clear_message(&[CLEAR_MSG_TYPE, 0x00]));
    }

    #[tokio::test]
    async fn send_clear_split_sends_correct_byte() -> Result<()> {
        use crate::network::Connection;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut conn = Connection::from_stream(stream).await?;
            let msg = conn.recv_message().await?;
            Ok::<_, anyhow::Error>(msg)
        });

        let conn = Connection::connect(addr, None).await?;
        let (sender, _receiver) = conn.split();
        send_clear_split(&sender).await?;
        let msg = server.await??;

        assert!(is_clear_message(&msg));
        Ok(())
    }

    #[tokio::test]
    async fn send_clear_and_chat_roundtrip() -> Result<()> {
        use crate::network::Connection;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut conn = Connection::from_stream(stream).await?;
            let msg1 = conn.recv_message().await?;
            let msg2 = conn.recv_message().await?;
            Ok::<_, anyhow::Error>((msg1, msg2))
        });

        let conn = Connection::connect(addr, None).await?;
        let (sender, _receiver) = conn.split();
        send_chat_split(&sender, "merhaba").await?;
        send_clear_split(&sender).await?;
        let (msg1, msg2) = server.await??;

        // İlk mesaj chat
        assert!(!is_clear_message(&msg1));
        assert_eq!(decode_chat(&msg1)?.unwrap(), "merhaba");
        // İkinci mesaj clear
        assert!(is_clear_message(&msg2));
        Ok(())
    }

    /// `send_chat` + `decode_chat` round-trip yardımcı fonksiyonu.
    /// Gerçek `Connection` yerine sahte bir bağlantı kullanır.
    async fn chat_roundtrip_helper(text: &str) -> Result<String> {
        use crate::network::Connection;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let text_clone = text.to_string();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut conn = Connection::from_stream(stream).await?;
            let msg = conn.recv_message().await?;
            Ok::<_, anyhow::Error>(msg)
        });

        let mut client = Connection::connect(addr, None).await?;
        send_chat(&mut client, &text_clone).await?;
        let msg = server.await??;
        drop(client);
        let decoded = decode_chat(&msg)?;
        decoded.ok_or_else(|| anyhow::anyhow!("expected chat message, got file marker"))
    }

    #[tokio::test]
    async fn send_and_decode_ascii_roundtrip() -> Result<()> {
        let result = chat_roundtrip_helper("hello world").await?;
        assert_eq!(result, "hello world");
        Ok(())
    }

    #[tokio::test]
    async fn send_and_decode_turkish_roundtrip() -> Result<()> {
        let result = chat_roundtrip_helper("merhaba dünya").await?;
        assert_eq!(result, "merhaba dünya");
        Ok(())
    }

    #[tokio::test]
    async fn send_and_decode_all_turkish_chars_roundtrip() -> Result<()> {
        // Tüm Türkçe karakterler tek mesajda
        let text = "Şş Ğğ Üü Öö Çç İı ĞÜŞİÖÇ ğüşıöç";
        let result = chat_roundtrip_helper(text).await?;
        assert_eq!(result, text);
        Ok(())
    }

    #[tokio::test]
    async fn send_and_decode_turkish_long_message_roundtrip() -> Result<()> {
        // 1000 baytlık Türkçe mesaj — chunked transfer sınırlarını test
        let text = "Şükrü".repeat(200); // ~1000 bayt
        let result = chat_roundtrip_helper(&text).await?;
        assert_eq!(result, text);
        Ok(())
    }

    #[tokio::test]
    async fn send_and_decode_turkish_with_emoji_roundtrip() -> Result<()> {
        let text = "Selam :heart: nasılsın :smile:";
        let result = chat_roundtrip_helper(text).await?;
        assert_eq!(result, text);
        Ok(())
    }

    #[tokio::test]
    async fn send_and_decode_empty_string_roundtrip() -> Result<()> {
        let result = chat_roundtrip_helper("").await?;
        assert_eq!(result, "");
        Ok(())
    }
}
