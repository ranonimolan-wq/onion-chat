// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! OnionChat CLI giriş noktası.
//!
//! Tor destekli P2P uçtan uca şifreli (E2EE) çoklu-kullanıcı sohbet
//! uygulaması. Dosya transferi ek özellik olarak sunulur.
//!
//! ## Kullanım
//!
//! ```text
//! onionchat                                          # TUI (varsayılan)
//! onionchat --listen 8080                            # 0.0.0.0:8080'de dinle
//! onionchat --listen :8080                           # 0.0.0.0:8080'de dinle
//! onionchat --listen 127.0.0.1:8080                  # tam adres
//! onionchat --connect 1.2.3.4:8080                   # bağlan (file opsiyonel)
//! onionchat --connect 1.2.3.4:8080 --file foo.txt    # dosya gönder
//! onionchat --anon --connect 1.2.3.4:8080            # Tor üzerinden bağlan
//! onionchat --socks5 127.0.0.1:9050 --connect ...    # özel SOCKS5
//! ```
//!
//! Hiçbir argüman verilmezse TUI moduna girilir. `--listen` ve `--connect`
//! aynı anda verilemez. Adres kısaltmaları: `8080` ve `:8080` otomatik
//! olarak `0.0.0.0:8080`'e genişletilir.

mod chat;
mod commands;
mod config;
mod crypto;
mod file_transfer;
mod history;
mod markdown;
mod network;
mod roles;
mod socks5;
mod tor_control;
mod ui;

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::{info, warn};

/// Tor SOCKS5 endpoint'inin varsayılan adresi. `--anon` bayrağı
/// verilip `--socks5` verilmediğinde kullanılır.
const DEFAULT_TOR_SOCKS5: &str = "127.0.0.1:9050";

/// CLI argümanları. Subcommand yok — flat args. Bu, kullanıcı için
/// `onionchat --listen 8080` gibi basit bir çağrı sağlar.
#[derive(Parser, Debug)]
#[command(name = "onionchat", version, about = "Tor-supported P2P E2EE multi-user chat with file transfer", long_about = None)]
struct Args {
    /// Dinleme adresi. Kısa formlar kabul edilir:
    /// `8080`, `:8080`, `127.0.0.1:8080`, `0.0.0.0:8080`.
    /// Verilirse program dinleyici modunda çalışır.
    #[arg(long)]
    listen: Option<String>,

    /// Bağlanılacak peer adresi. Aynı kısa formlar geçerli.
    /// Verilirse program istemci modunda çalışır.
    #[arg(long)]
    connect: Option<String>,

    /// Gönderilecek dosya (yalnızca `--connect` ile anlamlı).
    /// Verilmezse bağlantı kurulur ama dosya gönderilmez (chat için).
    #[arg(long)]
    file: Option<PathBuf>,

    /// Tor anonimliğini etkinleştir. `--socks5` verilmemişse
    /// Tor varsayılanı `127.0.0.1:9050` kullanılır.
    #[arg(long)]
    anon: bool,

    /// SOCKS5 proxy adresi (örn. `127.0.0.1:9050`). `--anon` ima eder.
    #[arg(long)]
    socks5: Option<String>,

    /// Hub (multi-peer) modu. `--listen` ile birlikte kullanılır.
    /// Birden fazla peer'ı paralel kabul eder, mesajları broadcast eder
    /// (star topoloji).
    #[arg(long)]
    multi: bool,

    /// Tor hidden service oluştur. `--listen` ile birlikte kullanılır.
    /// Tor control port üzerinden ADD_ONION komutu gönderir ve onion
    /// adres döner. Tor sistem servisi çalışıyor olmalı.
    #[arg(long)]
    hidden_service: bool,

    /// Tor control port adresi (varsayılan: `127.0.0.1:9051`).
    /// `--hidden-service` ile kullanılır.
    #[arg(long)]
    tor_control: Option<String>,

    /// Tor cookie dosyası yolu (varsayılan:
    /// `/var/run/tor/control.authcookie`). `--hidden-service` ile kullanılır.
    #[arg(long)]
    tor_cookie: Option<String>,

    /// Config dosyası yolu (varsayılan: `~/.onionchat/config.json`).
    /// Tüm ayarlar buradan yönetilir: history, roles, anonymity, permissions.
    #[arg(long)]
    config: Option<String>,
}

/// Kullanıcının verdiği adresi `SocketAddr`'a çevirir. Kısa formları
/// kabul eder:
/// - `8080` → `0.0.0.0:8080`
/// - `:8080` → `0.0.0.0:8080`
/// - `127.0.0.1:8080` → olduğu gibi
/// - `0.0.0.0:8080` → olduğu gibi
///
/// Geçersiz formatlarda `Err` döner.
fn normalize_addr(raw: &str) -> Result<SocketAddr, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty address".to_string());
    }
    // Sadece port: "8080"
    if let Ok(port) = trimmed.parse::<u16>() {
        return Ok(SocketAddr::from(([0, 0, 0, 0], port)));
    }
    // ":8080" formu
    if let Some(rest) = trimmed.strip_prefix(':')
        && let Ok(port) = rest.parse::<u16>()
    {
        return Ok(SocketAddr::from(([0, 0, 0, 0], port)));
    }
    // Tam SocketAddr: "127.0.0.1:8080"
    trimmed
        .parse::<SocketAddr>()
        .map_err(|e| format!("invalid address '{}': {}", trimmed, e))
}

/// `--anon` ve `--socks5 <addr>` parametrelerini çözümler.
///
/// Mantık:
/// - `--socks5 <addr>` verilmişse SOCKS5 o adreste etkin.
/// - `--anon` verilmişse ve `--socks5` verilmemişse Tor varsayılanı.
/// - İkisi de verilmemişse `None` (direkt TCP).
fn resolve_socks5(
    anon: bool,
    socks5: Option<String>,
) -> Result<Option<SocketAddr>, String> {
    if let Some(addr_str) = socks5 {
        let addr = normalize_addr(&addr_str)?;
        Ok(Some(addr))
    } else if anon {
        info!(
            "Anonymity mode: using default Tor SOCKS5 at {}",
            DEFAULT_TOR_SOCKS5
        );
        let addr = normalize_addr(DEFAULT_TOR_SOCKS5)?;
        Ok(Some(addr))
    } else {
        Ok(None)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    // Config yükle (veya default oluştur).
    let config_path = args
        .config
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(config::default_config_path);
    let app_config = config::load_or_create(&config_path).await?;
    info!(
        "Config yüklendi: {} (history={}, max_peers={}, roles={})",
        config::expand_tilde(&config_path).display(),
        app_config.history.enabled,
        app_config.server.max_peers,
        app_config.roles.enabled,
    );

    // Mod seçimi: --listen → dinleyici, --connect → istemci,
    // ikisi de yoksa TUI, ikisi de varsa hata.
    match (args.listen.as_deref(), args.connect.as_deref()) {
        (Some(listen_raw), None) => {
            run_listen(
                listen_raw,
                args.anon,
                args.multi,
                args.hidden_service,
                args.tor_control.as_deref(),
                args.tor_cookie.as_deref(),
                app_config,
                config_path,
            )
            .await?;
        }
        (None, Some(connect_raw)) => {
            run_connect(connect_raw, args.file.as_deref(), args.anon, args.socks5.as_deref())
                .await?;
        }
        (None, None) => {
            run_tui(args.anon).await?;
        }
        (Some(_), Some(_)) => {
            return Err("cannot use --listen and --connect together".into());
        }
    }

    Ok(())
}

/// Dinleyici modunu çalıştırır. Mod seçimi:
/// - `--multi` verilmişse → hub modu: birden fazla peer, star topoloji,
///   `ui::run_chat_tui_hub`.
/// - `--hidden-service` verilmişse → önce Tor hidden service oluştur,
///   sonra hub modunda devam et.
/// - İkisi de yoksa → tek-peer modu: ilk bağlantıyı kabul et, peek ile
///   chat mi dosya mı belirle.
#[allow(clippy::too_many_arguments)]
async fn run_listen(
    listen_raw: &str,
    anon: bool,
    multi: bool,
    hidden_service: bool,
    tor_control_addr: Option<&str>,
    tor_cookie_path: Option<&str>,
    app_config: config::Config,
    config_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = normalize_addr(listen_raw).map_err(|e| format!("invalid --listen address: {}", e))?;
    info!("Starting server on {}", addr);

    if anon && !hidden_service {
        warn!("--anon için listener tarafında --hidden-service kullanın ( SOCKS5 istemci-taraflı ).");
    }

    // Hidden service (Tor) — control port üzerinden ADD_ONION.
    if hidden_service {
        let control_addr = tor_control_addr.unwrap_or(tor_control::DEFAULT_CONTROL_ADDR);
        let cookie_path = PathBuf::from(
            tor_cookie_path.unwrap_or(tor_control::DEFAULT_COOKIE_PATH),
        );
        info!(
            "Creating Tor hidden service (control: {}, cookie: {}, local port: {})",
            control_addr,
            cookie_path.display(),
            addr.port()
        );
        match tor_control::create_hidden_service(control_addr, &cookie_path, addr.port()).await {
            Ok(hs) => {
                info!("=== Hidden service hazır ===");
                info!("Onion adres: {}.onion:80", hs.onion_address);
                info!("Peer'lar bu adresle bağlanmalı: --connect {}.onion:80 --socks5 127.0.0.1:9050", hs.onion_address);
                info!("============================");
            }
            Err(e) => {
                warn!("Hidden service oluşturulamadı: {}", e);
                warn!("Tor çalışıyor mu? ControlPort 9051 ve CookieAuthentication 1 ayarlı mı?");
                warn!("Hub moduna hidden service olmadan devam ediliyor...");
            }
        }
    }

    if multi || hidden_service {
        // Hub (multi-peer) modu — birden fazla peer, broadcast.
        // Config ile TUI'ya geç.
        info!("Entering hub (multi-peer) mode. Press Esc to quit.");
        ui::run_chat_tui_hub(addr, app_config, config_path).await?;
        return Ok(());
    }

    // Tek-peer modu — ilk bağlantıyı kabul et, peek ile modu belirle.
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Waiting for incoming connection...");
    let (stream, peer) = listener.accept().await?;
    info!("Accepted connection from {}", peer);
    let mut conn = network::Connection::from_stream(stream).await?;

    // İlk mesajı peek ederek modu belirle (chat mi dosya transferi mi).
    let first_msg = conn.recv_message().await?;
    let is_file_transfer =
        first_msg.len() == 1 && first_msg[0] == file_transfer::PROTOCOL_VERSION;

    if is_file_transfer {
        // Dosya transferi modu — sürüm baytı zaten tüketildi.
        info!("Peer file transfer başlattı, dosya alınıyor...");
        let dest = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let max_size: u64 = 16 * 1024 * 1024 * 1024; // 16 GiB
        let saved = file_transfer::recv_file_after_version(
            &mut conn,
            &dest,
            max_size,
            &file_transfer::NoProgress,
        )
        .await?;
        info!("File received: {}", saved.display());
    } else {
        // Sohbet modu — ilk mesajı chat olarak çözümla.
        match chat::decode_chat(&first_msg)? {
            Some(text) => {
                info!("Entering chat TUI mode. Press Esc to quit.");
                ui::run_chat_tui(conn, ui::UiMode::Listen, peer, Some(text)).await?;
            }
            None => {
                return Err(
                    "protocol mismatch: first message was file marker but len != 1".into(),
                );
            }
        }
    }
    Ok(())
}

/// İstemci modunu çalıştırır. Peer'a TCP (veya SOCKS5 üzerinden)
/// bağlanır ve `--file` verilmişse dosya gönderir; verilmediyse
/// **sohbet TUI moduna** girer.
async fn run_connect(
    connect_raw: &str,
    file: Option<&std::path::Path>,
    anon: bool,
    socks5: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = normalize_addr(connect_raw)
        .map_err(|e| format!("invalid --connect address: {}", e))?;
    let socks5_addr = resolve_socks5(anon, socks5.map(|s| s.to_string()))
        .map_err(|e| format!("invalid SOCKS5 address: {}", e))?;
    info!(
        "Connecting to {} (SOCKS5: {})",
        target,
        socks5_addr
            .map(|a| a.to_string())
            .unwrap_or_else(|| "disabled".to_string())
    );

    // SOCKS5 verilmişse `connect_via_socks5`, yoksa `connect`.
    let mut conn = if let Some(socks5_addr) = socks5_addr {
        network::Connection::connect_via_socks5(target, socks5_addr, None).await?
    } else {
        network::Connection::connect(target, None).await?
    };

    if let Some(path) = file {
        // Dosya transferi modu — dosyayı gönder ve çık.
        info!("Will send file: {}", path.display());
        file_transfer::send_file(
            &mut conn,
            path,
            file_transfer::DEFAULT_CHUNK_SIZE,
            &file_transfer::NoProgress,
        )
        .await?;
        info!("File sent: {}", path.display());
    } else {
        // Sohbet TUI modu — bağlantı kuruldu, etkileşimli chat başlat.
        info!("Entering chat TUI mode. Press Esc to quit.");
        ui::run_chat_tui(conn, ui::UiMode::Connect, target, None).await?;
    }
    Ok(())
}

/// TUI modunu çalıştırır. Argüman verilmediğinde çağrılır.
/// Şu an sadece yardım mesajı yazdırır — etkileşimli TUI için
/// `--listen` veya `--connect` ile başlatılması gerekir.
async fn run_tui(_anon: bool) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("OnionChat v0.1 — Tor P2P E2EE Chat");
    eprintln!();
    eprintln!("Başlamak için bir mod seçin:");
    eprintln!();
    eprintln!("  onionchat --listen 8080              # 8080 portunda dinle");
    eprintln!("  onionchat --connect 1.2.3.4:8080     # Peer'a bağlan (sohbet)");
    eprintln!("  onionchat --connect 1.2.3.4:8080 --file foo.txt  # Dosya gönder");
    eprintln!("  onionchat --anon --connect 1.2.3.4:8080          # Tor üzerinden");
    eprintln!();
    eprintln!("Tam yardım: onionchat --help");
    Ok(())
}
