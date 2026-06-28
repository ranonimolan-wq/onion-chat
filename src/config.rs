// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Yapılandırma modülü — `config.json` yükler, kaydeder, yönetir.
//!
//! OnionChat'ın tüm ayarları tek bir JSON dosyasından yönetilir. Bu
//! modül dosyayı yükler, eksik alanları default değerlerle doldurur,
//! ve runtime'da değişiklikleri kaydeder.
//!
//! ## Varsayılan davranış (anonymity-first)
//!
//! - **History**: DEFAULT OFF — kullanıcı istemedikçe mesaj kaydedilmez
//! - **Peer adresleri**: gösterilmez (anonymity)
//! - **Rol sistemi**: aktif, ilk hub operator admin
//! - **User yetkileri**: minimum (clear/kick/mute sadece admin/mod)
//!
//! ## config.json yolu
//!
//! Öncelik sırası:
//! 1. `--config <path>` CLI bayrağı
//! 2. `~/.onionchat/config.json` (default)
//! 3. `./onionchat-config.json` (HOME yoksa fallback)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Ana yapılandırma struct'ı. Tüm ayarlar burada toplanır.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Mesaj geçmişi ayarları.
    #[serde(default)]
    pub history: HistoryConfig,

    /// Sunucu (hub) ayarları.
    #[serde(default)]
    pub server: ServerConfig,

    /// Rol ve yetki ayarları.
    #[serde(default)]
    pub roles: RolesConfig,

    /// Anonimlik ayarları.
    #[serde(default)]
    pub anonymity: AnonymityConfig,

    /// Komut yetki ayarları (hangi roller hangi komutları kullanabilir).
    #[serde(default)]
    pub permissions: PermissionsConfig,
}

/// Mesaj geçmişi ayarları. **DEFAULT OFF** — anonymity prensibi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// History açıksa mesajlar diske kaydedilir. **Default: false**.
    #[serde(default = "default_history_enabled")]
    pub enabled: bool,

    /// History dosyasının yolu. `~` otomatik genişletilir.
    #[serde(default = "default_history_path")]
    pub path: PathBuf,

    /// TUI açılışında yüklenecek max mesaj sayısı.
    #[serde(default = "default_max_messages_loaded")]
    pub max_messages_loaded: usize,
}

fn default_history_enabled() -> bool {
    false
}

fn default_history_path() -> PathBuf {
    PathBuf::from("~/.onionchat/history.jsonl")
}

fn default_max_messages_loaded() -> usize {
    50
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_history_enabled(),
            path: default_history_path(),
            max_messages_loaded: default_max_messages_loaded(),
        }
    }
}

/// Sunucu (hub) ayarları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Maksimum eş zamanlı peer sayısı. Sunucu gücü kadar kişi katılsın.
    #[serde(default = "default_max_peers")]
    pub max_peers: usize,

    /// Hub görünen adı (TUI başlığında gösterilir).
    #[serde(default = "default_server_name")]
    pub name: String,
}

fn default_max_peers() -> usize {
    100
}

fn default_server_name() -> String {
    "OnionChat Hub".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_peers: default_max_peers(),
            name: default_server_name(),
        }
    }
}

/// Rol ve yetki ayarları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolesConfig {
    /// Rol sistemi aktif mi? Kapalıysa tüm komutlar herkese açık.
    #[serde(default = "default_roles_enabled")]
    pub enabled: bool,

    /// İlk bağlanan peer admin olsun mu? (hub operator her zaman admin)
    #[serde(default = "default_first_user_is_admin")]
    pub first_user_is_admin: bool,
}

fn default_roles_enabled() -> bool {
    true
}

fn default_first_user_is_admin() -> bool {
    false
}

impl Default for RolesConfig {
    fn default() -> Self {
        Self {
            enabled: default_roles_enabled(),
            first_user_is_admin: default_first_user_is_admin(),
        }
    }
}

/// Anonimlik ayarları. **Anonymity esastır.**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonymityConfig {
    /// Tor zorunlu olsun mu? (true ise SOCKS5 olmadan bağlantı reddedilir)
    #[serde(default)]
    pub require_tor: bool,

    /// Metadata temizleme: peer adreslerini gösterme.
    #[serde(default = "default_strip_metadata")]
    pub strip_metadata: bool,

    /// Peer adreslerini TUI'da göster (sadece debug için).
    #[serde(default)]
    pub show_peer_addresses: bool,
}

fn default_strip_metadata() -> bool {
    true
}

impl Default for AnonymityConfig {
    fn default() -> Self {
        Self {
            require_tor: false,
            strip_metadata: default_strip_metadata(),
            show_peer_addresses: false,
        }
    }
}

/// Komut yetki ayarları. Hangi roller hangi komutları kullanabilir.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionsConfig {
    /// User rolü `/clear` kullanabilir mi?
    #[serde(default)]
    pub allow_user_clear: bool,

    /// User rolü `/kick` kullanabilir mi?
    #[serde(default)]
    pub allow_user_kick: bool,

    /// User rolü `/mute` kullanabilir mi?
    #[serde(default)]
    pub allow_user_mute: bool,

    /// User rolü `/config set` kullanabilir mi?
    #[serde(default)]
    pub allow_user_change_config: bool,
}

/// Varsayılan config dosyasının yolu: `~/.onionchat/config.json`.
pub fn default_config_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".onionchat");
        p.push("config.json");
        p
    } else {
        PathBuf::from("onionchat-config.json")
    }
}

/// `~` ile başlayan yolu `HOME` çevre değişkeni ile genişletir.
/// `~` yoksa veya `HOME` yoksa orijinal yolu döndürür.
pub fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        let mut p = PathBuf::from(home);
        p.push(rest);
        return p;
    } else if s == "~"
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home);
    }
    path.to_path_buf()
}

/// Bir config dosyasını yükler. Dosya yoksa default config döner ve
/// dosya oluşturulur (save ile çağrılırsa).
///
/// Eksik alanlar default değerlerle doldurulur (serde `#[serde(default)]`).
pub async fn load(path: &Path) -> Result<Config> {
    let expanded = expand_tilde(path);
    match tokio::fs::read_to_string(&expanded).await {
        Ok(contents) => {
            let config: Config = serde_json::from_str(&contents).map_err(|e| {
                anyhow!("config.json parse hatası: {}", e)
            })?;
            Ok(config)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(
                "config dosyası bulunamadı {}, default kullanılıyor",
                expanded.display()
            );
            Ok(Config::default())
        }
        Err(e) => Err(anyhow!("config okuma hatası: {}", e)),
    }
}

/// Config'i dosyaya kaydeder. Üst dizin yoksa oluşturur.
pub async fn save(path: &Path, config: &Config) -> Result<()> {
    let expanded = expand_tilde(path);
    if let Some(parent) = expanded.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let json = serde_json::to_string_pretty(config)?;
    tokio::fs::write(&expanded, json).await?;
    Ok(())
}

/// Config dosyası yoksa default config oluşturup kaydeder.
/// Varsa yükler. Bu, ilk başlatmada kullanıcıya örnek config verir.
pub async fn load_or_create(path: &Path) -> Result<Config> {
    let expanded = expand_tilde(path);
    if !expanded.exists() {
        let config = Config::default();
        save(path, &config).await?;
        tracing::info!("Default config oluşturuldu: {}", expanded.display());
        Ok(config)
    } else {
        load(path).await
    }
}

/// Config'i JSON string olarak döndürür (pretty-printed).
/// `/config` komutu için kullanılır.
pub fn to_pretty_json(config: &Config) -> Result<String> {
    serde_json::to_string_pretty(config).map_err(|e| anyhow!("JSON serialize hatası: {}", e))
}

/// Bir config alanını noktayla ayrılmış yolla ayarla.
/// Örn: `set("history.enabled", "true")`.
///
/// Bu fonksiyon config'i yerinde değiştirir ama dosyaya kaydetmez.
/// Kaydetmek için `save` çağrılmalı.
pub fn set_field(config: &mut Config, key: &str, value: &str) -> Result<()> {
    match key {
        "history.enabled" => {
            config.history.enabled = parse_bool(value)?;
        }
        "history.path" => {
            config.history.path = PathBuf::from(value);
        }
        "history.max_messages_loaded" => {
            config.history.max_messages_loaded = value
                .parse()
                .map_err(|e| anyhow!("geçersiz sayı: {}", e))?;
        }
        "server.max_peers" => {
            config.server.max_peers = value
                .parse()
                .map_err(|e| anyhow!("geçersiz sayı: {}", e))?;
        }
        "server.name" => {
            config.server.name = value.to_string();
        }
        "roles.enabled" => {
            config.roles.enabled = parse_bool(value)?;
        }
        "roles.first_user_is_admin" => {
            config.roles.first_user_is_admin = parse_bool(value)?;
        }
        "anonymity.require_tor" => {
            config.anonymity.require_tor = parse_bool(value)?;
        }
        "anonymity.strip_metadata" => {
            config.anonymity.strip_metadata = parse_bool(value)?;
        }
        "anonymity.show_peer_addresses" => {
            config.anonymity.show_peer_addresses = parse_bool(value)?;
        }
        "permissions.allow_user_clear" => {
            config.permissions.allow_user_clear = parse_bool(value)?;
        }
        "permissions.allow_user_kick" => {
            config.permissions.allow_user_kick = parse_bool(value)?;
        }
        "permissions.allow_user_mute" => {
            config.permissions.allow_user_mute = parse_bool(value)?;
        }
        "permissions.allow_user_change_config" => {
            config.permissions.allow_user_change_config = parse_bool(value)?;
        }
        _ => {
            return Err(anyhow!(
                "bilinmeyen config anahtarı: '{}'. Geçerli anahtarlar: \
                 history.enabled, history.path, history.max_messages_loaded, \
                 server.max_peers, server.name, roles.enabled, \
                 roles.first_user_is_admin, anonymity.require_tor, \
                 anonymity.strip_metadata, anonymity.show_peer_addresses, \
                 permissions.allow_user_clear, permissions.allow_user_kick, \
                 permissions.allow_user_mute, permissions.allow_user_change_config",
                key
            ));
        }
    }
    Ok(())
}

/// String'i bool'a çevirir. `true`/`false`, `1`/`0`, `yes`/`no` kabul eder.
fn parse_bool(s: &str) -> Result<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" | "evet" => Ok(true),
        "false" | "0" | "no" | "off" | "hayir" | "hayır" => Ok(false),
        _ => Err(anyhow!(
            "geçersiz bool değeri: '{}'. true/false, 1/0, yes/no kabul edilir",
            s
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("onionchat-config-test-{}-{}.json", std::process::id(), id));
        p
    }

    #[test]
    fn default_config_has_history_disabled() {
        let c = Config::default();
        assert!(!c.history.enabled, "history default OFF olmalı (anonymity)");
    }

    #[test]
    fn default_config_strip_metadata_true() {
        let c = Config::default();
        assert!(c.anonymity.strip_metadata, "strip_metadata default true");
    }

    #[test]
    fn default_config_show_peer_addresses_false() {
        let c = Config::default();
        assert!(!c.anonymity.show_peer_addresses, "peer adresleri default gizli");
    }

    #[test]
    fn default_config_roles_enabled() {
        let c = Config::default();
        assert!(c.roles.enabled, "rol sistemi default aktif");
    }

    #[test]
    fn default_config_first_user_is_admin_false() {
        let c = Config::default();
        assert!(!c.roles.first_user_is_admin, "ilk kullanıcı default admin değil");
    }

    #[test]
    fn default_config_max_peers_100() {
        let c = Config::default();
        assert_eq!(c.server.max_peers, 100);
    }

    #[test]
    fn default_config_server_name() {
        let c = Config::default();
        assert_eq!(c.server.name, "OnionChat Hub");
    }

    #[test]
    fn default_config_permissions_all_false() {
        let c = Config::default();
        assert!(!c.permissions.allow_user_clear);
        assert!(!c.permissions.allow_user_kick);
        assert!(!c.permissions.allow_user_mute);
        assert!(!c.permissions.allow_user_change_config);
    }

    #[test]
    fn default_config_history_path_contains_onionchat() {
        let c = Config::default();
        let path_str = c.history.path.to_string_lossy();
        assert!(path_str.contains(".onionchat"));
        assert!(path_str.contains("history.jsonl"));
    }

    #[test]
    fn parse_bool_accepts_true_variants() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("TRUE").unwrap());
        assert!(parse_bool("1").unwrap());
        assert!(parse_bool("yes").unwrap());
        assert!(parse_bool("on").unwrap());
        assert!(parse_bool("evet").unwrap());
    }

    #[test]
    fn parse_bool_accepts_false_variants() {
        assert!(!(parse_bool("false").unwrap()));
        assert!(!(parse_bool("FALSE").unwrap()));
        assert!(!(parse_bool("0").unwrap()));
        assert!(!(parse_bool("no").unwrap()));
        assert!(!(parse_bool("off").unwrap()));
        assert!(!(parse_bool("hayir").unwrap()));
        assert!(!(parse_bool("hayır").unwrap()));
    }

    #[test]
    fn parse_bool_rejects_invalid() {
        assert!(parse_bool("maybe").is_err());
        assert!(parse_bool("").is_err());
        assert!(parse_bool("2").is_err());
    }

    #[test]
    fn expand_tilde_with_home_prefix() {
        // Rule 1: Zero Unsafe Policy — set_var Rust 2024'te unsafe.
        // Bu nedenle HOME zaten set ise test ederiz, değilse atlarız.
        if let Some(home) = std::env::var_os("HOME") {
            let test_path = Path::new("~/test/file.txt");
            let expanded = expand_tilde(test_path);
            let mut expected = PathBuf::from(&home);
            expected.push("test/file.txt");
            assert_eq!(expanded, expected);
        }
    }

    #[test]
    fn expand_tilde_just_tilde() {
        if let Some(home) = std::env::var_os("HOME") {
            let expanded = expand_tilde(Path::new("~"));
            assert_eq!(expanded, PathBuf::from(&home));
        }
    }

    #[test]
    fn expand_tilde_no_tilde_returns_as_is() {
        let p = expand_tilde(Path::new("/absolute/path"));
        assert_eq!(p, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn expand_tilde_relative_path() {
        let p = expand_tilde(Path::new("relative/path"));
        assert_eq!(p, PathBuf::from("relative/path"));
    }

    #[tokio::test]
    async fn save_and_load_roundtrip() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        let config = Config {
            history: HistoryConfig {
                enabled: true,
                path: PathBuf::from("/custom/path.jsonl"),
                max_messages_loaded: 100,
            },
            ..Config::default()
        };
        save(&path, &config).await?;

        let loaded = load(&path).await?;
        assert!(loaded.history.enabled);
        assert_eq!(loaded.history.path, PathBuf::from("/custom/path.jsonl"));
        assert_eq!(loaded.history.max_messages_loaded, 100);

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_nonexistent_returns_default() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;
        let config = load(&path).await?;
        assert!(!config.history.enabled);
        Ok(())
    }

    #[tokio::test]
    async fn load_or_create_creates_file_if_missing() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;
        assert!(!path.exists());

        let config = load_or_create(&path).await?;
        assert!(path.exists(), "config dosyası oluşturulmalı");
        assert!(!config.history.enabled, "default config history OFF");

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_or_create_loads_existing() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        // Önce özel config kaydet
        let custom = Config {
            history: HistoryConfig {
                enabled: true,
                ..HistoryConfig::default()
            },
            ..Config::default()
        };
        save(&path, &custom).await?;

        // Sonra load_or_create ile yükle
        let loaded = load_or_create(&path).await?;
        assert!(loaded.history.enabled, "mevcut config yüklenmeli");

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_partial_config_uses_defaults_for_missing() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        // Sadece history.enabled içeren kısmi config
        let partial = r#"{"history": {"enabled": true}}"#;
        tokio::fs::write(&path, partial).await?;

        let loaded = load(&path).await?;
        assert!(loaded.history.enabled, "verilen değer korunmalı");
        assert_eq!(
            loaded.history.max_messages_loaded, 50,
            "eksik alan default ile doldurulmalı"
        );
        assert!(loaded.roles.enabled, "roles default true");
        assert!(!loaded.permissions.allow_user_clear, "permissions default false");

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_empty_json_uses_all_defaults() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        tokio::fs::write(&path, "{}").await?;
        let loaded = load(&path).await?;
        assert!(!loaded.history.enabled);
        assert_eq!(loaded.server.max_peers, 100);

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[tokio::test]
    async fn load_invalid_json_returns_error() -> Result<()> {
        let path = test_path();
        let _ = tokio::fs::remove_file(&path).await;

        tokio::fs::write(&path, "not valid json").await?;
        assert!(load(&path).await.is_err());

        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }

    #[test]
    fn set_field_history_enabled() {
        let mut c = Config::default();
        set_field(&mut c, "history.enabled", "true").unwrap();
        assert!(c.history.enabled);
        set_field(&mut c, "history.enabled", "false").unwrap();
        assert!(!c.history.enabled);
    }

    #[test]
    fn set_field_history_path() {
        let mut c = Config::default();
        set_field(&mut c, "history.path", "/custom/path.jsonl").unwrap();
        assert_eq!(c.history.path, PathBuf::from("/custom/path.jsonl"));
    }

    #[test]
    fn set_field_max_messages_loaded() {
        let mut c = Config::default();
        set_field(&mut c, "history.max_messages_loaded", "100").unwrap();
        assert_eq!(c.history.max_messages_loaded, 100);
    }

    #[test]
    fn set_field_invalid_number_returns_error() {
        let mut c = Config::default();
        assert!(set_field(&mut c, "history.max_messages_loaded", "abc").is_err());
    }

    #[test]
    fn set_field_server_max_peers() {
        let mut c = Config::default();
        set_field(&mut c, "server.max_peers", "500").unwrap();
        assert_eq!(c.server.max_peers, 500);
    }

    #[test]
    fn set_field_server_name() {
        let mut c = Config::default();
        set_field(&mut c, "server.name", "My Hub").unwrap();
        assert_eq!(c.server.name, "My Hub");
    }

    #[test]
    fn set_field_roles_enabled() {
        let mut c = Config::default();
        set_field(&mut c, "roles.enabled", "false").unwrap();
        assert!(!c.roles.enabled);
    }

    #[test]
    fn set_field_anonymity_require_tor() {
        let mut c = Config::default();
        set_field(&mut c, "anonymity.require_tor", "true").unwrap();
        assert!(c.anonymity.require_tor);
    }

    #[test]
    fn set_field_permissions_allow_user_clear() {
        let mut c = Config::default();
        set_field(&mut c, "permissions.allow_user_clear", "true").unwrap();
        assert!(c.permissions.allow_user_clear);
    }

    #[test]
    fn set_field_unknown_key_returns_error() {
        let mut c = Config::default();
        assert!(set_field(&mut c, "unknown.key", "value").is_err());
    }

    #[test]
    fn set_field_invalid_bool_returns_error() {
        let mut c = Config::default();
        assert!(set_field(&mut c, "history.enabled", "maybe").is_err());
    }

    #[test]
    fn to_pretty_json_returns_valid_json() {
        let c = Config::default();
        let json = to_pretty_json(&c).unwrap();
        assert!(json.contains("\"history\""));
        assert!(json.contains("\"enabled\""));
        assert!(json.contains("\"server\""));
        assert!(json.contains("\"roles\""));
    }

    #[test]
    fn config_serde_roundtrip_preserves_all_fields() {
        let c = Config {
            history: HistoryConfig {
                enabled: true,
                path: PathBuf::from("/test/path.jsonl"),
                max_messages_loaded: 25,
            },
            server: ServerConfig {
                max_peers: 200,
                name: "Test Hub".to_string(),
            },
            roles: RolesConfig {
                enabled: false,
                first_user_is_admin: true,
            },
            anonymity: AnonymityConfig {
                require_tor: true,
                strip_metadata: false,
                show_peer_addresses: true,
            },
            permissions: PermissionsConfig {
                allow_user_clear: true,
                allow_user_kick: true,
                allow_user_mute: false,
                allow_user_change_config: true,
            },
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert!(back.history.enabled);
        assert_eq!(back.history.max_messages_loaded, 25);
        assert_eq!(back.server.max_peers, 200);
        assert_eq!(back.server.name, "Test Hub");
        assert!(!back.roles.enabled);
        assert!(back.roles.first_user_is_admin);
        assert!(back.anonymity.require_tor);
        assert!(!back.anonymity.strip_metadata);
        assert!(back.anonymity.show_peer_addresses);
        assert!(back.permissions.allow_user_clear);
        assert!(back.permissions.allow_user_kick);
        assert!(!back.permissions.allow_user_mute);
        assert!(back.permissions.allow_user_change_config);
    }

    #[test]
    fn default_config_path_contains_onionchat() {
        // Rule 1: set_var unsafe — mevcut HOME ile test et.
        if std::env::var_os("HOME").is_some() {
            let p = default_config_path();
            let s = p.to_string_lossy();
            assert!(s.contains(".onionchat"));
            assert!(s.contains("config.json"));
        }
    }
}
