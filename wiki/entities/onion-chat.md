---
title: OnionChat
created: 2026-06-26
updated: 2026-06-28
type: entity
tags: [p2p, e2ee, chat, tor, anonymity, file-transfer, rust-2024]
sources: [raw/articles/onion-chat-overview.md]
---

# OnionChat

Tor destekli P2P (peer-to-peer) uçtan uca şifreli (E2EE) çoklu-kullanıcı
sohbet uygulaması. Dosya transferi ek özellik olarak sunulur.

## Kimlik

OnionChat, kullanıcıların doğrudan IP'lerini ifşa etmeden, Tor ağı
üzerinden (varsayılan SOCKS5 endpoint `127.0.0.1:9050`) birbirleriyle
şifreli sohbet kurmasını amaçlar. Mimari P2P'dir — merkezi bir sunucu
yoktur; her peer hem istemci hem sunucu olabilir.

## Öncelikli Özellikler

1. **Çoklu-kullanıcı sohbet** — Birden fazla peer aynı oturuma katılabilir.
   (Henüz iskelet aşamasında; mevcut `v0.1.2` tek bağlantıyı destekler.)
2. **E2EE** — X25519 ECDHE anahtar değişimi + AES-256-GCM şifreleme.
   Tüm mesajlar `network::Connection` üzerinden şifreli çerçevelenir.
3. **Tor anonimliği** — `--anon` veya `--socks5 <addr>` bayrakları ile
   SOCKS5 proxy üzerinden bağlantı ([[socks5]]).

## Ek Özellikler

- **Dosya transferi** — `file_transfer` modülü üzerinden chunked v2
  protokolü. 64 KiB parçalar, `Progress` trait ile ilerleme takibi
  altyapısı. Detaylar için [[chunked-file-transfer]].

## Modüler Mimari

[[architecture]] sayfasında detaylı açıklama. Kısa özet:

- `crypto` — X25519 + AES-256-GCM
- `network` — TCP + ECDHE + length-prefixed framing + `split()`
- `socks5` — RFC 1928 istemci tarafı (anonimlik transportu)
- `file_transfer` — v2 chunked transfer + path traversal koruması
- `ui` — ratatui + crossterm TUI iskeleti
- `main` — ince CLI kabuğu

## Yol Haritası

- **v0.1.2 (mevcut)** — İskelet: crypto + network + socks5 + file_transfer
  + ui. Tek bağlantılı senaryolar E2E test edildi.
- **v0.2 (planlı)** — Çoklu-kullanıcı sohbet: `chat` modülü, message
  broadcast, kullanıcı listesi TUI'da.
- **v0.3 (planlı)** — Tor hidden service entegrasyonu (sunucu tarafı
  anonimlik için `arti-client` değerlendirme altında).
- **v0.4 (planlı)** — Resumable transfer (v3 protokol), SHA-256 manifest.

## İsim Geçmişi

Proje daha önce "ShadowShare" adıyla geliştiriliyordu ve odak noktası
dosya transferi idi. 2026-06-28 tarihinde odak **sohbet uygulamalarına**
kaydırıldı ve proje **"OnionChat"** olarak yeniden adlandırıldı. Dosya
transferi ek özellik olarak korundu. Eski wiki sayfalarındaki
"ShadowShare" referansları tarihsel kayıt olarak bırakıldı.

## İlgili Sayfalar

- [[architecture]] — modüler mimari
- [[socks5]] — anonimlik transportu
- [[chunked-file-transfer]] — dosya transfer protokolü
- [[bug-fixes-2026-06-28-v2]] — son isim-değişikliği öncesi oturum
