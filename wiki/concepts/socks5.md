---
title: SOCKS5 Client (Anonymity Transport)
created: 2026-06-28
updated: 2026-06-28
type: concept
tags: [socks5, tor, anonymity, transport, rfc1928, nat-traversal, rust-2024]
sources: [src/socks5.rs, src/network.rs, src/main.rs]
confidence: high
---

# SOCKS5 Client (Anonymity Transport)

ShadowShare'ın `--anon` / `--socks5 <addr>` bayrakları SOCKS5 proxy
üzerinden peer'a bağlanmayı sağlar. Tipik kullanım: Tor'un
`127.0.0.1:9050` SOCKS5 endpoint'i üzerinden bağlanmak, böylece
peer'ın gördüğü IP'yi gizlemek.

## Mimari Yerleşim

```
[ shadowshare connect ] ──TCP──> [ SOCKS5 proxy ] ──TCP──> [ shadowshare listen ]
   (socks5.rs + network.rs)        (Tor / I2P / diğer)        (network.rs)
```

- `socks5.rs` — RFC 1928 istemci tarafı el sıkışması. Tek sorumluluk:
  bir `TcpStream`'i proxy üzerinden hedefe tünellemek.
- `network.rs` — `Connection::connect_via_socks5` metodu: önce SOCKS5
  el sıkışması, sonra normal ECDHE + AES-GCM.
- `main.rs` — `--anon` ve `--socks5 <addr>` CLI bayrakları.

Modül sınırları ([[architecture]]) korunmuştur: `socks5.rs` bağımsız bir
modül, `network.rs`'a entegre edilirken mevcut kod değiştirilmedi.

## Protokol Detayları (RFC 1928)

### 1. Metot Anlaşması

```text
İstemci → Proxy: [VER=0x05, NMETHODS=1, METHODS=[0x00]]
Proxy → İstemci: [VER=0x05, METHOD=0x00]
```

ShadowShare yalnızca "no authentication required" (`0x00`) teklif eder.
Proxy reddederse (METHOD = `0xFF`) bağlantı hata ile sonlanır.

### 2. CONNECT İsteği

```text
İstemci → Proxy:
  [VER=0x05, CMD=0x01, RSV=0x00, ATYP, DST.ADDR, DST.PORT]
```

- `ATYP = 0x01` (IPv4): 4 bayt adres
- `ATYP = 0x04` (IPv6): 16 bayt adres
- `ATYP = 0x03` (domain): ShadowShare kullanmaz (SocketAddr zaten IP)

### 3. Yanıt

```text
Proxy → İstemci:
  [VER=0x05, REP, RSV=0x00, ATYP, BND.ADDR, BND.PORT]
```

`REP = 0x00` ise başarılı. Diğer değerler RFC 1928 §5'teki hata
kodlarına göre `reply_message()` ile okuyucu dostu metne çevrilir:

| REP | Mesaj |
|-----|-------|
| 0x00 | succeeded |
| 0x01 | general SOCKS server failure |
| 0x02 | connection not allowed by ruleset |
| 0x03 | network unreachable |
| 0x04 | host unreachable |
| 0x05 | connection refused |
| 0x06 | TTL expired |
| 0x07 | command not supported |
| 0x08 | address type not supported |

## Güvenlik Modeli

SOCKS5 trafiği **şifresizdir** — istemci ↔ proxy arasındaki bağlantı
açık metindir. Bu nedenle:

- ✅ SOCKS5 sadece **peer'ın IP'sini gizler** (anonymity)
- ❌ SOCKS5 veriyi şifrelemez (E2EE değildir)
- ✅ Asıl veri aktarımı her zaman `Connection` üzerinden ECDHE + AES-GCM
  ile uçtan uca şifrelidir

Tor'un `127.0.0.1:9050` endpoint'i localhost'ta olduğu için bu
şifresizlik pratik bir sorun yaratmaz. Uzak SOCKS5 proxy kullanılıyorsa
kullanıcı kendi sorumluluğundadır.

## CLI Kullanımı

```bash
# 1. Direkt TCP (SOCKS5 yok)
shadowshare connect --connect 1.2.3.4:8080 --file /tmp/x

# 2. Tor varsayılanı (127.0.0.1:9050)
shadowshare connect --connect 1.2.3.4:8080 --file /tmp/x --anon

# 3. Özel SOCKS5 adresi
shadowshare connect --connect 1.2.3.4:8080 --file /tmp/x --socks5 127.0.0.1:1080
```

`--anon` ve `--socks5` birlikte verilirse `--socks5` önceliklidir.
`--anon` tek başına verilirse Tor varsayılanına düşülür.

## Sunucu Tarafı Notu

`listen` alt komutu SOCKS5 desteklemez — bir dinleyici SOCKS5 üzerinden
gelen bağlantıları kabul edemez (SOCKS5 istemci-taraflı bir protokoldür).
İleride Tor hidden service entegrasyonu için ayrı bir modül eklenebilir.

## Test Kapsamı

`socks5::tests` modülünde:

- `constants_match_rfc1928` — sabit değerler RFC ile uyumlu
- `reply_message_covers_known_codes` — tüm hata kodları çevrilir
- `socks5_handshake_round_trip` — uçtan uca başarılı el sıkışma
  (sahte proxy sunucusu ile)
- `socks5_rejects_auth_required` — auth-gerektiren proxy reddedilir
- `socks5_propagates_connect_failure` — REP != 0x00 hatası iletilir

Ek olarak **E2E entegrasyon testi** ([[bug-fixes-2026-06-28-v2]]):
Python ile yazılmış sahte bir SOCKS5 proxy'si üzerinden gerçek dosya
transferi doğrulandı (md5 birebir aynı).

## İlgili Sayfalar

- [[architecture]] — güncel modül yapısı
- [[chunked-file-transfer]] — v2 transfer protokolü
- [[bug-fixes-2026-06-28-v2]] — SOCKS5 entegrasyon düzeltmeleri
- [[shadowshare]] — proje genel bakış
