// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Ağ katmanı — TCP üzerinden uçtan uca şifreli bağlantı yönetimi.
//!
//! Bu modül OnionChat'ın transport katmanıdır. Sorumlulukları:
//! - TCP dinleyici açmak ve gelen bağlantıları kabul etmek
//! - Uzak bir adrese TCP bağlantısı açmak
//! - X25519 anahtar değişimi (ECDHE) yaparak paylaşılan sırı türetmek
//! - Mesajların uzunluk öneki (length-prefixed) ile çerçevelenmesini sağlamak
//! - Bir `Connection`'ı okuma/yazma yarımlarına ayırmak (`split`)
//!
//! Dosya transferi, dosya adı doğrulama ve disk I/O gibi konular bu modülde
//! yer almaz; bunlar `file_transfer` modülünün sorumluluğundadır.
//!
//! ## Model
//! `Connection` ham `TcpStream` üzerinde tek-tutucu (single-owner) bir
//! sarmalayıcıdır; `&mut self` ile sıralı mesajlaşma için idealdir.
//! Birden fazla async görev aynı `Connection` üzerinden mesajlaşmak
//! istediğinde `split()` çağrılarak `Sender`/`Receiver` yarımları
//! elde edilir. Bu yarımlar kendi başlarına yazma/okuma görevlerini
//! çalıştırır; ham TCP akışı onlara taşınır.

use anyhow::Result;
use futures::{Stream, StreamExt};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, tcp::OwnedReadHalf, tcp::OwnedWriteHalf};
use tokio::sync::mpsc;
use crate::crypto;

/// Tek bir peer'a kurulmuş şifreli bağlantı.
///
/// Bağlantı tek bir `TcpStream` üzerinde tek sahipli (single-owner) bir
/// sarmalayıcıdır; bu nedenle `&mut self` ile sıralı olarak kullanılır.
/// Birden fazla async görev tarafından paylaşılmak istenirse `split()`
/// çağrılarak `Sender`/`Receiver` çiftine dönüştürülür.
pub struct Connection {
    stream: TcpStream,
    key: [u8; 32],
}

impl Connection {
    /// Uzak bir adrese TCP bağlantısı açar, ECDHE anahtar değişimini
    /// tamamlar ve isteğe bağlı olarak bir dosya gönderir.
    ///
    /// Bu metod `network` modülünün `file_transfer` modülüne karşı
    /// önceden var olan bir kolaylık (convenience) bağımlılığıdır.
    /// `chunk_size` ve `progress` parametreleri `file_transfer::send_file`'a
    /// varsayılan değerlerle (`DEFAULT_CHUNK_SIZE`, `NoProgress`) iletilir;
    /// bu seviyede özelleştirme gerekiyorsa `main.rs` doğrudan
    /// `Connection::connect(addr, None)` + `file_transfer::send_file` çağrı
    /// zincirini kullanmalıdır.
    pub async fn connect(addr: SocketAddr, file: Option<PathBuf>) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        let mut conn = Self::handshake(stream).await?;
        if let Some(path) = file {
            crate::file_transfer::send_file(
                &mut conn,
                &path,
                crate::file_transfer::DEFAULT_CHUNK_SIZE,
                &crate::file_transfer::NoProgress,
            )
            .await?;
        }
        Ok(conn)
    }

    /// Uzak bir adrese SOCKS5 proxy üzerinden TCP bağlantısı açar,
    /// ECDHE anahtar değişimini tamamlar ve isteğe bağlı olarak bir
    /// dosya gönderir.
    ///
    /// Bu metod `socks5` modülünü kullanarak `socks5_addr`'a bağlanır,
    /// proxy üzerinden `target` adresine tünel açar ve sonra normal
    /// ECDHE el sıkışmasını çalıştırır. Anonim ağ kullanımı
    /// (örn. Tor `127.0.0.1:9050`) için tasarlanmıştır.
    ///
    /// Sock5 trafiği şifresizdir; bu yüzden asıl veri aktarımı her
    /// zaman `Connection` üzerinden AES-GCM ile uçtan uca şifrelidir.
    /// SOCKS5 sadece "peer'ın IP'sini gizle" görevini üstlenir.
    pub async fn connect_via_socks5(
        target: SocketAddr,
        socks5_addr: SocketAddr,
        file: Option<PathBuf>,
    ) -> Result<Self> {
        // 1. SOCKS5 proxy'ye TCP bağlan.
        let mut stream = TcpStream::connect(socks5_addr).await?;
        // 2. SOCKS5 el sıkışması: proxy üzerinden hedefe tünel aç.
        crate::socks5::connect(&mut stream, target).await?;
        // 3. Tünellenmiş akış üzerinden normal ECDHE el sıkışması.
        let mut conn = Self::handshake(stream).await?;
        // 4. İsteğe bağlı dosya gönderimi.
        if let Some(path) = file {
            crate::file_transfer::send_file(
                &mut conn,
                &path,
                crate::file_transfer::DEFAULT_CHUNK_SIZE,
                &crate::file_transfer::NoProgress,
            )
            .await?;
        }
        Ok(conn)
    }

    /// Daha önce kabul edilmiş bir TCP akışından ECDHE anahtar değişimi
    /// yaparak `Connection` üretir.
    pub async fn from_stream(stream: TcpStream) -> Result<Self> {
        Self::handshake(stream).await
    }

    /// İki tarafın da aynı sırayı izlediği ECDHE el sıkışmasını çalıştırır.
    async fn handshake(mut stream: TcpStream) -> Result<Self> {
        let (our_secret, our_public) = crypto::generate_key();
        stream.write_all(our_public.as_bytes()).await?;
        let mut their_pubkey = [0u8; 32];
        stream.read_exact(&mut their_pubkey).await?;
        let their_public = x25519_dalek::PublicKey::from(their_pubkey);
        let key = crypto::compute_shared_secret(our_secret, &their_public);
        Ok(Self { stream, key })
    }

    /// Bir mesajı AES-GCM ile şifreler ve 4 bayt uzunluk öneki ile birlikte
    /// karşı tarafa gönderir. Çerçeve biçimi: `[len: u32 BE][nonce|ciphertext]`.
    pub async fn send_message(&mut self, msg: &[u8]) -> Result<()> {
        let ciphertext = crypto::encrypt(&self.key, msg)?;
        let mut buf = Vec::with_capacity(4 + ciphertext.len());
        buf.extend_from_slice(&(ciphertext.len() as u32).to_be_bytes());
        buf.extend_from_slice(&ciphertext);
        self.stream.write_all(&buf).await?;
        Ok(())
    }

    /// Uzunluk önekli bir çerçeveyi okur, AES-GCM ile çözer ve düz metni
    /// döndürür. Hata durumunda (boyut uyuşmazlığı veya kimlik doğrulama
    /// başarısızlığı) hata döner.
    pub async fn recv_message(&mut self) -> Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut msg_buf = vec![0u8; len];
        self.stream.read_exact(&mut msg_buf).await?;
        let plaintext = crypto::decrypt(&self.key, &msg_buf)?;
        Ok(plaintext)
    }

    /// Bağlantıyı okuma ve yazma yarılarına ayırır.
    ///
    /// `TcpStream`'i `OwnedReadHalf`/`OwnedWriteHalf` olarak iki ayrı
    /// göreve böler. Her iki görev de kendi kanalı üzerinden gelen
    /// mesajları işler; okuma görevi düz metni `Receiver`'a, yazma
    /// görevi ise `Sender`'dan gelen düz metni şifreleyerek akışa aktarır.
    #[allow(dead_code)]
    pub fn split(self) -> (Sender, Receiver) {
        split_connection(self.stream, self.key)
    }
}

/// Ham `TcpStream` + paylaşılan anahtardan yarımları üreten yardımcı.
/// Hem `Connection::split` hem de `listen` tarafından çağrılır; böylece
/// `TcpStream`'i `into_split` ettikten sonra geri birleştirmek zorunda
/// kalmadan her iki çağrı yolu da çalışır.
#[allow(dead_code)]
fn split_connection(stream: TcpStream, key: [u8; 32]) -> (Sender, Receiver) {
    let (mut read_half, mut write_half) = stream.into_split();

    let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(32);
    let (in_tx, in_rx) = mpsc::channel::<Vec<u8>>(32);

    // Yazma görevi: düz metin → şifrele → çerçevele → akışa yaz.
    tokio::spawn(async move {
        while let Some(plaintext) = out_rx.recv().await {
            let ciphertext = match crypto::encrypt(&key, &plaintext) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("şifreleme hatası: {}", e);
                    break;
                }
            };
            let mut buf = Vec::with_capacity(4 + ciphertext.len());
            buf.extend_from_slice(&(ciphertext.len() as u32).to_be_bytes());
            buf.extend_from_slice(&ciphertext);
            if write_half.write_all(&buf).await.is_err() {
                break;
            }
        }
    });

    // Okuma görevi: çerçeveleri oku → çöz → kanala aktar.
    tokio::spawn(async move {
        loop {
            let mut len_buf = [0u8; 4];
            if read_half.read_exact(&mut len_buf).await.is_err() {
                break;
            }
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut msg_buf = vec![0u8; len];
            if read_half.read_exact(&mut msg_buf).await.is_err() {
                break;
            }
            let plaintext = match crypto::decrypt(&key, &msg_buf) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("şifre çözme hatası: {}", e);
                    break;
                }
            };
            if in_tx.send(plaintext).await.is_err() {
                break;
            }
        }
    });

    (Sender { tx: out_tx }, Receiver { rx: in_rx })
}

/// `Connection::split` tarafından üretilen yazma yarısı.
pub struct Sender {
    #[allow(dead_code)]
    tx: mpsc::Sender<Vec<u8>>,
}

impl Sender {
    /// Bir plaintext mesajı kuyruğa alır; arka plandaki görev onu şifreler
    /// ve TCP üzerinden gönderir.
    #[allow(dead_code)]
    pub async fn send(&self, msg: Vec<u8>) -> Result<()> {
        self.tx
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!("send channel closed: {}", e))
    }
}

/// `Connection::split` tarafından üretilen okuma yarısı.
///
/// Çözülmüş plaintext mesajları bir `Stream` olarak dışa aktarır.
pub struct Receiver {
    rx: mpsc::Receiver<Vec<u8>>,
}

impl Stream for Receiver {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().rx).poll_recv(cx)
    }
}

/// Belirtilen adreste gelen bağlantıları kabul eder; her bağlantı için
/// ECDHE anahtar değişimi tamamlanmış bir `Sender`/`Receiver` çifti
/// üreten bir akış döner.
///
/// Şu an `main.rs` tek bağlantılı senaryolar için doğrudan `TcpListener`
/// kullanıyor; bu fonksiyon ileride çoklu-peer (multi-peer) senaryoları
/// için `Connection::pool` veya UI event loop'unda kullanılacak.
#[allow(dead_code)]
pub async fn listen(addr: SocketAddr) -> Result<impl Stream<Item = Result<(Sender, Receiver)>>> {
    let listener = TcpListener::bind(addr).await?;
    let stream = tokio_stream::wrappers::TcpListenerStream::new(listener)
        .map(|result| async move {
            let stream = result?;
            // ECDHE'yi `TcpStream` üzerinden elle yürütüp `split`'e geçiriyoruz.
            let (mut read_half, mut write_half) = stream.into_split();
            let (our_secret, our_public) = crypto::generate_key();
            write_half.write_all(our_public.as_bytes()).await?;
            let mut their_pubkey = [0u8; 32];
            read_half.read_exact(&mut their_pubkey).await?;
            let their_public = x25519_dalek::PublicKey::from(their_pubkey);
            let key = crypto::compute_shared_secret(our_secret, &their_public);

            // Yarımları `TcpStream` gibi davranan bir sarmalayıcıya
            // veremiyoruz; bunun yerine yarımlarla çalışan özel bir
            // kanal-eşleme (channel-mediated) sürümünü çağırıyoruz.
            Ok(split_halves(read_half, write_half, key))
        })
        .buffer_unordered(10);
    Ok(stream)
}

/// Yarımlardan doğrudan `Sender`/`Receiver` üreten yardımcı.
#[allow(dead_code)]
fn split_halves(
    mut read_half: OwnedReadHalf,
    mut write_half: OwnedWriteHalf,
    key: [u8; 32],
) -> (Sender, Receiver) {
    let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(32);
    let (in_tx, in_rx) = mpsc::channel::<Vec<u8>>(32);

    tokio::spawn(async move {
        while let Some(plaintext) = out_rx.recv().await {
            let ciphertext = match crypto::encrypt(&key, &plaintext) {
                Ok(c) => c,
                Err(_) => break,
            };
            let mut buf = Vec::with_capacity(4 + ciphertext.len());
            buf.extend_from_slice(&(ciphertext.len() as u32).to_be_bytes());
            buf.extend_from_slice(&ciphertext);
            if write_half.write_all(&buf).await.is_err() {
                break;
            }
        }
    });

    tokio::spawn(async move {
        loop {
            let mut len_buf = [0u8; 4];
            if read_half.read_exact(&mut len_buf).await.is_err() {
                break;
            }
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut msg_buf = vec![0u8; len];
            if read_half.read_exact(&mut msg_buf).await.is_err() {
                break;
            }
            let plaintext = match crypto::decrypt(&key, &msg_buf) {
                Ok(p) => p,
                Err(_) => break,
            };
            if in_tx.send(plaintext).await.is_err() {
                break;
            }
        }
    });

    (Sender { tx: out_tx }, Receiver { rx: in_rx })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tek bir `Connection` üzerinden şifreli mesaj gönderip almayı doğrular.
    #[tokio::test]
    async fn test_message_round_trip() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut conn = Connection::from_stream(stream).await?;
            let got = conn.recv_message().await?;
            assert_eq!(got, b"ping");
            conn.send_message(b"pong").await?;
            Ok::<_, anyhow::Error>(())
        });

        let mut client = Connection::connect(addr, None).await?;
        client.send_message(b"ping").await?;
        let got = client.recv_message().await?;
        assert_eq!(got, b"pong");
        server.await??;
        Ok(())
    }

    /// `split()` ile üretilen `Sender`/`Receiver` yarımlarının mesaj
    /// iletebildiğini doğrular.
    #[tokio::test]
    async fn test_split_round_trip() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let (tx, mut rx) = Connection::from_stream(stream).await?.split();
            while let Some(msg) = rx.next().await {
                tx.send(msg).await?;
            }
            Ok::<_, anyhow::Error>(())
        });

        let conn = Connection::connect(addr, None).await?;
        let (tx, mut rx) = conn.split();
        tx.send(b"ping".to_vec()).await?;
        let got = rx.next().await.unwrap();
        assert_eq!(got, b"ping");
        drop(server);
        Ok(())
    }
}
