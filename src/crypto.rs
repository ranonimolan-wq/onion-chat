// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Kriptografi modülü — X25519 ECDH anahtar değişimi ve AES-256-GCM şifreleme.
//!
//! Bu modül OnionChat'ın uçtan uca şifreleme (E2EE) katmanını sağlar.
//! Her bağlantı için geçici (ephemeral) bir X25519 anahtar çifti üretilir,
//! karşı tarafın genel anahtarıyla ECDH uygulanarak 32 baytlık ortak sır
//! elde edilir. Bu sır, AES-256-GCM'in 12 baytlık rastgele nonce ile
//! kullandığı anahtar olarak işlev görür.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use rand::{rngs::OsRng, RngCore};
use x25519_dalek::{EphemeralSecret, PublicKey};

/// Yeni bir X25519 geçici anahtar çifti üretir.
///
/// Dönen `EphemeralSecret` tüketilir (X25519-Dalek API'si tasarım gereği
/// `Drop` ile sıfırlanır), bu yüzden güvenli biçimde ele alınmalıdır.
pub fn generate_key() -> (EphemeralSecret, PublicKey) {
    let secret = EphemeralSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    (secret, public)
}

/// 32 baytlık rastgele bir oda (room) anahtarı üretir.
///
/// AES-GCM için doğrudan anahtar olarak kullanılabilir; bağlantı
/// kurulmadan önce peer'lere güvenli bir kanaldan dağıtılmalıdır.
#[allow(dead_code)]
pub fn generate_room_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    let mut rng = OsRng;
    rng.fill_bytes(&mut key);
    key
}

/// Kendi gizli anahtarımızla peer'ın genel anahtarı üzerinden ECDH
/// uygulayarak 32 baytlık ortak sırı hesaplar.
///
/// Dönen değer AES-256-GCM için doğrudan anahtar olarak kullanılabilir.
pub fn compute_shared_secret(secret: EphemeralSecret, peer_pub: &PublicKey) -> [u8; 32] {
    let shared_secret = secret.diffie_hellman(peer_pub);
    *shared_secret.as_bytes()
}

/// Veriyi AES-GCM ile şifreler, rastgele 12 baytlık bir nonce üretir
/// ve çıktıyı `nonce || ciphertext` formatında döndürür.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow!("invalid AES key: {}", e))?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| anyhow!("encryption failed: {}", e))?;
    let mut result = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// `nonce || ciphertext` formatındaki girdiyi AES-GCM ile çözer
/// ve düz metni döndürür. Doğrulama başarısız olursa hata verir.
pub fn decrypt(key: &[u8; 32], ciphertext_with_nonce: &[u8]) -> Result<Vec<u8>> {
    if ciphertext_with_nonce.len() < 12 {
        return Err(anyhow!("ciphertext too short"));
    }
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow!("invalid AES key: {}", e))?;
    let nonce = Nonce::from_slice(&ciphertext_with_nonce[..12]);
    let ciphertext = &ciphertext_with_nonce[12..];
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("decryption failed: {}", e))?;
    Ok(plaintext)
}