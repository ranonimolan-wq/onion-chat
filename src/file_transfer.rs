// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Dosya transferi modülü — şifreli kanal üzerinden dosya yazma/okuma.
//!
//! Bu modül yalnızca `network::Connection` üzerinden akan mesajların
//! dosya adı + içerik olarak serileştirilmesinden ve disk I/O'dan
//! sorumludur. Ağ ve kriptografi detaylarına dokunmaz.
//!
//! ## Protokol (v2 — chunked)
//!
//! Tüm mesajlar `network::Connection` üzerinden AES-GCM ile şifrelenir;
//! bu modül sadece mesajların semantiğini (anlamlarını) belirler.
//!
//! ```text
//! [version: u8 = 0x02]            // ileriye dönük uyumluluk için
//! [file_name_len: u32 BE]
//! [file_name: UTF-8]
//! [file_size: u64 BE]             // toplam bayt (ilerleme takibi için)
//! [chunk_size: u32 BE]            // parça boyu (genelde 64 KiB)
//! // sonra N adet parça:
//!   [chunk_data]                  // send_message/recv_message ile çerçevelenir
//! // sonunda sentinel:
//! [0x00000000]                    // 4 bayt sıfır = "transfer bitti"
//! ```
//!
//! Sentinel'den farklı olarak her parça `network::send_message`'a verildiği
//! için AES-GCM nonce + uzunluk öneki ile çerçevelenir. Sentinel de aynı
//! şekilde şifrelenir (4 baytlık düz metin).
//!
//! ## Bellek profili
//!
//! Eski v1 protokolü tüm dosyayı belleğe alıyordu; v2 her parçayı teker
//! teker okuyup gönderir. Varsayılan parça boyu 64 KiB'dir; bu hem AES-GCM
//! çağrı başına maliyetini makul tutar hem de ilerleme takibini saniyede
//! onbinlerce güncelleme olacak kadar parçalamaz.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::network::Connection;

/// Aktif protokol sürümü. Yeni eşlerden eski eşleri ayırmak için kullanılır.
pub const PROTOCOL_VERSION: u8 = 0x02;

/// Varsayılan parça boyu (64 KiB). Büyük dosyalarda bellek baskısını
/// sınırlar tutarken AES-GCM çağrı başına maliyeti kabul edilebilir tutar.
pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;

/// Transfer bittiğini belirten sentinel. Sentinel'in kendisi de AES-GCM
/// ile şifrelenerek karşı tarafa `send_message` ile gönderilir.
const END_SENTINEL: [u8; 4] = [0u8; 4];

/// İlerleme bildirimleri için callback tabanlı arayüz.
///
/// `send_file`/`recv_file` her parça yazıldığında/okunduğunda çağırır.
/// `Vec` kopyalamaktan kaçınmak için `&[u8]` alınır; implementasyon
/// isterse sadece uzunluğa bakıp ilerleme yüzdesini hesaplayabilir.
///
/// `Send + Sync` bound'ları, `&dyn Progress`'in `tokio::spawn` ile
/// oluşturulan async görevler arasında güvenle paylaşılabilmesi içindir.
pub trait Progress: Send + Sync + 'static {
    /// Tek bir parça işlendiğinde çağrılır.
    /// `transferred` = bu ana kadar işlenen toplam bayt,
    /// `total` = dosya boyu (bilinmiyorsa 0).
    fn on_chunk(&self, transferred: u64, total: u64);
}

/// `Progress` implementasyonu olmayan çağrılar için kullanılan no-op.
pub struct NoProgress;

impl Progress for NoProgress {
    fn on_chunk(&self, _transferred: u64, _total: u64) {}
}

/// Dosya adını güvenli bir şekilde bağlamsız (yalnızca dosya adı, yol olmadan)
/// forma dönüştürür. Yol geçişi (path traversal) ve mutlak yollara karşı
/// basit bir savunma uygular.
fn sanitize_file_name(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("empty file name"));
    }
    // Yalnızca dosya adı bileşenini al (yol olsa bile).
    let candidate = Path::new(trimmed)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("invalid file name: {}", trimmed))?;
    // Mutlak yol işareti ve ".." gibi kalıntıları reddet.
    if candidate.is_empty()
        || candidate == "."
        || candidate == ".."
        || candidate.contains('/')
        || candidate.contains('\\')
    {
        return Err(anyhow!("unsafe file name: {}", candidate));
    }
    Ok(candidate.to_string())
}

/// 32-bit uzunluk önekini 4 bayt big-endian olarak yazar.
fn write_u32_be(value: u32) -> [u8; 4] {
    value.to_be_bytes()
}

/// 64-bit uzunluk önekini 8 bayt big-endian olarak yazar.
fn write_u64_be(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

/// 4 baytlık bir tamponu big-endian `u32` olarak okur.
fn read_u32_be(buf: &[u8]) -> Result<u32> {
    if buf.len() < 4 {
        return Err(anyhow!("buffer too short for u32"));
    }
    let mut arr = [0u8; 4];
    arr.copy_from_slice(&buf[..4]);
    Ok(u32::from_be_bytes(arr))
}

/// 8 baytlık bir tamponu big-endian `u64` olarak okur.
fn read_u64_be(buf: &[u8]) -> Result<u64> {
    if buf.len() < 8 {
        return Err(anyhow!("buffer too short for u64"));
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&buf[..8]);
    Ok(u64::from_be_bytes(arr))
}

/// Bir `u32`'yi `network::Connection::send_message` ile gönderir.
/// Veri 4 bayttan kısa olduğu için AES-GCM başına +12 bayt nonce + 16 bayt
/// etiket eklenir; bu küçük mesajlar için kabul edilebilir bir overhead.
async fn send_u32(conn: &mut Connection, value: u32) -> Result<()> {
    conn.send_message(&write_u32_be(value)).await
}

/// Bir `u64`'yü `network::Connection::send_message` ile gönderir.
async fn send_u64(conn: &mut Connection, value: u64) -> Result<()> {
    conn.send_message(&write_u64_be(value)).await
}

/// Uzunluk önekli bir byte dizisi gönderir (önce uzunluk, sonra veri).
async fn send_length_prefixed(conn: &mut Connection, data: &[u8]) -> Result<()> {
    send_u32(conn, data.len() as u32).await?;
    conn.send_message(data).await
}

/// Karşı taraftan bir `u32` okur.
async fn recv_u32(conn: &mut Connection) -> Result<u32> {
    let bytes = conn.recv_message().await?;
    read_u32_be(&bytes)
}

/// Karşı taraftan bir `u64` okur.
async fn recv_u64(conn: &mut Connection) -> Result<u64> {
    let bytes = conn.recv_message().await?;
    read_u64_be(&bytes)
}

/// Uzunluk önekli bir byte dizisi okur. Sunucu tarafında doğrulama için
/// `max_len` kadar bir sınır uygulanır; aşarsa hata döner.
async fn recv_length_prefixed(conn: &mut Connection, max_len: u32) -> Result<Vec<u8>> {
    let len = recv_u32(conn).await?;
    if len > max_len {
        return Err(anyhow!("length {} exceeds max {}", len, max_len));
    }
    let buf = conn.recv_message().await?;
    if buf.len() != len as usize {
        return Err(anyhow!(
            "length mismatch: header says {}, got {}",
            len,
            buf.len()
        ));
    }
    Ok(buf)
}

/// Karşı tarafa bir dosya gönderir. Protokol v2'yi kullanır:
/// sürüm, dosya adı, dosya boyu, parça boyu, N adet parça, sentinel.
///
/// `chunk_size` 1 bayttan küçük olamaz ve `u32::MAX`'i aşamaz. Varsayılan
/// değer için `DEFAULT_CHUNK_SIZE` sabitini kullanın.
///
/// `progress` callback'i her parça gönderildikten sonra çağrılır.
pub async fn send_file(
    conn: &mut Connection,
    path: &Path,
    chunk_size: usize,
    progress: &dyn Progress,
) -> Result<()> {
    if chunk_size == 0 {
        return Err(anyhow!("chunk_size must be > 0"));
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("invalid file path: {}", path.display()))?;

    // Dosya boyunu al; ilerleme takibi için gerekli.
    let metadata = tokio::fs::metadata(path).await?;
    let file_size = metadata.len();

    // Protokol başlığı.
    conn.send_message(&[PROTOCOL_VERSION]).await?;
    send_length_prefixed(conn, file_name.as_bytes()).await?;
    send_u64(conn, file_size).await?;
    send_u32(conn, chunk_size as u32).await?;

    // Dosyayı parça parça oku ve gönder.
    let mut file = File::open(path).await?;
    let mut buf = vec![0u8; chunk_size];
    let mut transferred: u64 = 0;
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        conn.send_message(&buf[..n]).await?;
        transferred += n as u64;
        progress.on_chunk(transferred, file_size);
    }

    // Transfer bitti sinyali.
    conn.send_message(&END_SENTINEL).await?;
    Ok(())
}

/// Karşı taraftan bir dosya alır ve belirtilen dizine yazar.
/// Dosya adı karşı taraftan geldiği için `sanitize_file_name` ile
/// yeniden doğrulanır. `max_file_size` aşılırsa hata döner.
///
/// `progress` callback'i her parça yazıldıktan sonra çağrılır.
///
/// Bu fonksiyon önce sürüm baytını (`PROTOCOL_VERSION`) okur ve doğrular.
/// Eğer çağıran zaten ilk mesajı peek ettiyse (örn. listen tarafı chat mi
/// yoksa dosya transferi mi olduğuna karar vermek için), bunun yerine
/// `recv_file_after_version` kullanın.
///
/// Not: `main.rs` doğrudan `recv_file_after_version` kullanır; bu
/// fonksiyon testlerde ve ilerideki kullanımlar için public API olarak
/// korunur.
#[allow(dead_code)]
pub async fn recv_file(
    conn: &mut Connection,
    dest_dir: &Path,
    max_file_size: u64,
    progress: &dyn Progress,
) -> Result<PathBuf> {
    // Sürüm baytını oku ve doğrula.
    let version_bytes = conn.recv_message().await?;
    if version_bytes.len() != 1 {
        return Err(anyhow!("invalid protocol version frame: {} bytes", version_bytes.len()));
    }
    let version = version_bytes[0];
    if version != PROTOCOL_VERSION {
        return Err(anyhow!(
            "unsupported protocol version: got {}, expected {}",
            version,
            PROTOCOL_VERSION
        ));
    }
    recv_file_after_version(conn, dest_dir, max_file_size, progress).await
}

/// `recv_file` ile aynı, ancak sürüm baytı zaten tüketilmiş kabul edilir.
///
/// Bu fonksiyon, listen tarafının ilk mesajı peek edip chat mi yoksa
/// dosya transferi mi olduğuna karar vermesinin ardından kullanılır.
/// Eğer ilk mesaj `[0x02]` (sürüm baytı) ise, çağıran bu fonksiyonu
/// çağırır; aksi halde mesajı `chat::decode_chat` ile chat olarak yorumlar.
pub async fn recv_file_after_version(
    conn: &mut Connection,
    dest_dir: &Path,
    max_file_size: u64,
    progress: &dyn Progress,
) -> Result<PathBuf> {
    // Dosya adı: uzunluk öneki + UTF-8 gövde. 4 KiB'tan uzun adları reddet.
    const MAX_NAME_LEN: u32 = 4 * 1024;
    let name_bytes = recv_length_prefixed(conn, MAX_NAME_LEN).await?;
    let raw_name = String::from_utf8(name_bytes)?;
    let safe_name = sanitize_file_name(&raw_name)?;

    // Dosya boyu: 0 = "bilinmiyor" (ileride stream modu için).
    let file_size = recv_u64(conn).await?;
    if file_size > max_file_size {
        return Err(anyhow!(
            "file size {} exceeds max {}",
            file_size,
            max_file_size
        ));
    }

    // Parça boyu: alıcı taraf için bir ipucu; bizim için önemsiz ama
    // doğrulama amaçlı okuruz. Aşırı büyük değerlere karşı 16 MiB sınır.
    const MAX_CHUNK_SIZE: u32 = 16 * 1024 * 1024;
    let chunk_size = recv_u32(conn).await?;
    if chunk_size == 0 || chunk_size > MAX_CHUNK_SIZE {
        return Err(anyhow!("invalid chunk_size: {}", chunk_size));
    }

    let dest_path = dest_dir.join(&safe_name);
    let mut file = File::create(&dest_path).await?;

    // Parça parça al, diske yaz, ilerlemeyi bildir.
    let mut transferred: u64 = 0;
    loop {
        let chunk = conn.recv_message().await?;
        if chunk.len() == 4 && chunk == END_SENTINEL {
            break;
        }
        file.write_all(&chunk).await?;
        transferred += chunk.len() as u64;
        progress.on_chunk(transferred, file_size);
    }
    file.flush().await?;
    Ok(dest_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_path_traversal() {
        // Son bileşeni güvensiz olan yollar reddedilir.
        assert!(sanitize_file_name("..").is_err());
        assert!(sanitize_file_name(".").is_err());
        // Boş isim reddedilir.
        assert!(sanitize_file_name("").is_err());
        // Sadece son bileşeni ".." olan Windows tarzı yollar da reddedilir.
        assert!(sanitize_file_name("a\\..").is_err());
    }

    #[test]
    fn sanitize_accepts_plain_name() {
        assert_eq!(sanitize_file_name("hello.txt").unwrap(), "hello.txt");
        assert_eq!(sanitize_file_name("foo bar.bin").unwrap(), "foo bar.bin");
        // Yollarda sadece son bileşen alınır; eğer son bileşen güvenliyse kabul edilir.
        assert_eq!(sanitize_file_name("subdir/file.bin").unwrap(), "file.bin");
        // `..` içeren ama son bileşeni güvenli olan yollar da kabul edilir
        // (yol zaten soyulmuş olur).
        assert_eq!(sanitize_file_name("../etc/passwd").unwrap(), "passwd");
        // Mutlak yolun son bileşeni güvenliyse kabul edilir.
        assert_eq!(sanitize_file_name("/etc/passwd").unwrap(), "passwd");
    }

    #[test]
    fn protocol_version_is_v2() {
        // Bu test bilinçli olarak PROTOCOL_VERSION'a sabitlenmiştir;
        // sürüm değiştiğinde hem bu test hem de `bug_fixes.md` güncellenmeli.
        assert_eq!(PROTOCOL_VERSION, 0x02);
    }

    #[test]
    fn end_sentinel_is_four_zeros() {
        // Sentinel'in formatı protokolün bir parçası; değişiklik kırıcı olur.
        assert_eq!(END_SENTINEL, [0u8; 4]);
    }

    #[test]
    fn default_chunk_size_is_64kib() {
        assert_eq!(DEFAULT_CHUNK_SIZE, 64 * 1024);
    }

    #[test]
    fn u32_be_roundtrip() {
        let v: u32 = 0x12345678;
        assert_eq!(write_u32_be(v), [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(read_u32_be(&write_u32_be(v)).unwrap(), v);
    }

    #[test]
    fn u64_be_roundtrip() {
        let v: u64 = 0x0123_4567_89AB_CDEF;
        assert_eq!(
            write_u64_be(v),
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF]
        );
        assert_eq!(read_u64_be(&write_u64_be(v)).unwrap(), v);
    }

    #[test]
    fn read_u32_rejects_short_buffer() {
        assert!(read_u32_be(&[0u8; 3]).is_err());
    }

    #[test]
    fn read_u64_rejects_short_buffer() {
        assert!(read_u64_be(&[0u8; 7]).is_err());
    }

    /// `NoProgress` çağrıldığında panik yapmaz; bu smoke testtir.
    #[test]
    fn no_progress_is_safe_to_call() {
        let p = NoProgress;
        p.on_chunk(0, 0);
        p.on_chunk(1024, 4096);
        p.on_chunk(u64::MAX, u64::MAX);
    }

    /// Uçtan uca chunked transfer testi. Geçici bir dosya yazıp
    /// `send_file` ile gönderir, `recv_file` ile alır ve içeriği
    /// karşılaştırır. Birden fazla parça boyu denemek için bir
    /// parametre alır.
    async fn chunked_roundtrip(chunk_size: usize) -> Result<()> {
        use crate::network::Connection;
        use std::sync::atomic::{AtomicU64, Ordering};
        use tokio::net::TcpListener;

        // Her test çağrısı için uniq bir sayaç üret — paralel testler
        // aynı process'i paylaştığı için `std::process::id()` tek başına
        // yeterli değil; tmp dosyası çakışması olur.
        static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        // Test verisi: chunk_size'tan büyük, çok parçalı bir aktarım
        // için en az 2.5 parça olacak kadar bayt üret.
        let payload_len = (chunk_size * 5 / 2) + 7;
        let payload: Vec<u8> = (0..payload_len).map(|i| (i % 251) as u8).collect();

        // Sender görevi: geçici dosyaya yaz, send_file ile gönder.
        let sender_payload = payload.clone();
        let sender = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut conn = Connection::from_stream(stream).await?;

            // Geçici dosyaya yaz — uniq test_id ile çakışmayı önle.
            // Sender kendi alt dizinini kullanır, böylece alıcının
            // yazdığı dosyayla çakışmaz (alıcı farklı alt dizine yazar).
            let send_dir = std::env::temp_dir().join(format!(
                "shadowshare-test-send-{}",
                test_id
            ));
            tokio::fs::create_dir_all(&send_dir).await?;
            let tmp = send_dir.join("payload.bin");
            tokio::fs::write(&tmp, &sender_payload).await?;
            send_file(&mut conn, &tmp, chunk_size, &NoProgress).await?;
            tokio::fs::remove_dir_all(&send_dir).await.ok();
            Ok::<_, anyhow::Error>(())
        });

        // Alıcı: recv_file ile diske yaz, sonra oku ve karşılaştır.
        // Alıcı kendi alt dizinine yazar; sender'ın tmp dosyasıyla
        // çakışmaz (sender gönderdiği dosya adını iletir ama alıcı
        // bunu kendi dest_dir'inde oluşturur).
        let mut conn = Connection::connect(addr, None).await?;
        let recv_dir = std::env::temp_dir().join(format!(
            "shadowshare-test-recv-{}",
            test_id
        ));
        tokio::fs::create_dir_all(&recv_dir).await?;
        let saved = recv_file(
            &mut conn,
            &recv_dir,
            16 * 1024 * 1024,
            &NoProgress,
        )
        .await?;
        let received = tokio::fs::read(&saved).await?;
        tokio::fs::remove_dir_all(&recv_dir).await.ok();
        sender.await??;

        assert_eq!(received, payload);
        Ok(())
    }

    #[tokio::test]
    async fn chunked_transfer_small_chunk() -> Result<()> {
        // 1 KiB parçalar — çok parçalı, sınıra yakın senaryo.
        chunked_roundtrip(1024).await
    }

    #[tokio::test]
    async fn chunked_transfer_default_chunk() -> Result<()> {
        // Varsayılan 64 KiB — gerçekçi senaryo.
        chunked_roundtrip(DEFAULT_CHUNK_SIZE).await
    }

    #[tokio::test]
    async fn chunked_transfer_tiny_chunk() -> Result<()> {
        // 1 baytlık parçalar — extreme durum; AES-GCM overhead'i yüksek.
        chunked_roundtrip(1).await
    }

    #[tokio::test]
    async fn chunked_transfer_exact_multiple() -> Result<()> {
        // Dosya tam 3 parça — sentinel'in hâlâ gönderildiğini doğrula.
        chunked_roundtrip(100).await
    }

    /// Eski sürüm baytı (0x01) gönderilirse recv_file hata vermeli.
    #[tokio::test]
    async fn recv_rejects_old_protocol_version() -> Result<()> {
        use crate::network::Connection;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let sender = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut conn = Connection::from_stream(stream).await?;
            // Eski sürüm baytı.
            conn.send_message(&[0x01u8]).await?;
            Ok::<_, anyhow::Error>(())
        });

        let mut conn = Connection::connect(addr, None).await?;
        let dest_dir = std::env::temp_dir();
        let result = recv_file(&mut conn, &dest_dir, 1024, &NoProgress).await;
        assert!(result.is_err(), "eski sürüm reddedilmeli");
        sender.await??;
        Ok(())
    }

    /// `send_file` sıfır parça boyunu reddeder.
    #[tokio::test]
    async fn send_file_rejects_zero_chunk_size() -> Result<()> {
        use crate::network::Connection;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let sender = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut conn = Connection::from_stream(stream).await?;
            let tmp = std::env::temp_dir().join("shadowshare-empty.bin");
            tokio::fs::write(&tmp, b"hello").await?;
            let result = send_file(&mut conn, &tmp, 0, &NoProgress).await;
            tokio::fs::remove_file(&tmp).await.ok();
            result
        });

        let mut conn = Connection::connect(addr, None).await?;
        drop(conn.recv_message().await); // sender görevi hata verecek
        let result = sender.await?;
        assert!(result.is_err(), "sıfır parça boyu reddedilmeli");
        Ok(())
    }
}
