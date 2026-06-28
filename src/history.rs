// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Mesaj geçmişi modülü — sohbeti diske kaydet ve yükle.
//!
//! Bu modül sohbet mesajlarının persistansından sorumludur. JSON Lines
//! formatında (her satır bir JSON nesnesi) `~/.onionchat/history.jsonl`
//! dosyasına yazar. Ağ ve kriptografi detaylarına dokunmaz.
//!
//! ## Format
//!
//! Her satır bir `HistoryEntry` nesnesinin JSON serialization'ı:
//!
//! ```json
//! {"timestamp":1719600000,"kind":"sent","text":"merhaba"}
//! {"timestamp":1719600010,"kind":"received","text":"selam"}
//! {"timestamp":1719600015,"kind":"system","text":"peer bağlandı"}
//! ```
//!
//! JSON Lines seçilmesinin nedeni: append-only (yeni mesajlar dosya sonuna
//! eklenir, tüm dosyayı yeniden yazma gerekmez), satır satır okunabilir
//! (hata tolerant — bozuk satır atlanır), ve standart JSON tooling ile
//! incelenebilir.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

/// Bir geçmiş kaydı. `ChatLine`'ın serileştirilebilir hali.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    /// UNIX epoch saniyesi (UTC).
    pub timestamp: u64,
    /// Mesaj türü: "sent", "received", veya "system".
    pub kind: String,
    /// Mesaj metni.
    pub text: String,
}

impl HistoryEntry {
    /// Yeni bir kayıt oluştur. Zaman damgası otomatik `SystemTime::now()`.
    pub fn new(kind: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            kind: kind.into(),
            text: text.into(),
        }
    }

    /// `SystemTime`'a çevir (UI için).
    pub fn to_system_time(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.timestamp)
    }
}

/// Geçmiş dosyasının varsayılan yolu: `~/.onionchat/history.jsonl`.
///
/// `HOME` çevre değişkeni yoksa `./onionchat-history.jsonl`'a düşer.
pub fn default_history_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".onionchat");
        p.push("history.jsonl");
        p
    } else {
        PathBuf::from("onionchat-history.jsonl")
    }
}

/// Geçmiş dosyasını açar veya oluşturur. Üst dizin yoksa oluşturur.
///
/// Dosya append modunda açılır — yeni kayıtlar sona eklenir.
pub async fn open_for_append(path: &Path) -> Result<File> {
    // Üst dizini oluştur (yoksa).
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    Ok(file)
}

/// Bir kaydı dosyaya yazar. Append-only; mevcut içerik dokunulmaz.
///
/// Kayıt tek satır JSON olarak yazılır, sonuna `\n` eklenir.
pub async fn append_entry(file: &mut File, entry: &HistoryEntry) -> Result<()> {
    let mut json = serde_json::to_string(entry)?;
    json.push('\n');
    file.write_all(json.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

/// Tüm geçmişi dosyadan okur. Bozuk satırlar atlanır (hata tolerant).
///
/// Büyük dosyalarda tüm içeriği belleğe yükler; şu an için kabul edilebilir
/// (tipik sohbet <10K mesaj). İleride streaming okuma eklenebilir.
pub async fn load_all(path: &Path) -> Result<Vec<HistoryEntry>> {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(e) => return Err(e.into()),
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;

    let mut entries = Vec::new();
    for (lineno, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<HistoryEntry>(line) {
            Ok(e) => entries.push(e),
            Err(e) => {
                tracing::warn!("geçmiş satır {} atlandı: {}", lineno + 1, e);
            }
        }
    }
    Ok(entries)
}

/// Geçmiş dosyasını temizler (boşaltır). Dosya yoksa sessizce geçer.
pub async fn clear(path: &Path) -> Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Son N kaydı döndürür. Dosya yoksa boş vektör döner.
///
/// Tüm dosyayı okuyup son N'u döndürür — büyük dosyalar için optimize
/// edilmemiştir. Pratik kullanımda yeterli.
pub async fn load_recent(path: &Path, n: usize) -> Result<Vec<HistoryEntry>> {
    let all = load_all(path).await?;
    if all.len() <= n {
        Ok(all)
    } else {
        Ok(all[all.len() - n..].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Her test için uniq dosya adı üret.
    fn test_path() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("onionchat-history-test-{}-{}.jsonl", std::process::id(), id));
        p
    }

    #[test]
    fn history_entry_new_sets_fields() {
        let e = HistoryEntry::new("sent", "merhaba");
        assert_eq!(e.kind, "sent");
        assert_eq!(e.text, "merhaba");
        assert!(e.timestamp > 1_700_000_000); // 2023 sonrası
    }

    #[test]
    fn history_entry_serde_roundtrip() {
        let e = HistoryEntry {
            timestamp: 1719600000,
            kind: "received".to_string(),
            text: "selam".to_string(),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn history_entry_json_has_expected_fields() {
        let e = HistoryEntry::new("sent", "test");
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"timestamp\""));
        assert!(json.contains("\"kind\""));
        assert!(json.contains("\"text\""));
    }

    #[tokio::test]
    async fn append_and_load_roundtrip() -> Result<()> {
        let path = test_path();
        // Test başlangıcında dosya olmamalı.
        let _ = tokio::fs::remove_file(&path).await;

        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "merhaba")).await?;
        append_entry(&mut file, &HistoryEntry::new("received", "selam")).await?;
        append_entry(&mut file, &HistoryEntry::new("system", "bağlandı")).await?;
        drop(file);

        let loaded = load_all(&path).await?;
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].text, "merhaba");
        assert_eq!(loaded[0].kind, "sent");
        assert_eq!(loaded[1].text, "selam");
        assert_eq!(loaded[1].kind, "received");
        assert_eq!(loaded[2].text, "bağlandı");

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_nonexistent_returns_empty() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;
        let loaded = load_all(&path).await?;
        assert!(loaded.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn load_skips_corrupt_lines() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "good1")).await?;
        // Bozuk satır
        file.write_all(b"not valid json\n").await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "good2")).await?;
        // Bir başka bozuk
        file.write_all(b"{broken\n").await?;
        drop(file);

        let loaded = load_all(&path).await?;
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "good1");
        assert_eq!(loaded[1].text, "good2");

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_recent_returns_last_n() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let mut file = open_for_append(&path).await?;
        for i in 0..10 {
            append_entry(&mut file, &HistoryEntry::new("sent", format!("msg{}", i))).await?;
        }
        drop(file);

        let recent = load_recent(&path, 3).await?;
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].text, "msg7");
        assert_eq!(recent[2].text, "msg9");

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_recent_when_fewer_than_n_returns_all() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "only")).await?;
        drop(file);

        let recent = load_recent(&path, 100).await?;
        assert_eq!(recent.len(), 1);

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn clear_removes_file() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "x")).await?;
        drop(file);

        assert!(path.exists());
        clear(&path).await?;
        assert!(!path.exists());
        Ok(())
    }

    #[tokio::test]
    async fn clear_nonexistent_is_ok() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;
        clear(&path).await?; // hata vermemeli
        Ok(())
    }

    #[tokio::test]
    async fn open_for_append_creates_parent_dirs() -> Result<()> {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "onionchat-test-nested-{}-{}/sub/history.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "nested")).await?;
        drop(file);

        assert!(path.exists());
        let loaded = load_all(&path).await?;
        assert_eq!(loaded.len(), 1);

        // Cleanup
        let parent = path.parent().unwrap().parent().unwrap();
        tokio::fs::remove_dir_all(parent).await.ok();
        Ok(())
    }

    #[test]
    fn default_history_path_uses_home() {
        // HOME set edip test etmek Rust 2024'te `set_var` unsafe olduğu
        // için (Rule 1: Zero Unsafe Policy) kaçınılır. Bunun yerine
        // `var_os` ile HOME varsa default_history_path'in onu içerdiğini
        // doğrularız.
        if let Some(home) = std::env::var_os("HOME") {
            let p = default_history_path();
            let home_str = home.to_string_lossy();
            assert!(
                p.to_string_lossy().starts_with(&*home_str),
                "path should start with HOME: {} vs {}",
                p.display(),
                home_str
            );
            assert!(p.to_string_lossy().contains(".onionchat"));
            assert!(p.to_string_lossy().contains("history.jsonl"));
        }
    }

    #[test]
    fn to_system_time_returns_valid_time() {
        let e = HistoryEntry {
            timestamp: 1719600000,
            kind: "sent".to_string(),
            text: "x".to_string(),
        };
        let t = e.to_system_time();
        assert!(t.duration_since(SystemTime::UNIX_EPOCH).is_ok());
    }

    #[tokio::test]
    async fn append_and_load_turkish_text() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "Merhaba dünya")).await?;
        append_entry(&mut file, &HistoryEntry::new("received", "Nasılsın?")).await?;
        append_entry(&mut file, &HistoryEntry::new("system", "İstanbul'a bağlandı")).await?;
        drop(file);

        let loaded = load_all(&path).await?;
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].text, "Merhaba dünya");
        assert_eq!(loaded[1].text, "Nasılsın?");
        assert_eq!(loaded[2].text, "İstanbul'a bağlandı");

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn append_and_load_all_turkish_chars() -> Result<()> {
        // Tüm Türkçe karakterleri içeren mesaj
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let turkish_text = "Şş Ğğ Üü Öö Çç İı — İstanbul'da güzel bir gün";
        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", turkish_text)).await?;
        drop(file);

        let loaded = load_all(&path).await?;
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, turkish_text);

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn append_and_load_long_turkish_message() -> Result<()> {
        // Uzun Türkçe mesaj — çok baytlı karakterlerle
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let long_text = "Şükrü ".repeat(100); // ~700 bayt
        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", &long_text)).await?;
        drop(file);

        let loaded = load_all(&path).await?;
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, long_text);

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn append_and_load_turkish_with_emoji_in_text() -> Result<()> {
        // Türkçe + emoji shortcode (henüz render edilmemiş, raw text)
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let text = "Merhaba :heart: nasılsın :smile:";
        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", text)).await?;
        drop(file);

        let loaded = load_all(&path).await?;
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, text);

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn append_and_load_mixed_turkish_and_emoji_chars() -> Result<()> {
        // Türkçe + gerçek Unicode emoji karakterleri (shortcode değil)
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let text = "Selam \u{1F642} dünya \u{2764}\u{FE0F}";
        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", text)).await?;
        drop(file);

        let loaded = load_all(&path).await?;
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, text);

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_recent_turkish_messages() -> Result<()> {
        // load_recent Türkçe mesajlarla doğru çalışmalı
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let mut file = open_for_append(&path).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "bir")).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "iki")).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "Şükrü")).await?;
        append_entry(&mut file, &HistoryEntry::new("sent", "dört")).await?;
        drop(file);

        let recent = load_recent(&path, 2).await?;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text, "Şükrü");
        assert_eq!(recent[1].text, "dört");

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[test]
    fn history_entry_turkish_text_serde_roundtrip() {
        // Türkçe metin içeren HistoryEntry serde round-trip
        let entry = HistoryEntry {
            timestamp: 1719600000,
            kind: "sent".to_string(),
            text: "İstanbul'da güzel bir gün".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn history_entry_all_turkish_chars_serde() {
        let entry = HistoryEntry::new("received", "ŞşĞğÜüÖöÇçİı");
        let json = serde_json::to_string(&entry).unwrap();
        let back: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry.text, back.text);
        assert_eq!(entry.kind, back.kind);
    }
}
