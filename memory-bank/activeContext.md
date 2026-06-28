# Active Context

- **Current Focus**: OnionChat v0.1.2 — proje adı tamamen normalize edildi
  (klasör `onionchat/`, crate `onionchat`, binary `onionchat`). CLI
  subcommand'lardan flat args'a geçirildi; `onionchat --listen 8080`
  gibi tek düzey komutlar artık çalışıyor. Odak: Tor destekli P2P
  E2EE çoklu-kullanıcı sohbet uygulaması (dosya transferi ek özellik).
- **Recent Changes (2026-06-28 — CLI refactor + final rename)**:
  - **Klasör**: `/home/z/my-project/shadowshare/shadowshare/` →
    `/home/z/my-project/onionchat/onionchat/` (`mv` ile).
  - **Crate adı**: `Cargo.toml`'da `onion-chat` → `onionchat`.
    Binary adı da `onion-chat` → `onionchat` oldu.
  - **CLI yapısı**: Subcommand'lar (`listen`/`connect`/`ui`) tamamen
    kaldırıldı. Artık tek düzey flat args var:
    - `onionchat` → TUI (varsayılan)
    - `onionchat --listen 8080` → dinleyici (port kısa formu)
    - `onionchat --listen :8080` → dinleyici (colon formu)
    - `onionchat --listen 127.0.0.1:8080` → dinleyici (tam adres)
    - `onionchat --connect 1.2.3.4:8080 --file foo.txt` → istemci
    - `onionchat --anon --connect ...` → Tor üzerinden
    - `onionchat --socks5 127.0.0.1:9050 --connect ...` → özel SOCKS5
  - **Adres normalizasyonu**: `normalize_addr(raw)` yardımcısı eklendi.
    `8080` ve `:8080` → `0.0.0.0:8080`; `127.0.0.1:8080` olduğu gibi.
  - **Mod seçimi**: `--listen` ve `--connect` aynı anda verilirse hata.
    Hiçbiri verilmezse TUI moduna düşülür.
  - **Kod organizasyonu**: `main.rs`'taki mod-seçim mantığı üç yardımcı
    fonksiyona ayrıldı: `run_listen`, `run_connect`, `run_tui`. Bu
    sayede `main()` fonksiyonu ince kaldı (Rule 4).
  - **Clippy düzeltmesi**: `if let ... { if let ... { } }` →
    `if let ... && let ... { }` (Rust 2024 let-chains syntax'ı,
    `clippy::collapsible_if` uyarısı).
  - **Wiki**: `architecture.md`'ye "CLI Yapısı (v0.1.2 — kullanıcı dostu)"
    bölümü eklendi. `log.md`'ya rename + refactor kaydı eklendi.
  - Doğrulama: `cargo check` 0/0, `cargo clippy -- -D warnings` 0 uyarı,
    `cargo test` 23/23.
- **Tooling**:
  - Rust 1.96.0 (stable) + clippy.
  - Binary: `target/release/onionchat` (2.5 MB).
  - E2E test altyapısı: `scripts/fake_socks5_proxy.py`.
- **Next Steps**:
  1. **`chat` modülü** (v0.2 hedefi): çoklu-kullanıcı sohbet için
     mesaj broadcast, kullanıcı listesi, oturum yönetimi. `network::listen`
     akışını kullanacak bir `ListenerPool` abstraction'ı gerekebilir.
     `--connect` (dosyasız) şu an placeholder; chat modülü gelince
     gerçek event loop olacak.
  2. **TUI event loop** (v0.2): klavye kısayolları, mesaj geçmişi, dosya
     transferi için `TuiProgress` ile ilerleme çubuğu, SOCKS5 adresi girişi.
     `onionchat` (argümansız) varsayılan TUI moduna bir event loop eklemek.
  3. **Tor hidden service** (v0.3): sunucu tarafı anonimlik için `arti-client`
     crate değerlendirmesi; `onionchat --anon --listen 8080` gerçek bir
     onion adrese bağlanacak.
  4. **Resumable transfer** (v0.4): SHA-256 manifest + offset-based resume,
     v3 protokolüne geçiş.
- **Open Questions**:
  - Çoklu-kullanıcı sohbet için mesh-topoloji mi, yoksa "bir peer sunucu
    diğerleri istemci" modeli mi tercih edilecek? Mesh daha karmaşık ama
    merkeziyetsiz.
  - Tor hidden service için `arti-client` crate'ini mi kullanmalı, yoksa
    kendi SOCKS5 + system Tor yönetimini mi tercih etmeli?
  - SOCKS5 username/password auth desteği gerekecek mi? (Tor no-auth kullanır,
    ama özel proxy'ler auth isteyebilir.)
  - Sentinel ile 4-byte-sıfır chunk çakışması: opcode byte ön eki eklensin mi?
- **E2E Test Sonuçları (2026-06-28, CLI refactor sonrası)**:
  - Test 1: `onionchat --listen 8080` (port kısa formu) — `0.0.0.0:8080` ✓
  - Test 2: `onionchat --connect 127.0.0.1:8080 --file ...` — md5 birebir ✓
  - Test 3: `onionchat --listen :8090` (colon formu) — `0.0.0.0:8090` ✓
  - Test 4: `onionchat --listen + --connect` — hata mesajı ✓
  - Test 5: `onionchat --listen 127.0.0.1:8091` (tam adres) ✓
- **İsim Geçmişi**:
  - 2026-06-26 → 2026-06-28 (önce): "ShadowShare" — dosya transferi odaklı.
  - 2026-06-28 (öğle): "OnionChat" — sohbet odaklı, crate `onion-chat`, klasör `shadowshare`.
  - 2026-06-28 (şimdi): "OnionChat" — crate `onionchat`, klasör `onionchat/onionchat/`,
    CLI flat args'a geçti.
