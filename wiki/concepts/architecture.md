---
title: Architecture
created: 2026-06-26
updated: 2026-06-28
type: concept
tags: [modular, single-responsibility, e2ee, p2p, tokio, ratatui, chunking, socks5]
sources: [src/main.rs, src/crypto.rs, src/network.rs, src/file_transfer.rs, src/ui.rs, src/socks5.rs]
confidence: high
---

# Mimari

ShadowShare modüler bir Rust projesidir; her modülün **tek sorumluluğu** vardır
ve modüller arası sınır zorunludur ([[shadowshare]]).

## Modüller

- **`crypto`** — X25519 ECDH anahtar değişimi + AES-256-GCM şifreleme.
  Rastgele 12 bayt nonce üretir ve çıktıyı `nonce || ciphertext` olarak paketler.
- **`network`** — TCP taşıma katmanı. ECDHE el sıkışması, uzunluk önekli çerçeveleme
  (`[len: u32 BE][nonce|ciphertext]`) ve `Connection::split` ile okuma/yazma yarıları.
  `listen()` akışı her yeni bağlantı için ECDHE anahtar değişimini otomatik yapar.
  `v2` değişiklik: `Connection::connect` artık `file_transfer::send_file`'a
  `DEFAULT_CHUNK_SIZE` + `NoProgress` iletilerek çağrılıyor ([[chunked-file-transfer]]).
  `v2.1` değişiklik: `Connection::connect_via_socks5` metodu eklendi;
  `socks5` modülünü çağırarak proxy üzerinden ECDHE bağlantısı kurar ([[socks5]]).
- **`socks5`** — RFC 1928 SOCKS5 istemci tarafı el sıkışması. Tek sorumluluk:
  bir `TcpStream`'i SOCKS5 proxy üzerinden hedef adrese tünellemek. Sadece
  "no auth" + `CONNECT` + IPv4/IPv6 destekler. Detaylar için [[socks5]].
- **`chat`** — Sohbet mesaj protokolü. `[0x01][utf-8]` formatında mesajlar
  gönderir/alır. Dosya transferi (`0x02`) ile çakışmaz. Detaylar için
  [[chat-tui]]. (v0.1.3'te eklendi.)
- **`file_transfer`** — `Connection` üzerinden dosya gönderme/alma + path traversal
  koruması (`sanitize_file_name`). `v2` protokolü: chunked transfer, 64 KiB
  parçalar, `Progress` trait'i ile ilerleme callback'leri. Detaylar için
  [[chunked-file-transfer]]. `v0.1.3` değişiklik: `recv_file_after_version`
  eklendi (peek sonrası sürüm baytını atlamak için).
- **`ui`** — `ratatui` + `crossterm` tabanlı **sohbet TUI'si**. `tokio::select!`
  ile ağ ve klavye olaylarını birleştirir. UTF-8 çok baytlı karakter desteği,
  scroll, cursor movement, bağlantı kopması algılama. Detaylar için
  [[chat-tui]]. (v0.1.3'te tamamen yeniden yazıldı.)
- **`main`** — İnce CLI kabuğu; sadece komut ayrıştırır ve modüllere yönlendirir.
  `v2` değişiklik: `Listen` artık doğrudan `TcpListener` + `Connection::from_stream`
  + `recv_file` zincirini kullanıyor; `Connect` ise `Connection::connect` +
  `send_file` zincirini.
  `v2.1` değişiklik: `Connect`'e `--socks5 <addr>` bayrağı eklendi; `--anon`
  Tor varsayılan `127.0.0.1:9050`'a düşüyor. `resolve_socks5` yardımcısı.
  `v0.1.2` değişiklik: **Subcommand'lar kaldırıldı**, flat args'a geçildi.
  Artık `onionchat --listen 8080` veya `onionchat --connect 1.2.3.4:8080`
  gibi tek düzey komutlar kullanılıyor. `normalize_addr` yardımcısı ile
  port kısa formları (`8080`, `:8080`, `127.0.0.1:8080`) kabul ediliyor.
  `v0.1.3` değişiklik: **Chat TUI** eklendi. `--connect` (dosyasız) ve
  `--listen` artık etkileşimli sohbet TUI'na girer. `run_listen` peek
  ile ilk mesajı kontrol eder: `0x02` ise dosya transferi, değilse chat.
  Detaylar için [[chat-tui]].

## CLI Yapısı (v0.1.2 — kullanıcı dostu)

Önceki sürümlerde `onionchat listen --listen 8080` gibi iç içe iki
argüman gerekiyordu. `v0.1.2` ile subcommand'lar kaldırıldı; tek düzey
flat args kullanılıyor:

```bash
# Dinleyici modu — port kısa formu
onionchat --listen 8080
onionchat --listen :8080
onionchat --listen 127.0.0.1:8080

# İstemci modu — dosya gönder
onionchat --connect 1.2.3.4:8080 --file foo.txt

# İstemci modu — chat için bağlan (dosya yok, ileride)
onionchat --connect 1.2.3.4:8080

# Anonimlik (Tor SOCKS5)
onionchat --anon --connect 1.2.3.4:8080
onionchat --socks5 127.0.0.1:9050 --connect 1.2.3.4:8080

# Varsayılan (argüman yok) — TUI modu
onionchat
```

Mod seçimi:
- Sadece `--listen` → dinleyici
- Sadece `--connect` → istemci
- İkisi birden → hata
- Hiçbiri → TUI

Adres normalizasyonu (`normalize_addr` yardımcısı):
- `8080` → `0.0.0.0:8080`
- `:8080` → `0.0.0.0:8080`
- `127.0.0.1:8080` → olduğu gibi
- Geçersiz format → açık hata mesajı

## Veri Akışı (v2)

```
İstemci (connect)                          Sunucu (listen)
─────────────────                          ────────────────
TCP bağlantısı ───────────────────────────►
pub_key[32] gönder ─────────────────────►
                    pub_key[32] oku ◄──── pub_key[32] gönder
pub_key[32] oku ◄────────────────────────
AES anahtarı türet (her iki taraf)
Connection hazır
send_message(version=0x02) ─────────────►
send_message(name_len) ─────────────────►
send_message(name) ─────────────────────►
send_message(file_size: u64 BE) ────────►
send_message(chunk_size: u32 BE) ───────►
send_message(chunk_1) ───[len|nonce|ct]►
send_message(chunk_2) ───[len|nonce|ct]►
...
send_message([0u8;4] sentinel) ─────────►
```

Her mesaj AES-GCM ile şifrelendiği için ağ izleyici sadece rastgele gürültü görür.

## Splitleme Modeli

`Connection::split()` çağrıldığında `TcpStream::into_split` ile elde edilen
`OwnedReadHalf` ve `OwnedWriteHalf` iki ayrı göreve taşınır. Her görev bir
`tokio::sync::mpsc` kanalı üzerinden dış dünyayla konuşur:

- `Sender::send(vec)` → `out_tx` kanalına yaz → görev şifreler + TCP'ye yazar
- `Receiver` (Stream) ← `in_rx` kanalı ← görev TCP'den okur + çözer

Yarım dupleks modeli tek bir `Connection`'ın birden fazla async görev tarafından
güvenle paylaşılmasını sağlar.

`v2` ile birlikte bu splitleme modeli şu an `main.rs`'ta kullanılmıyor (tek
bağlantılı senaryolar yeterli); `network::listen` ve `split_halves` `#[allow(dead_code)]`
ile işaretlendi, ileride multi-peer / pool senaryoları için korunuyor.

## Bellek Profili (v2)

`v1`'de tüm dosya belleğe alınıyordu. `v2`'de sabit bellek:

- 1 × `chunk_size` bayt okuma tamponu (varsayılan 64 KiB)
- 1 × `chunk_size + 12 (nonce) + 16 (GCM tag)` bayt AES-GCM çıktısı
- 4 bayt uzunluk öneki

≈130 KiB sabit bellekle 10 GiB dosya aktarılabilir.

## Anonimlik Katmanı (v2.1)

`--anon` veya `--socks5 <addr>` bayraklarıyla `connect` SOCKS5 proxy
üzerinden peer'a bağlanır. Akış:

```text
[shadowshare connect]
       │
       │ TCP (şifresiz)
       ▼
[SOCKS5 proxy (örn. Tor 127.0.0.1:9050)]
       │
       │ TCP (şifresiz, tünel)
       ▼
[shadowshare listen] ◄── ECDHE + AES-GCM (uçtan uca şifreli)
```

SOCKS5 sadece IP gizler; veri her zaman `Connection` üzerinden ECDHE +
AES-GCM ile uçtan uca şifrelidir. Detaylar için [[socks5]].

## İlgili Sayfalar

- [[shadowshare]] — proje genel bakış
- [[chunked-file-transfer]] — `v2` protokolü detayları
- [[socks5]] — SOCKS5 anonimlik katmanı
- [[bug-fixes-2026-06-26]] — ilk iskelet düzeltmeleri
- [[bug-fixes-2026-06-28]] — `v2` geçişi düzeltmeleri
- [[bug-fixes-2026-06-28-v2]] — `v2.1` SOCKS5 entegrasyonu düzeltmeleri
