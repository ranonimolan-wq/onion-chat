---
title: Chunked File Transfer Protocol v2
created: 2026-06-28
updated: 2026-06-28
type: concept
tags: [chunking, file-transfer-protocol, message-format, progress, resumable, rust-2024]
sources: [src/file_transfer.rs]
confidence: high
---

# Chunked File Transfer Protocol v2

ShadowShare'ın dosya transferi artık `v1`'in tüm-dosya-bellekte modelini
bırakıp `v2` **parçalı (chunked)** protokolünü kullanıyor. Bu sayede büyük
dosyalar sabit bellekle aktarılabilir; ileride resumable transfer ve
ilerleme çubuğu için zemin hazırdır.

## Motivasyon

`v1` protokolünde `send_file` dosyayı tek seferde `tokio::fs::read` ile
belleğe alıp tek bir AES-GCM çerçevesi olarak gönderiyordu. Bu yaklaşım:

- **10 GiB'lik bir dosya = 10 GiB heap** — tipik bir masaüstü makinede OOM.
- **İlerleme takibi imkânsız** — tek çağrı, tek ilerleme olayı.
- **Hata durumunda baştan başlama** — kısmi transferi sürdürmek için
  protokolde hiçbir işaret yok.

`v2` bu üç sorunu da çözer.

## Kablo Formatı

Tüm mesajlar `network::Connection::send_message`/`recv_message` üzerinden
AES-GCM ile şifrelenir; bu modül sadece mesajların **semantiğini** belirler.

```text
[version: u8 = 0x02]            // ileriye dönük uyumluluk için
[file_name_len: u32 BE]
[file_name: UTF-8]
[file_size: u64 BE]             // toplam bayt (ilerleme takibi için)
[chunk_size: u32 BE]            // parça boyu (genelde 64 KiB)
// sonra N adet parça:
  [chunk_data]                  // send_message/recv_message ile çerçevelenir
// sonunda sentinel:
[0x00000000]                    // 4 bayt sıfır = "transfer bitti"
```

Sentinel de AES-GCM ile şifrelenir; bu nedenle ağ izleyici sadece rastgele
gürültü görür.

## Bellek Profili

`v2` her parçayı sırayla okur, gönderir, ilerlemeyi bildirir ve bir sonraki
parçaya geçer. Sabit bellek kullanımı:

- 1 × `chunk_size` bayt okuma tamponu (varsayılan 64 KiB)
- 1 × `chunk_size + 12 (nonce) + 16 (GCM tag)` bayt AES-GCM çıktısı
- 4 bayt uzunluk öneki

Yani ~130 KiB sabit bellekle 10 GiB dosya aktarılabilir.

## Sentinel Doğrulaması

Alıcı her `recv_message` çağrısından dönen byte dizisini `END_SENTINEL`
ile karşılaştırır:

```rust
if chunk.len() == 4 && chunk == END_SENTINEL {
    break;
}
```

`chunk.len() == 4` kontrolü **kasıtlı**. 4 bayttan kısa veya uzun parçalar
asla sentinel ile karıştırılmaz. Edge case: tam 4 baytlık ve hepsi sıfır
olan bir parça, sentinel ile çakışır. Pratikte bu:

- Rastgele dosya içeriğinde 2⁻³² olasılıkla oluşur (yok sayılır).
- Yapısal veride tesadüfen oluşursa (örn. zero-padded dosya sonu),
  kullanıcı `chunk_size`'ı değiştirerek kaçınabilir.

İleride bu edge case'i kaldırmak için sentinel'e özel bir **opcode byte**
ön eki eklenebilir (örn. `[0xFF, 0x00, 0x00, 0x00, 0x00]`).

## `Progress` Trait

```rust
pub trait Progress: Send + Sync + 'static {
    fn on_chunk(&self, transferred: u64, total: u64);
}
```

`Send + Sync + 'static` bound'ları, `&dyn Progress`'in `tokio::spawn` ile
oluşturulan async görevler arasında güvenle paylaşılabilmesi içindir.
İleride UI modülü bu trait'i implement edip TUI ilerleme çubuğunu
besleyecek. Şimdilik `NoProgress` no-op implementasyonu var.

## Sürüm Uyumluluğu

Alıcı ilk frame'deki `version` baytını kontrol eder; `0x02`'den farklıysa
hata döner. Bu sayede ileride `v3` (örn. SHA-256 manifest, resumable offset)
tanıtıldığında eski eşler açıkça reddedilir — sessiz veri bozukluğu olmaz.

Eski `v1` eşlerle uyumluluk **kasıtlı olarak yok**; `v1` protokolü hiçbir
üretim kullanımı olmadığı için geriye dönük uyumluluk gerekmiyor.

## Test Kapsamı

`file_transfer::tests` modülünde:

- 4 adet uçtan-uca chunked round-trip (tiny/small/default/exact-multiple)
- Eski sürüm baytı reddi (`recv_rejects_old_protocol_version`)
- Sıfır parça boyu reddi (`send_file_rejects_zero_chunk_size`)
- Sabit değer doğrulamaları (`protocol_version_is_v2`,
  `end_sentinel_is_four_zeros`, `default_chunk_size_is_64kib`)
- Big-endian u32/u64 round-trip + kısa tampon reddi
- `NoProgress` smoke test
- Orijinal sanitize testleri (v1'den taşındı)

Toplam: 18 test, hepsi paralel ve tekil thread modlarında geçiyor.

## İlgili Sayfalar

- [[architecture]] — güncel modül yapısı
- [[bug-fixes-2026-06-28]] — bu oturumdaki düzeltmeler
- [[shadowshare]] — proje genel bakış
