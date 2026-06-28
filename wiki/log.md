# Wiki Log

> Chronological record of all wiki actions. Append-only.
> Format: `## [YYYY-MM-DD] action | subject`
> Actions: ingest, update, query, lint, create, archive, delete
> When this file exceeds 500 entries, rotate: rename to log-YYYY.md, start fresh.

## [2026-06-26] create | Wiki initialized
- Domain: ShadowShare: Peer-to-peer end-to-end encrypted file transfer system with anonymity focus
- Structure created with SCHEMA.md, index.md, log.md

## [2026-06-26] update | Initial implementation sweep
- crypto: AES-GCM `new_from_slice` ile tip çıkarımı düzeltildi
- network: ECDHE `as_bytes`/`From<[u8;32]>` ref problemleri düzeltildi
- network: `Connection::split` artık `OwnedReadHalf`/`OwnedWriteHalf` + mpsc kullanıyor
- network: `listen` akışı otomatik ECDHE yapıp `(Sender, Receiver)` çifti üretiyor
- file_transfer: `send_file`/`recv_file` + `sanitize_file_name` (path traversal koruması)
- ui: ratatui 0.20 API (`Frame::size`, `Paragraph::new(String)`)
- main: `CliUiMode` + `From<CliUiMode> for ui::UiMode`
- tests: 4/4 geçti (mesaj round-trip, split round-trip, sanitize × 2)
- cargo check: 0 hata, 0 uyarı
- E2E smoke: dinleyici ilk mesajı (15 bayt dosya adı) başarıyla aldı

## [2026-06-26] create | architecture.md
- Yeni sayfa: `wiki/concepts/architecture.md`
- Modül haritası + veri akışı + splitleme modeli

## [2026-06-26] create | bug_fixes.md
- Yeni sayfa: `wiki/history/bug_fixes.md`
- 11 derleme hatasının kök neden ve çözüm kaydı

## [2026-06-26] update | index.md
- Yeni sayfalar eklendi; toplam 3 sayfa

## [2026-06-28] create | chunked-file-transfer.md
- Yeni sayfa: `wiki/concepts/chunked-file-transfer.md`
- v2 chunked transfer protokolü: kablo formatı, bellek profili,
  sentinel doğrulaması, Progress trait, sürüm uyumluluğu

## [2026-06-28] create | bug_fixes_2026-06-28.md
- Yeni sayfa: `wiki/history/bug_fixes_2026-06-28.md`
- 8 değişiklik kaydı: v2 protokol geçişi, imza senkronizasyonu,
  paralel test tmp çakışması, dead_code uyarıları

## [2026-06-28] update | architecture.md
- v2 değişiklikleri eklendi (modül başına)
- Yeni veri akışı diyagramı (chunked)
- Bellek profili bölümü
- Yeni cross-link'ler: [[chunked-file-transfer]], [[bug-fixes-2026-06-28]]

## [2026-06-28] update | index.md
- Yeni sayfalar eklendi; toplam 5 sayfa

## [2026-06-28] create | socks5.md
- Yeni sayfa: `wiki/concepts/socks5.md`
- RFC 1928 SOCKS5 istemci tarafı detayları, güvenlik modeli,
  CLI kullanımı, sunucu tarafı notu, test kapsamı

## [2026-06-28] create | bug_fixes_2026-06-28-v2.md
- Yeni sayfa: `wiki/history/bug_fixes_2026-06-28-v2.md`
- 7 değişiklik kaydı: yeni socks5 modülü, connect_via_socks5,
  --socks5 CLI bayrağı, clippy doc_lazy_continuation, test module
  path fix, Çince karakter temizliği
- 3 senaryolu E2E test kaydı: direkt TCP text, direkt TCP binary,
  SOCKS5 üzerinden transfer (md5 birebir aynı)

## [2026-06-28] update | architecture.md
- `socks5` modülü eklendi (modül listesi)
- `network.rs` ve `main.rs` için v2.1 değişiklik notları
- Yeni "Anonimlik Katmanı" bölümü + diyagram
- Yeni cross-link'ler: [[socks5]], [[bug-fixes-2026-06-28-v2]]

## [2026-06-28] update | index.md
- Yeni sayfalar eklendi; toplam 7 sayfa

## [2026-06-28] rename | Proje ShadowShare → OnionChat
- **Domain değişikliği**: Projenin odak noktası dosya transferinden
  çoklu-kullanıcı E2EE sohbete kaydırıldı. Tor destekli P2P chat app'i
  olarak yeniden konumlandırıldı; dosya transferi ek özellik olarak korundu.
- **Crate adı**: `shadowshare` → `onion-chat` (`Cargo.toml`).
- **CLI adı**: `ShadowShare` → `onion-chat` (`main.rs` `#[command(name = ...)]`).
- **CLI about**: "A peer-to-peer end-to-end encrypted file transfer system
  with anonymity focus" → "Tor-supported P2P E2EE multi-user chat with
  file transfer".
- **Copyright başlıkları**: Tüm `src/*.rs` dosyalarında "ShadowShare
  Developers" → "OnionChat Developers".
- **Doc-comment'ler**: Tüm `///` açıklamalarında `ShadowShare` → `OnionChat`.
- **TUI başlıkları**: `ui.rs`'ta "ShadowShare — Listening/Connecting" →
  "OnionChat — Listening/Connecting"; footer metni "OnionChat v0.1 —
  Tor P2P E2EE chat + file transfer".
- **Wiki**: `entities/shadowshare.md` silindi, `entities/onion-chat.md`
  oluşturuldu (yeni kimlik, yol haritası, isim geçmişi bölümü).
  `SCHEMA.md` domain tanımı güncellendi. `index.md` linkler güncellendi.
- **Eski wiki sayfaları**: `bug-fixes-2026-06-26.md`, `bug_fixes_2026-06-28.md`,
  `bug_fixes_2026-06-28-v2.md`, `chunked-file-transfer.md`, `socks5.md`,
  `architecture.md` tarihsel kayıt olarak korundu — içindeki "ShadowShare"
  referansları geçmiş tarihleri yansıtır.
- **Doğrulama**: `cargo check` 0/0, `cargo clippy -- -D warnings` 0,
  `cargo test` 23/23 (isim değişikliği testleri etkilemedi).

## [2026-06-28] update | index.md (post-rename)
- Entity `[[shadowshare]]` → `[[onion-chat]]` link güncellendi
- Toplam sayfa sayısı: 7 (aynı)

## [2026-06-28] rename | Klasör + Crate adı onionchat'e normalize
- **Klasör**: `/home/z/my-project/shadowshare/shadowshare/` →
  `/home/z/my-project/onionchat/onionchat/` (`mv` ile).
- **Crate adı**: `onion-chat` → `onionchat` (`Cargo.toml`).
  Binary adı: `onion-chat` → `onionchat`.
- **CLI yapısı**: Subcommand'lar (`listen`/`connect`/`ui`) kaldırıldı;
  flat args'a geçildi. `clap::Parser` tek düzey argümanlar kullanıyor.
- **Adres kısa formları**: `normalize_addr` yardımcısı eklendi.
  `8080` → `0.0.0.0:8080`, `:8080` → `0.0.0.0:8080`, `127.0.0.1:8080` →
  olduğu gibi.
- **Mod seçimi**: `--listen` → dinleyici, `--connect` → istemci,
  hiçbiri → TUI (varsayılan), ikisi birden → hata.
- **Kod organizasyonu**: `main.rs`'taki mod-seçim mantığı üç yardımcı
  fonksiyona ayrıldı: `run_listen`, `run_connect`, `run_tui`.
- **Clippy düzeltmesi**: `if let ... { if let ... { } }` →
  `if let ... && let ... { }` (Rust 2024 let-chains syntax'ı).
- **Doğrulama**: `cargo check` 0/0, `cargo clippy -- -D warnings` 0,
  `cargo test` 23/23.
- **E2E**: 5 senaryo test edildi:
  - `--listen 8080` (port kısa formu) → `0.0.0.0:8080` ✓
  - `--listen :8090` (colon formu) → `0.0.0.0:8090` ✓
  - `--listen 127.0.0.1:8091` (tam adres) → olduğu gibi ✓
  - `--connect 127.0.0.1:8091 --file ...` → dosya transferi md5 birebir ✓
  - `--listen + --connect` aynı anda → "cannot use --listen and --connect together" ✓

## [2026-06-28] update | architecture.md
- `main` modülüne v0.1.2 değişiklik notu eklendi (subcommand'lar kaldırıldı)
- Yeni "CLI Yapısı (v0.1.2 — kullanıcı dostu)" bölümü: örnek komutlar,
  mod seçimi kuralları, adres normalizasyonu tablosu

## [2026-06-28] create | chat-tui.md
- Yeni sayfa: `wiki/concepts/chat-tui.md`
- Sohbet modülü (`chat.rs`) ve ratatui TUI (`ui.rs`) detayları
- Mimari diyagram, protokol formatı, mod seçimi, layout, klavye
  kısayolları, bağlantı kopması, test kapsamı

## [2026-06-28] create | bug_fixes_2026-06-28-v3.md
- Yeni sayfa: `wiki/history/bug_fixes_2026-06-28-v3.md`
- 7 değişiklik kaydı: yeni chat modülü, recv_file_after_version
  refactor, ui.rs tamamen yeniden yazım, main.rs chat TUI'a bağlanma,
  clippy collapsible_if, lifetime syntax, dead_code uyarıları
- E2E test: 3 senaryo (merhaba, selam, uzun mesaj) — çift yönlü sohbet
  doğrulandı

## [2026-06-28] update | architecture.md (v0.1.3)
- Yeni `chat` modülü eklendi (modül listesi)
- `ui` modülü açıklaması güncellendi (artık tam chat TUI)
- `file_transfer` modülüne v0.1.3 notu (`recv_file_after_version`)
- `main` modülüne v0.1.3 notu (chat TUI entegrasyonu, peek mantığı)

## [2026-06-28] update | index.md (v0.1.3)
- Yeni sayfa eklendi: `[[chat-tui]]`, `[[bug-fixes-2026-06-28-v3]]`
- Toplam sayfa sayısı: 8

## [2026-06-28] create | v0.2-features.md ( büyük sürüm )
- Yeni sayfa: `wiki/concepts/v0.2-features.md`
- v0.2 özellik seti özeti: 5 yeni modül, 9 senaryolu E2E test
- Modül sınırı uyumu (Rule 7) ve Zero Unsafe Policy (Rule 1) notları

## [2026-06-28] create | 5 yeni modül
- `src/history.rs` — JSON Lines persistansı (14 test)
- `src/markdown.rs` — inline format + emoji (21 test)
- `src/commands.rs` — slash komut parser (16 test)
- `src/tor_control.rs` — Tor hidden service (4 test)
- `src/ui.rs`'a `run_chat_tui_hub` eklendi — multi-peer star topoloji

## [2026-06-28] update | main.rs (yeni flag'ler)
- `--multi` — hub modu (multi-peer star topoloji)
- `--hidden-service` — Tor control port üzerinden ADD_ONION
- `--tor-control <addr>` — Tor control port adresi
- `--tor-cookie <path>` — Tor cookie dosyası yolu

## [2026-06-28] update | index.md (v0.2)
- Yeni sayfa: `[[v0.2-features]]`
- Toplam sayfa sayısı: 9

## [2026-06-28] create | turkish-support.md + 59 unit test + 11 E2E test
- Yeni sayfa: `wiki/concepts/turkish-support.md`
- Türkçe karakter (ş ğ ü ö ç ı İ Ş Ğ Ü Ö Ç) tam UTF-8 desteği dokümantasyonu
- Katman katman destek: chat.rs, ui.rs, markdown.rs, commands.rs, history.rs
- 59 yeni unit test (chat: 13, ui: 24, markdown: 11, commands: 8, history: 8)
- 11 senaryolu canlı E2E test (3 terminal: hub + 2 spoke)

## [2026-06-28] fix | markdown.rs UTF-8 bozulması (kritik bug)
- **Sorun**: `render_spans` `bytes[i] as char` kullanıyordu — tek baytı
  char'a çeviriyordu, çok baytlı UTF-8 karakterleri bozuyordu.
  `ş` (C5 9F) → `Å` (C5) + `Ÿ` (9F) olarak render ediliyordu.
- **Çözüm**: `text.char_indices().peekable()` ile karakter sınırlarında
  ilerle. Her iterasyon bir tam Unicode karakter döner.
- **Etki**: Tüm Türkçe karakterler markdown rendering'de artık doğru.

## [2026-06-28] update | index.md (Turkish support)
- Yeni sayfa: `[[turkish-support]]`
- Toplam sayfa sayısı: 10

## [2026-06-28] create | config-roles.md + 2 yeni modül + 94 test (v0.3)
- Yeni sayfa: `wiki/concepts/config-roles.md`
- v0.3 özellik seti: config.json, rol sistemi, /clear N, /on_admin, /config set
- 2 yeni modül: `config.rs` (35 test), `roles.rs` (35 test)
- 24 yeni komut testi (`commands.rs`)
- 9 senaryolu canlı E2E test (config + rol + anonymity)

## [2026-06-28] create | config.rs (merkezi yapılandırma)
- `Config` struct: history, server, roles, anonymity, permissions
- Default: history=false (anonymity), strip_metadata=true
- `load_or_create` — ilk başlatmada default config oluşturur
- `expand_tilde` — `~` genişletme (HOME ile)
- `set_field` — runtime config değişikliği (`/config set`)
- 35 unit test

## [2026-06-28] create | roles.rs (rol + peer kimliği)
- `Role` enum: Admin, Moderator, User, Guest
- İzin matrisi: can_clear/kick/mute/grant/revoke/view
- `PeerInfo`: id, nick, role, muted, addr
- `display_name`: nick > peer-<id> > addr (anonymity)
- `PeerRegistry`: add/remove/find_by_nick/count_by_role
- 35 unit test

## [2026-06-28] update | commands.rs (yeni komutlar)
- `/clear [N]` — ClearCount varyantı
- `/on_admin`, `/off_admin` (+ alias op, deop)
- `/kick`, `/mute`, `/unmute`
- `/role`, `/role <nick>`
- `/config`, `/config set <key> <value>`
- Help text güncellendi (anonymity notu)
- 24 yeni test

## [2026-06-28] update | ui.rs hub (PeerRegistry + Config)
- `run_chat_tui_hub` imza değişti: Config + config_path alır
- PeerRegistry ile peer ID + nick + role tracking
- Anonimlik: peer-<id> ile gösterim (IP gizli)
- Config-driven history (enabled ise yükler/kaydeder)
- Rol-bazlı komut işleme (hub operator = admin)
- `/clear [N]` implementasyonu (truncate)
- `/on_admin` peer'a bildirir
- `/kick` sender'ı kaldırır (bağlantı kapanır)
- `/mute` peer mesajlarını broadcast etmez
- `/config set` dosyaya kaydeder

## [2026-06-28] update | main.rs (--config flag)
- `--config <path>` CLI bayrağı
- `config::load_or_create` ile config yükleme
- Config bilgisi log'lanır (history, max_peers, roles)

## [2026-06-28] update | index.md (v0.3)
- Yeni sayfa: `[[v0.3-config-roles]]`
- Toplam sayfa sayısı: 11

## [2026-06-28] fix | Backspace (DEL) + /clear broadcast (bug fix session)
- **Bug 1: Backspace çalışmıyordu.** Çoğu terminal Backspace için
  `\x7f` (DEL) veya `\x08` (BS) gönderir, ama `KeyCode::Backspace`
  bekliyorduk. Çözüm: `KeyCode::Char(c) if c == '\x7f' || c == '\x08'`
  ile her iki karakteri de backspace olarak handle et (hub + tek-peer).
- **Bug 2: /clear herkeste silinmiyordu.** Hub `/clear` yapınca
  sadece kendi ekranını temizliyordu, peer'lara broadcast etmiyordu.
  Çözüm: `chat.rs`'a `CLEAR_MSG_TYPE = 0x03` + `send_clear_split` +
  `is_clear_message` eklendi. Hub `/clear` yapınca tüm peer'lara
  clear komutu broadcast edilir; peer alınca kendi ekranını temizler.
  Aynı `/clear <N>` için de. Spoke `/clear` yapınca hub'a gönderir,
  hub diğer spoke'lara broadcast eder (chained).
- **18 yeni test**: backspace cursor ortasında, Türkçe karakter silme,
  multi-backspace, clear round-trip, clear + chat sıralı, is_clear_message
  edge cases.
- **6 senaryolu canlı E2E test**: backspace + clear broadcast (hub→spoke,
  spoke→hub chained).
- Doğrulama: `cargo check` 0/0, `cargo clippy -- -D warnings` 0,
  `cargo test` 281/281 (263 + 18 yeni).
