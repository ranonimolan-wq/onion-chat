// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! SOCKS5 modülü — RFC 1928 istemci tarafı el sıkışması.
//!
//! Bu modül yalnızca SOCKS5 protokolünün **istemci tarafı** implementasyonudur.
//! Sorumluluğu: bir `TcpStream`'i SOCKS5 proxy'si üzerinden hedef adrese
//! tünellemek. Şifreleme, dosya transferi veya üst seviye bağlantı yönetimi
//! bu modülün dışındadır.
//!
//! ## Desteklenen özellikler
//!
//! - **Metot**: yalnızca "no authentication required" (`0x00`).
//!   Username/password (`0x02`) ve GSSAPI (`0x01`) desteklenmez —
//!   anonim ağlar (Tor/I2P) için yeterlidir.
//! - **Komut**: yalnızca `CONNECT` (`0x01`). `BIND` ve `UDP ASSOCIATE`
//!   yok — OnionChat sadece TCP istemci modunda SOCKS5 kullanır.
//! - **Adres tipleri**: IPv4 (`0x01`) ve IPv6 (`0x04`). Domain (`0x03`)
//!   desteklenir ama `SocketAddr::Ip` zaten IP çözümlemesi yaptığı için
//!   kullanılmaz.
//!
//! ## Güvenlik notları
//!
//! SOCKS5 trafiği **şifresizdir** — yani istemci ↔ proxy arasındaki bağlantı
//! açık metindir. Bu yüzden OnionChat SOCKS5'i yalnızca `--anon` bayrağı
//! ile etkinleştirilir; asıl veri aktarımı her zaman `network::Connection`
//! üzerinden ECDHE + AES-GCM ile uçtan uca şifrelidir. SOCKS5 sadece
//! "peer'ın IP'sini gizle" görevini üstlenir.
//!
//! Tor'un `127.0.0.1:9050` SOCKS5 endpoint'i tipik olarak localhost
//! üzerinde olduğu için bu şifresizlik pratik bir sorun yaratmaz; uzak
//! bir SOCKS5 proxy kullanılıyorsa kullanıcı kendi sorumluluğundadır.

use anyhow::{anyhow, Result};
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// SOCKS5 protokol sürümü (RFC 1928).
const SOCKS_VERSION: u8 = 0x05;

/// "No authentication required" metot kodu.
const METHOD_NO_AUTH: u8 = 0x00;

/// `CONNECT` komutu — TCP bağlantı açma.
const CMD_CONNECT: u8 = 0x01;

/// Rezerve alan — her zaman `0x00`.
const RSV: u8 = 0x00;

/// Adres tipi: IPv4.
const ATYP_IPV4: u8 = 0x01;

/// Adres tipi: IPv6.
const ATYP_IPV6: u8 = 0x04;

/// SOCKS5 yanıt kodlarından başarılı olanı.
const REP_SUCCESS: u8 = 0x00;

/// Verilen `TcpStream`'i SOCKS5 proxy'si üzerinden `target` adresine
/// tünellemek için gerekli el sıkışmasını yapar.
///
/// Başarıyla dönerse, `stream` artık doğrudan `target`'a bağlanmış gibi
/// kullanılabilir — sonraki `read`/`write` çağrıları proxy üzerinden
/// hedefe iletilir.
///
/// # Örnek
///
/// ```ignore
/// let mut stream = TcpStream::connect(socks5_addr).await?;
/// socks5::connect(&mut stream, target_addr).await?;
/// // stream artık target_addr'a bağlı
/// ```
pub async fn connect(stream: &mut TcpStream, target: SocketAddr) -> Result<()> {
    negotiate_method(stream).await?;
    send_connect_request(stream, target).await?;
    receive_connect_reply(stream).await?;
    Ok(())
}

/// Metot anlaşması: istemci "no auth" teklif eder, proxy onaylamalıdır.
///
/// Wire format (RFC 1928 §3):
/// ```text
/// İstemci → Proxy: [VER=0x05, NMETHODS=1, METHODS=[0x00]]
/// Proxy → İstemci: [VER=0x05, METHOD=0x00]
/// ```
async fn negotiate_method(stream: &mut TcpStream) -> Result<()> {
    // İstemci tarafı: sadece "no auth" teklif et.
    stream
        .write_all(&[SOCKS_VERSION, 0x01, METHOD_NO_AUTH])
        .await?;
    // Proxy tarafı: 2 bayt yanıt bekle.
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;
    if buf[0] != SOCKS_VERSION {
        return Err(anyhow!("invalid SOCKS version in reply: {}", buf[0]));
    }
    if buf[1] != METHOD_NO_AUTH {
        return Err(anyhow!(
            "proxy rejected no-auth method (selected: {}); \
             auth not supported",
            buf[1]
        ));
    }
    Ok(())
}

/// `CONNECT` komutu gönderir.
///
/// Wire format (RFC 1928 §4):
/// ```text
/// İstemci → Proxy:
///   [VER=0x05, CMD=0x01, RSV=0x00, ATYP, DST.ADDR, DST.PORT]
/// ```
async fn send_connect_request(stream: &mut TcpStream, target: SocketAddr) -> Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(22);
    buf.extend_from_slice(&[SOCKS_VERSION, CMD_CONNECT, RSV]);
    match target.ip() {
        IpAddr::V4(v4) => {
            buf.push(ATYP_IPV4);
            buf.extend_from_slice(&v4.octets());
        }
        IpAddr::V6(v6) => {
            buf.push(ATYP_IPV6);
            buf.extend_from_slice(&v6.octets());
        }
    }
    buf.extend_from_slice(&target.port().to_be_bytes());
    stream.write_all(&buf).await?;
    Ok(())
}

/// Proxy'nin `CONNECT` yanıtını okur ve doğrular.
///
/// Wire format:
/// ```text
/// Proxy → İstemci:
///   [VER=0x05, REP, RSV=0x00, ATYP, BND.ADDR, BND.PORT]
/// ```
///
/// `REP == 0x00` ise başarı. Diğer değerler için RFC 1928 §5'teki
/// hata kodlarını okuyucu dostu metne çeviririz.
async fn receive_connect_reply(stream: &mut TcpStream) -> Result<()> {
    // İlk 4 bayt: VER, REP, RSV, ATYP.
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await?;
    if head[0] != SOCKS_VERSION {
        return Err(anyhow!("invalid SOCKS version in connect reply: {}", head[0]));
    }
    if head[1] != REP_SUCCESS {
        return Err(anyhow!("SOCKS5 connect failed: {}", reply_message(head[1])));
    }
    // BND.ADDR — atla. ATYP'a göre değişken uzunlukta.
    let atyp = head[3];
    let addr_len = match atyp {
        ATYP_IPV4 => 4,
        ATYP_IPV6 => 16,
        _ => return Err(anyhow!("unsupported BND.ADDR type: {}", atyp)),
    };
    let mut addr_buf = vec![0u8; addr_len];
    stream.read_exact(&mut addr_buf).await?;
    // BND.PORT — 2 bayt.
    let mut port_buf = [0u8; 2];
    stream.read_exact(&mut port_buf).await?;
    Ok(())
}

/// SOCKS5 `REP` alanını insan-okur hata mesajına çevirir.
fn reply_message(rep: u8) -> &'static str {
    match rep {
        0x00 => "succeeded",
        0x01 => "general SOCKS server failure",
        0x02 => "connection not allowed by ruleset",
        0x03 => "network unreachable",
        0x04 => "host unreachable",
        0x05 => "connection refused",
        0x06 => "TTL expired",
        0x07 => "command not supported",
        0x08 => "address type not supported",
        _ => "unknown error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `reply_message` tüm bilinen kodları döndürmeli.
    #[test]
    fn reply_message_covers_known_codes() {
        assert_eq!(reply_message(0x00), "succeeded");
        assert_eq!(reply_message(0x01), "general SOCKS server failure");
        assert_eq!(reply_message(0x05), "connection refused");
        assert_eq!(reply_message(0x08), "address type not supported");
        assert_eq!(reply_message(0xFF), "unknown error");
    }

    /// Sabit değerler RFC 1928 ile uyumlu olmalı.
    #[test]
    fn constants_match_rfc1928() {
        assert_eq!(SOCKS_VERSION, 0x05);
        assert_eq!(METHOD_NO_AUTH, 0x00);
        assert_eq!(CMD_CONNECT, 0x01);
        assert_eq!(ATYP_IPV4, 0x01);
        assert_eq!(ATYP_IPV6, 0x04);
        assert_eq!(REP_SUCCESS, 0x00);
    }

    /// Uçtan uca SOCKS5 el sıkışması: sahte bir SOCKS5 proxy sunucusu
    /// başlatır, istemci tarafı `connect()` çağrılır, başarılı bağlantı
    /// doğrulanır.
    #[tokio::test]
    async fn socks5_handshake_round_trip() -> Result<()> {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let proxy_addr = listener.local_addr()?;
        let target: SocketAddr = "127.0.0.1:8080".parse()?;

        // Sahte SOCKS5 proxy'si: RFC 1928 mesajlarını sırayla okur/yazar.
        let proxy = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await?;

            // 1. Metot anlaşması: istemciden 3 bayt bekle.
            let mut method_req = [0u8; 3];
            sock.read_exact(&mut method_req).await?;
            assert_eq!(method_req[0], SOCKS_VERSION);
            assert_eq!(method_req[1], 0x01); // NMETHODS = 1
            assert_eq!(method_req[2], METHOD_NO_AUTH);
            // Yanıt: "no auth" seçildi.
            sock.write_all(&[SOCKS_VERSION, METHOD_NO_AUTH]).await?;

            // 2. CONNECT isteği: 10 bayt (IPv4) veya 22 bayt (IPv6).
            let mut head = [0u8; 4];
            sock.read_exact(&mut head).await?;
            assert_eq!(head[0], SOCKS_VERSION);
            assert_eq!(head[1], CMD_CONNECT);
            assert_eq!(head[2], RSV);
            let addr_len = match head[3] {
                ATYP_IPV4 => 4,
                ATYP_IPV6 => 16,
                _ => return Err(anyhow!("unexpected ATYP")),
            };
            let mut addr = vec![0u8; addr_len];
            sock.read_exact(&mut addr).await?;
            let mut port = [0u8; 2];
            sock.read_exact(&mut port).await?;

            // 3. Yanıt: başarılı + IPv4 0.0.0.0:0 (BND.ADDR/PORT anlamsız).
            sock.write_all(&[SOCKS_VERSION, REP_SUCCESS, RSV, ATYP_IPV4])
                .await?;
            sock.write_all(&[0u8; 4]).await?; // BND.ADDR
            sock.write_all(&[0u8; 2]).await?; // BND.PORT

            Ok::<_, anyhow::Error>(())
        });

        // İstemci tarafı: proxy'ye bağlan ve `connect()` çağır.
        let mut client = TcpStream::connect(proxy_addr).await?;
        crate::socks5::connect(&mut client, target).await?;
        proxy.await??;
        Ok(())
    }

    /// Proxy "no auth" metodunu reddederse `connect()` hata vermeli.
    #[tokio::test]
    async fn socks5_rejects_auth_required() -> Result<()> {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let proxy_addr = listener.local_addr()?;
        let target: SocketAddr = "127.0.0.1:8080".parse()?;

        let proxy = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await?;
            // Metot isteğini oku.
            let mut buf = [0u8; 3];
            sock.read_exact(&mut buf).await?;
            // Yanıt: "no methods acceptable" (0xFF).
            sock.write_all(&[SOCKS_VERSION, 0xFF]).await?;
            Ok::<_, anyhow::Error>(())
        });

        let mut client = TcpStream::connect(proxy_addr).await?;
        let result = crate::socks5::connect(&mut client, target).await;
        assert!(result.is_err(), "auth-gerektiren proxy reddedilmeli");
        proxy.await??;
        Ok(())
    }

    /// Proxy `REP != 0x00` dönerse `connect()` hata vermeli.
    #[tokio::test]
    async fn socks5_propagates_connect_failure() -> Result<()> {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let proxy_addr = listener.local_addr()?;
        let target: SocketAddr = "127.0.0.1:8080".parse()?;

        let proxy = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await?;
            // Metot anlaşması: OK.
            let mut buf = [0u8; 3];
            sock.read_exact(&mut buf).await?;
            sock.write_all(&[SOCKS_VERSION, METHOD_NO_AUTH]).await?;
            // CONNECT isteğini oku ama başarısız yanıt ver (REP=0x05).
            let mut head = [0u8; 4];
            sock.read_exact(&mut head).await?;
            let addr_len = if head[3] == ATYP_IPV4 { 4 } else { 16 };
            let mut addr = vec![0u8; addr_len];
            sock.read_exact(&mut addr).await?;
            let mut port = [0u8; 2];
            sock.read_exact(&mut port).await?;
            // REP = 0x05 (connection refused).
            sock.write_all(&[SOCKS_VERSION, 0x05, RSV, ATYP_IPV4]).await?;
            sock.write_all(&[0u8; 4]).await?;
            sock.write_all(&[0u8; 2]).await?;
            Ok::<_, anyhow::Error>(())
        });

        let mut client = TcpStream::connect(proxy_addr).await?;
        let result = crate::socks5::connect(&mut client, target).await;
        assert!(result.is_err(), "REP != 0x00 reddedilmeli");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("connection refused"), "hata mesajı REP'i içermeli: {}", err);
        proxy.await??;
        Ok(())
    }
}
