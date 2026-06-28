---
title: Bug Fixes — 2026-06-26
created: 2026-06-26
updated: 2026-06-26
type: concept
tags: [bug-fix, cargo-check, e2ee, path-traversal, split, listener]
sources: [src/main.rs, src/crypto.rs, src/network.rs, src/file_transfer.rs, src/ui.rs]
---

# Bug Fixes — 2026-06-26

İlk `cargo check` 11 hata + 11 uyarı ile başarısız oldu. Aşağıdaki düzeltmeler
bu oturumda uygulandı.

## Hatalar

### 1. `crypto::encrypt/decrypt` AES-GCM tip çıkarımı
- **Sorun**: `Key::<Aes256Gcm>::from_slice` + `Nonce::from_slice` zinciri
  `generic_array::ArrayLength` için tip çıkarımı yapamadı (`E0283`/`E0284`).
- **Çözüm**: `Aes256Gcm::new_from_slice(key)` (doğrudan `&[u8]` alır) + `Nonce::from_slice(&[u8;12])` kullanıldı.
- **Dosya**: `src/crypto.rs`

### 2. `network::from_stream` çift ref
- **Sorun**: `stream.write_all(&our_public.as_bytes())` `&&[u8;32]` üretiyordu (`E0308`).
  `PublicKey::from(&their_pubkey)` trait bound hatası (`E0277`).
  `compute_shared_secret(&our_secret, ...)` gereksiz borrow (`E0308`).
- **Çözüm**: `as_bytes()` çağrısı kaldırıldı (zaten `[u8;32]` döner),
  `PublicKey::from(their_pubkey)` (by-value), `our_secret` borrow'suz taşındı.
- **Dosya**: `src/network.rs`

### 3. `network::connect` adres uyuşmazlığı
- **Sorun**: `main.rs` `network::connect(addr, file)` çağırıyordu ama modülde
  böyle bir serbest fonksiyon yoktu; sadece `Connection::connect` vardı (`E0425`).
- **Çözüm**: `main.rs` `Connection::connect(addr, Some(file))` kullanacak şekilde
  güncellendi. CLI davranışı değişmedi (dosya gönderimi aynı şekilde yapılıyor).
- **Dosya**: `src/main.rs`

### 4. `TcpListenerStream` adaptor + buffer_unordered
- **Sorun**: `tokio_stream::wrappers::TcpListenerStream` doğrudan bir `Stream`
  değil; `.map(|r| async move { ... })` zinciri `buffer_unordered` için `futures::StreamExt`
  trait'ini gerektiriyordu.
- **Çözüm**: `use futures::{Stream, StreamExt};` eklendi.
- **Dosya**: `src/network.rs`

### 5. `ui::run` 5 argüman bekliyordu, 4 geçildi
- **Sorun**: `ui::run`'a `anon` boolean'ı eklenmişti ama `main.rs` hala 4
  argümanla çağırıyordu (`E0061`).
- **Çözüm**: `main.rs` 5. argüman olarak `anon`'u geçti.
- **Dosya**: `src/main.rs`

### 6. `mpsc::channel` tip çıkarımı
- **Sorun**: İkinci `mpsc::channel(32)` çağrısı (`tx2`/`rx2` kullanılmayan)
  tip çıkarımı yapamadı (`E0282`).
- **Çözüm**: Kullanılmayan `tx2`/`rx2` kaldırıldı; sadece tek yarım dupleks
  kanal çifti tutuldu.
- **Dosya**: `src/network.rs`

### 7. Çift `UiMode` tipi
- **Sorun**: Hem `main.rs` hem `ui.rs` `UiMode` enum'u tanımlıyordu; derleyici
  hangisinin kullanılacağını çözemedi (`E0308`).
- **Çözüm**: `ui.rs`'deki `pub enum UiMode` korundu, `main.rs`'deki
  `enum UiMode` → `enum CliUiMode` olarak yeniden adlandırıldı ve
  `From<CliUiMode> for ui::UiMode` dönüşümü eklendi.
- **Dosya**: `src/main.rs`

### 8. `Frame::area()` ratatui 0.20'de yok
- **Sorun**: `f.area()` ratatui 0.23+ API'si; 0.20'de `f.size()` kullanılmalı (`E0599`).
- **Çözüm**: `f.size()` ile değiştirildi.
- **Dosya**: `src/ui.rs`

### 9. `ratatui::text::Line` 0.20'de yok
- **Sorun**: `Line` tipi ratatui 0.21+'te eklendi (`E0433`).
- **Çözüm**: `Paragraph::new(String)` ile string-tabanlı içerik kullanıldı;
  `Span` zaten 0.20'de var, gerek kalmadı.
- **Dosya**: `src/ui.rs`

### 10. `TcpStream` aynı anda iki görevde kullanılamadı
- **Sorun**: `split()` iki `tokio::spawn` görevi arasında `stream`'i
  paylaşmaya çalışıyordu ama `TcpStream` tek sahipli (`E0382`).
- **Çözüm**: `Connection::split` artık `stream.into_split()` ile
  `OwnedReadHalf`/`OwnedWriteHalf` üretip iki ayrı göreve taşıyor;
  mesajlar `mpsc` kanalları üzerinden geçiriliyor.
- **Dosya**: `src/network.rs`

### 11. `network::listen` Stream'i polled olmalı
- **Sorun**: `network::listen(addr).await?;` ifadesi `must_use` uyarısı
  veriyordu (`unused_must_use`).
- **Çözüm**: `main.rs`'te akış pin'lenip ilk elemanı poll ediliyor;
  bu sayede dinleyici gerçekten en az bir bağlantıyı kabul ediyor.
- **Dosya**: `src/main.rs`

## Uyarılar (temizlendi)

- Kullanılmayan importlar (`anyhow`, `async_trait`, `tokio::fs`, `PublicKey`,
  `TcpStream`) kaldırıldı.
- `compute_shared_secret` `mut secret` → `secret` (gereksiz mutable).
- `Receiver::poll_next` içindeki `cx` artık `_cx` (kullanılmıyor; gerçek
  implementasyon `mpsc::Receiver::poll_recv`'a delege ediyor).
- `ui::run`'daki kullanılmayan `anon` parametresi `_anon` yapıldı.
- `generate_room_key`, `Connection::split`, `split_connection`, `Sender::send`
  ve `Sender::tx` `#[allow(dead_code)]` ile işaretlendi (ileride kullanılacak
  public API).

## Yeni Özellikler

- **`file_transfer::send_file`/`recv_file`** — `Connection` üzerinden dosya
  alışverişi. `recv_file` gelen dosya adını `sanitize_file_name` ile doğrular.
- **`sanitize_file_name`** — Path traversal koruması: boş isim, `.`, `..`,
  ayraç (`/`, `\`) içeren isimler reddedilir.
- **`split_halves` + `split_connection`** — `OwnedReadHalf`/`OwnedWriteHalf`
  çiftini mpsc kanalları ile sarmalayarak `Sender`/`Receiver` üreten yardımcılar.
- **Üç entegrasyon testi**: `network::test_message_round_trip`,
  `network::test_split_round_trip`, `file_transfer::sanitize_*` testleri.

## Doğrulama

- `cargo check`: 0 hata, 0 uyarı
- `cargo test`: 4/4 geçti
- E2E smoke test: `shadowshare listen --listen 127.0.0.1:18888` ile
  `shadowshare connect --connect 127.0.0.1:18888 --file /tmp/x` çalıştırıldı,
  sunucu ilk mesajı (dosya adı, 15 bayt) başarıyla aldı.

## İlgili Sayfalar

- [[architecture]] — güncel modül yapısı
- [[shadowshare]] — proje genel bakış
