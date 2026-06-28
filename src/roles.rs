// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Rol ve yetki modülü — peer kimliği, roller, izin kontrolü.
//!
//! Bu modül OnionChat'ın rol-tabanlı erişim kontrolünü (RBAC) sağlar.
//! Anonimlik prensibi korunarak peer'lar IP yerine rastgele ID ile
//! tanımlanır.
//!
//! ## Roller
//!
//! - **Admin**: Tam yetki. Rol verme/almaz, kick, mute, config değiştirme.
//!   Hub operator (sunucuyu başlatan) her zaman admin'dir.
//! - **Moderator**: Kick, mute, clear yapabilir. Config değiştiremez.
//! - **User**: Temel sohbet. Config'den izin verilirse bazı komutlar.
//! - **Guest**: Sadece okuma (ileride, şimdilik User ile aynı).
//!
//! ## Anonimlik
//!
//! Peer'lar `peer-<id>` formatında rastgele ID ile gösterilir. Gerçek
//! IP adresleri `anonymity.show_peer_addresses: false` iken asla
//! gösterilmez. Nick isteğe bağlıdır ve `/nick` ile ayarlanır.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::config::PermissionsConfig;

/// Bir peer'ın rolü. Düşükten yükseğe: Guest < User < Moderator < Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Sadece okuma (ileride). Şimdilik User ile aynı yetkilere sahip.
    Guest,
    /// Temel kullanıcı. Sohbet edebilir, nick ayarlayabilir.
    User,
    /// Moderatör. Kick, mute, clear yapabilir.
    Moderator,
    /// Yönetici. Tam yetki, rol verebilir, config değiştirebilir.
    Admin,
}

impl Role {
    /// Rolün string karşılığı (TUI ve komutlar için).
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Guest => "guest",
            Role::User => "user",
            Role::Moderator => "moderator",
            Role::Admin => "admin",
        }
    }

    /// String'den Role'e çevir. Büyük/küçük harf duyarsız.
    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "guest" => Some(Role::Guest),
            "user" => Some(Role::User),
            "moderator" | "mod" => Some(Role::Moderator),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    /// `/clear` komutunu kullanabilir mi?
    #[allow(dead_code)]
    pub fn can_clear(&self, perms: &PermissionsConfig) -> bool {
        match self {
            Role::Admin | Role::Moderator => true,
            Role::User => perms.allow_user_clear,
            Role::Guest => false,
        }
    }

    /// `/kick` komutunu kullanabilir mi?
    #[allow(dead_code)]
    pub fn can_kick(&self, perms: &PermissionsConfig) -> bool {
        match self {
            Role::Admin | Role::Moderator => true,
            Role::User => perms.allow_user_kick,
            Role::Guest => false,
        }
    }

    /// `/mute` komutunu kullanabilir mi?
    #[allow(dead_code)]
    pub fn can_mute(&self, perms: &PermissionsConfig) -> bool {
        match self {
            Role::Admin | Role::Moderator => true,
            Role::User => perms.allow_user_mute,
            Role::Guest => false,
        }
    }

    /// `/config set` komutunu kullanabilir mi?
    #[allow(dead_code)]
    pub fn can_change_config(&self, perms: &PermissionsConfig) -> bool {
        match self {
            Role::Admin => true,
            Role::Moderator | Role::Guest => false,
            Role::User => perms.allow_user_change_config,
        }
    }

    /// `/on_admin` ile başkasına admin verebilir mi? Sadece admin.
    #[allow(dead_code)]
    pub fn can_grant_admin(&self) -> bool {
        matches!(self, Role::Admin)
    }

    /// `/off_admin` ile admin alabilir mi? Sadece admin.
    #[allow(dead_code)]
    pub fn can_revoke_admin(&self) -> bool {
        matches!(self, Role::Admin)
    }

    /// `/config` (görüntüleme) kullanabilir mi? Tüm roller görebilir.
    #[allow(dead_code)]
    pub fn can_view_config(&self) -> bool {
        true
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Bir peer'ın kimlik bilgileri. Anonimlik için gerçek IP gizlenir.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Rastgele session ID. Gerçek IP değil. Anonimlik için.
    pub id: u64,
    /// Peer'ın taktığı nick (varsa). `/nick` ile ayarlanır.
    pub nick: Option<String>,
    /// Peer'ın rolü.
    pub role: Role,
    /// Mute durumu. Mute edilmiş peer'ların mesajları kabul edilir ama
    /// broadcast edilmez.
    pub muted: bool,
    /// Gerçek socket adresi. Anonymity.show_peer_addresses false iken
    /// asla kullanıcıya gösterilmez.
    pub addr: SocketAddr,
}

impl PeerInfo {
    /// Yeni peer oluştur. Varsayılan rol User, mute değil.
    pub fn new(id: u64, addr: SocketAddr) -> Self {
        Self {
            id,
            nick: None,
            role: Role::User,
            muted: false,
            addr,
        }
    }

    /// Peer'ın görünen adını döndürür. Anonimlik ayarına bağlı olarak:
    /// - Nick varsa → nick
    /// - `show_addr` true ve nick yoksa → IP adresi
    /// - İkisi de yoksa → `peer-<id>`
    pub fn display_name(&self, show_addr: bool) -> String {
        if let Some(nick) = &self.nick {
            nick.clone()
        } else if show_addr {
            format!("{}", self.addr)
        } else {
            format!("peer-{:x}", self.id)
        }
    }

    /// Peer'ı belirli bir role yükselt.
    pub fn set_role(&mut self, role: Role) {
        self.role = role;
    }

    /// Peer'a nick ata.
    #[allow(dead_code)]
    pub fn set_nick(&mut self, nick: String) {
        self.nick = Some(nick);
    }

    /// Peer'ı mute yap/çöz.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }
}

/// Hub'ın tüm peer'larını yöneten struct. Peer ekleme, çıkarma,
/// nick/role değiştirme işlemleri burada.
#[derive(Debug, Default)]
pub struct PeerRegistry {
    peers: Vec<PeerInfo>,
    /// Bir sonraki peer için ID sayacı. Basit ve öngörülebilir —
    /// anonimlik için random da olabilir ama ID sadece görünen addır,
    /// gerçek IP'yi ifşa etmez.
    next_id: u64,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Yeni peer ekle. Atanan ID'yi döndür.
    pub fn add(&mut self, addr: SocketAddr) -> u64 {
        let id = self.next_id + 1;
        self.next_id = id;
        self.peers.push(PeerInfo::new(id, addr));
        id
    }

    /// ID ile peer çıkar. Bulunamazsa false.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.peers.len();
        self.peers.retain(|p| p.id != id);
        self.peers.len() < before
    }

    /// ID ile peer bul (mutable).
    #[allow(dead_code)]
    pub fn get_mut(&mut self, id: u64) -> Option<&mut PeerInfo> {
        self.peers.iter_mut().find(|p| p.id == id)
    }

    /// ID ile peer bul (immutable).
    pub fn get(&self, id: u64) -> Option<&PeerInfo> {
        self.peers.iter().find(|p| p.id == id)
    }

    /// Nick ile peer bul (immutable). İlk eşleşeni döner.
    pub fn find_by_nick(&self, nick: &str) -> Option<&PeerInfo> {
        self.peers
            .iter()
            .find(|p| p.nick.as_deref() == Some(nick))
    }

    /// Nick ile peer bul (mutable).
    pub fn find_by_nick_mut(&mut self, nick: &str) -> Option<&mut PeerInfo> {
        self.peers
            .iter_mut()
            .find(|p| p.nick.as_deref() == Some(nick))
    }

    /// Tüm peer'ları listele (immutable).
    pub fn list(&self) -> &[PeerInfo] {
        &self.peers
    }

    /// Toplam peer sayısı.
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    /// Peer yok mu?
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Bir role sahip kaç peer var?
    #[allow(dead_code)]
    pub fn count_by_role(&self, role: Role) -> usize {
        self.peers.iter().filter(|p| p.role == role).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    // ===== Role tests =====

    #[test]
    fn role_as_str_returns_correct_strings() {
        assert_eq!(Role::Guest.as_str(), "guest");
        assert_eq!(Role::User.as_str(), "user");
        assert_eq!(Role::Moderator.as_str(), "moderator");
        assert_eq!(Role::Admin.as_str(), "admin");
    }

    #[test]
    fn role_from_str_parses_all_variants() {
        assert_eq!(Role::from_str("admin"), Some(Role::Admin));
        assert_eq!(Role::from_str("ADMIN"), Some(Role::Admin));
        assert_eq!(Role::from_str("Admin"), Some(Role::Admin));
        assert_eq!(Role::from_str("moderator"), Some(Role::Moderator));
        assert_eq!(Role::from_str("mod"), Some(Role::Moderator));
        assert_eq!(Role::from_str("user"), Some(Role::User));
        assert_eq!(Role::from_str("guest"), Some(Role::Guest));
    }

    #[test]
    fn role_from_str_rejects_invalid() {
        assert_eq!(Role::from_str("superadmin"), None);
        assert_eq!(Role::from_str(""), None);
        assert_eq!(Role::from_str("owner"), None);
    }

    #[test]
    fn role_display_uses_as_str() {
        assert_eq!(format!("{}", Role::Admin), "admin");
        assert_eq!(format!("{}", Role::User), "user");
    }

    #[test]
    fn role_can_clear_admin_always_true() {
        let perms = PermissionsConfig::default();
        assert!(Role::Admin.can_clear(&perms));
    }

    #[test]
    fn role_can_clear_moderator_true() {
        let perms = PermissionsConfig::default();
        assert!(Role::Moderator.can_clear(&perms));
    }

    #[test]
    fn role_can_clear_user_respects_config() {
        let mut perms = PermissionsConfig::default();
        assert!(!Role::User.can_clear(&perms));
        perms.allow_user_clear = true;
        assert!(Role::User.can_clear(&perms));
    }

    #[test]
    fn role_can_clear_guest_always_false() {
        let perms = PermissionsConfig::default();
        assert!(!Role::Guest.can_clear(&perms));
    }

    #[test]
    fn role_can_kick_matrix() {
        let mut perms = PermissionsConfig::default();
        assert!(Role::Admin.can_kick(&perms));
        assert!(Role::Moderator.can_kick(&perms));
        assert!(!Role::User.can_kick(&perms));
        assert!(!Role::Guest.can_kick(&perms));
        perms.allow_user_kick = true;
        assert!(Role::User.can_kick(&perms));
        assert!(!Role::Guest.can_kick(&perms));
    }

    #[test]
    fn role_can_mute_matrix() {
        let mut perms = PermissionsConfig::default();
        assert!(Role::Admin.can_mute(&perms));
        assert!(Role::Moderator.can_mute(&perms));
        assert!(!Role::User.can_mute(&perms));
        perms.allow_user_mute = true;
        assert!(Role::User.can_mute(&perms));
    }

    #[test]
    fn role_can_change_config_admin_only_by_default() {
        let perms = PermissionsConfig::default();
        assert!(Role::Admin.can_change_config(&perms));
        assert!(!Role::Moderator.can_change_config(&perms));
        assert!(!Role::User.can_change_config(&perms));
        assert!(!Role::Guest.can_change_config(&perms));
    }

    #[test]
    fn role_can_change_config_user_with_permission() {
        let perms = PermissionsConfig {
            allow_user_change_config: true,
            ..Default::default()
        };
        assert!(Role::User.can_change_config(&perms));
        assert!(!Role::Moderator.can_change_config(&perms));
    }

    #[test]
    fn role_can_grant_admin_only_admin() {
        assert!(Role::Admin.can_grant_admin());
        assert!(!Role::Moderator.can_grant_admin());
        assert!(!Role::User.can_grant_admin());
        assert!(!Role::Guest.can_grant_admin());
    }

    #[test]
    fn role_can_revoke_admin_only_admin() {
        assert!(Role::Admin.can_revoke_admin());
        assert!(!Role::Moderator.can_revoke_admin());
    }

    #[test]
    fn role_can_view_config_all_roles() {
        assert!(Role::Admin.can_view_config());
        assert!(Role::Moderator.can_view_config());
        assert!(Role::User.can_view_config());
        assert!(Role::Guest.can_view_config());
    }

    // ===== PeerInfo tests =====

    #[test]
    fn peer_info_new_defaults() {
        let p = PeerInfo::new(42, test_addr(8080));
        assert_eq!(p.id, 42);
        assert_eq!(p.role, Role::User);
        assert!(!p.muted);
        assert!(p.nick.is_none());
    }

    #[test]
    fn peer_info_display_name_with_nick() {
        let mut p = PeerInfo::new(1, test_addr(8080));
        p.set_nick("alice".to_string());
        assert_eq!(p.display_name(false), "alice");
        assert_eq!(p.display_name(true), "alice");
    }

    #[test]
    fn peer_info_display_name_without_nick_anonymous() {
        let p = PeerInfo::new(255, test_addr(8080));
        assert_eq!(p.display_name(false), "peer-ff");
    }

    #[test]
    fn peer_info_display_name_without_nick_show_addr() {
        let p = PeerInfo::new(255, test_addr(8080));
        assert_eq!(p.display_name(true), "127.0.0.1:8080");
    }

    #[test]
    fn peer_info_set_role() {
        let mut p = PeerInfo::new(1, test_addr(8080));
        p.set_role(Role::Admin);
        assert_eq!(p.role, Role::Admin);
    }

    #[test]
    fn peer_info_set_nick() {
        let mut p = PeerInfo::new(1, test_addr(8080));
        p.set_nick("bob".to_string());
        assert_eq!(p.nick, Some("bob".to_string()));
    }

    #[test]
    fn peer_info_set_muted() {
        let mut p = PeerInfo::new(1, test_addr(8080));
        p.set_muted(true);
        assert!(p.muted);
        p.set_muted(false);
        assert!(!p.muted);
    }

    // ===== PeerRegistry tests =====

    #[test]
    fn registry_new_is_empty() {
        let r = PeerRegistry::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn registry_add_assigns_incrementing_ids() {
        let mut r = PeerRegistry::new();
        let id1 = r.add(test_addr(8080));
        let id2 = r.add(test_addr(8081));
        let id3 = r.add(test_addr(8082));
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn registry_get_by_id() {
        let mut r = PeerRegistry::new();
        let id = r.add(test_addr(8080));
        let p = r.get(id).unwrap();
        assert_eq!(p.id, id);
        assert_eq!(p.addr, test_addr(8080));
    }

    #[test]
    fn registry_get_nonexistent_returns_none() {
        let mut r = PeerRegistry::new();
        r.add(test_addr(8080));
        assert!(r.get(999).is_none());
    }

    #[test]
    fn registry_remove_existing() {
        let mut r = PeerRegistry::new();
        let id = r.add(test_addr(8080));
        assert!(r.remove(id));
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn registry_remove_nonexistent_returns_false() {
        let mut r = PeerRegistry::new();
        assert!(!r.remove(999));
    }

    #[test]
    fn registry_find_by_nick() {
        let mut r = PeerRegistry::new();
        let id = r.add(test_addr(8080));
        r.get_mut(id).unwrap().set_nick("alice".to_string());

        let found = r.find_by_nick("alice").unwrap();
        assert_eq!(found.id, id);
    }

    #[test]
    fn registry_find_by_nick_not_found() {
        let mut r = PeerRegistry::new();
        r.add(test_addr(8080));
        assert!(r.find_by_nick("nonexistent").is_none());
    }

    #[test]
    fn registry_find_by_nick_mut_allows_role_change() {
        let mut r = PeerRegistry::new();
        let id = r.add(test_addr(8080));
        r.get_mut(id).unwrap().set_nick("bob".to_string());

        let found = r.find_by_nick_mut("bob").unwrap();
        found.set_role(Role::Admin);

        assert_eq!(r.get(id).unwrap().role, Role::Admin);
    }

    #[test]
    fn registry_list_returns_all() {
        let mut r = PeerRegistry::new();
        r.add(test_addr(8080));
        r.add(test_addr(8081));
        r.add(test_addr(8082));
        assert_eq!(r.list().len(), 3);
    }

    #[test]
    fn registry_count_by_role() {
        let mut r = PeerRegistry::new();
        let id1 = r.add(test_addr(8080));
        let id2 = r.add(test_addr(8081));
        r.add(test_addr(8082));
        r.get_mut(id1).unwrap().set_role(Role::Admin);
        r.get_mut(id2).unwrap().set_role(Role::Moderator);

        assert_eq!(r.count_by_role(Role::Admin), 1);
        assert_eq!(r.count_by_role(Role::Moderator), 1);
        assert_eq!(r.count_by_role(Role::User), 1);
        assert_eq!(r.count_by_role(Role::Guest), 0);
    }

    #[test]
    fn registry_remove_does_not_reuse_ids() {
        let mut r = PeerRegistry::new();
        let _id1 = r.add(test_addr(8080));
        let id2 = r.add(test_addr(8081));
        r.remove(id2);
        let id3 = r.add(test_addr(8082));
        // id3, id2'nin yerine geçmez — yeni bir ID alır
        assert_eq!(id3, 3);
        assert_eq!(r.len(), 2);
    }
}
