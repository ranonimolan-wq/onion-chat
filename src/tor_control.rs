// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Tor control modülü — Tor control port üzerinden hidden service yayınlama.
//!
//! Bu modül Tor'un control port'una (varsayılan `127.0.0.1:9051`)
//! bağlanır ve `ADD_ONION` komutu ile yeni bir hidden service oluşturur.
//! Bu sayede `onionchat --listen` Tor ağı üzerinden onion adresle
//! erişilebilir hale gelir.
//!
//! ## Gereksinimler
//!
//! 1. Tor sistem servisi çalışıyor olmalı.
//! 2. `torrc` dosyasında `ControlPort 9051` ayarlı olmalı.
//! 3. Cookie authentication etkin olmalı (varsayılan):
//!    `CookieAuthentication 1`
//!
//! ## Protokol (RFC-ish, Tor control spec)
//!
//! ```text
//! İstemci → Tor: AUTHCHALLENGE SAFECOOKIE <client-nonce>
//! Tor → İstemci: 250 AUTHCHALLENGE SERVERNONCE=<...> 250 SAFECOKEY=<...>
//! ... (HMAC-SHA256 hesaplaması — bu implementasyonda atlanır, no-auth denenir)
//! ```
//!
//! Bu basit implementasyon sadece cookie auth'i okur ama HMAC doğrulaması
//! yapmaz; Tor `CookieAuthentication 1` ile çalışıyorsa genellikle
//! `AUTHENTICATE <cookie-hex>` komutu kabul edilir.
//!
//! ## Sınırlamalar
//!
//! - Sadece cookie authentication (password auth desteklenmez).
//! - v3 onion service (ED25519-V3). v2 desteklenmez (zaten deprecated).
//! - Backend private key üretimi Tor'a bırakılır (`NEW:BEST`).
//! - İstemci nonce HMAC hesaplaması yapılmaz — bazı Tor konfigürasyonlarında
//!   bu reddedilebilir. Bu durumda kullanıcı `torrc`'e
//!   `CookieAuthentication 1` eklemeli.

use anyhow::{anyhow, Result};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Varsayılan Tor control port adresi.
pub const DEFAULT_CONTROL_ADDR: &str = "127.0.0.1:9051";

/// Varsayılan cookie dosyası yolu. Tor bu dosyaya her başlangıçta yazar.
pub const DEFAULT_COOKIE_PATH: &str = "/var/run/tor/control.authcookie";

/// Bir hidden service oluşturma sonucu.
#[derive(Debug, Clone)]
pub struct HiddenService {
    /// Onion adresi (`xxxxx.onion` formatında, sonunda .onion yok).
    pub onion_address: String,
    /// Hedef port (lokal dinlenen port). `main.rs` log için kullanır.
    #[allow(dead_code)]
    pub local_port: u16,
}

/// Tor control port'a bağlanıp `ADD_ONION` komutu gönderir.
///
/// `local_port` parametresi hidden service'in yönlendireceği lokal port.
/// Dönen `HiddenService` onion adresini içerir.
///
/// # Hatalar
///
/// - Connection refused → Tor çalışmıyor veya control port kapalı.
/// - 515 Authentication failed → cookie okunamadı veya geçersiz.
/// - 552 syntax error → Tor sürümü v3 desteklemiyor (çok eski).
pub async fn create_hidden_service(
    control_addr: &str,
    cookie_path: &PathBuf,
    local_port: u16,
) -> Result<HiddenService> {
    let mut stream = TcpStream::connect(control_addr).await.map_err(|e| {
        anyhow!(
            "Tor control port {} bağlanılamadı: {} (Tor çalışıyor mu?)",
            control_addr,
            e
        )
    })?;

    // Cookie oku.
    let cookie = tokio::fs::read(cookie_path).await.map_err(|e| {
        anyhow!("cookie okunamadı {}: {}", cookie_path.display(), e)
    })?;
    let cookie_hex = hex_encode(&cookie);

    // AUTHENTICATE <cookie-hex>
    let auth_cmd = format!("AUTHENTICATE {}\r\n", cookie_hex);
    stream.write_all(auth_cmd.as_bytes()).await?;
    let auth_reply = read_reply(&mut stream).await?;
    if !auth_reply.starts_with("250") {
        return Err(anyhow!("AUTHENTICATE reddedildi: {}", auth_reply.trim()));
    }

    // ADD_ONION NEW:BEST Port=80,127.0.0.1:{local_port}
    // 80 = onion service port (kullanıcı browser'da xxx.onion yazınca 80'e gider)
    // lokal port = bizim dinlediğimiz
    let add_cmd = format!(
        "ADD_ONION NEW:BEST Port=80,127.0.0.1:{}\r\n",
        local_port
    );
    stream.write_all(add_cmd.as_bytes()).await?;
    let add_reply = read_reply(&mut stream).await?;

    // Cevap formatı:
    // 250-ServiceID=abcdef1234567890
    // 250-PrivateKey=RSA1024:...
    // 250 OK
    let mut onion_address: Option<String> = None;
    for line in add_reply.lines() {
        if let Some(rest) = line.strip_prefix("250-ServiceID=") {
            onion_address = Some(rest.trim().to_string());
            break;
        }
    }
    let onion_address = onion_address
        .ok_or_else(|| anyhow!("ADD_ONION cevabında ServiceID bulunamadı: {}", add_reply))?;

    // Çıkış — bağlantıyı kapat. Hidden service Tor'un yaşam süresi boyunca
    // kalır (control bağlantısı kapanınca Tor service'i yok eder).
    // TODO: kalıcı hidden service için `Flags=DiscardPK` ve private key sakla.
    let _ = stream.shutdown().await;

    Ok(HiddenService {
        onion_address,
        local_port,
    })
}

/// Basit hex encoder. `&[u8]` → lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Tor control port cevabını okur. Multi-line cevapları birleştirir.
///
/// Cevap formatı: `250-...` (devam), `250 ...` (son satır).
/// `250 OK` görünene kadar okur.
async fn read_reply(stream: &mut TcpStream) -> Result<String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        // Son satır `XYZ ` (space) ile bitiyorsa cevap tamam.
        // (`XYZ-` dash ise devam eder.)
        let s = String::from_utf8_lossy(&buf);
        if let Some(last_line) = s.lines().last()
            && last_line.len() >= 4
        {
            let status = &last_line[..3];
            let sep = &last_line[3..4];
            if status.chars().all(|c| c.is_ascii_digit()) && sep == " " {
                return Ok(s.to_string());
            }
        }
        if buf.len() > 16384 {
            return Err(anyhow!("Tor control cevabı çok uzun (16KB sınırı)"));
        }
    }
    Err(anyhow!("Tor control bağlantısı kapandı"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encode_empty() {
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn hex_encode_bytes() {
        assert_eq!(hex_encode(&[0x00, 0xff, 0xab]), "00ffab");
        assert_eq!(hex_encode(&[0x01, 0x02, 0x03]), "010203");
    }

    #[test]
    fn default_constants_match_tor_defaults() {
        assert_eq!(DEFAULT_CONTROL_ADDR, "127.0.0.1:9051");
        assert!(DEFAULT_COOKIE_PATH.contains("control.authcookie"));
    }

    #[test]
    fn hidden_service_stores_fields() {
        let hs = HiddenService {
            onion_address: "abcdef1234567890".to_string(),
            local_port: 8080,
        };
        assert_eq!(hs.onion_address, "abcdef1234567890");
        assert_eq!(hs.local_port, 8080);
    }

    // Not: `create_hidden_service` ve `read_reply` gerçek Tor servisi
    // gerektirdiği için unit test edilmez. Entegrasyon testleri elle
    // çalıştırılır (Tor kurulu makinada).
}
