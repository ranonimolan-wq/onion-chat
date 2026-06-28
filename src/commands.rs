// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Komut modülü — TUI slash komutlarını parse eder ve çalıştırır.
//!
//! Bu modül kullanıcının girdiği `/komut argümanlar` formatındaki
//! metni parse eder. Komutun çalıştırılması (dosya gönderme, çıkma,
//! yardım gösterme) `ui.rs`'ın sorumluluğundadır; bu modül sadece
//! parse eder ve `Command` enum'u döndürür.
//!
//! ## Desteklenen komutlar
//!
//! - `/help` — kullanılabilir komutları listele
//! - `/quit` veya `/exit` — sohbetten çık
//! - `/clear` — sohbet geçmişini temizle (ekran + disk)
//! - `/send <path>` — bir dosya gönder
//! - `/nick <name>` — takma ad ayarla (ileride)
//! - `/who` — bağlı peer'ları listele (multi-peer modunda)

use std::path::PathBuf;

/// Bir slash komutunu temsil eder. Parse sonucu döndürülür.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `/help` — yardım metni göster.
    Help,
    /// `/quit` veya `/exit` — sohbetten çık.
    Quit,
    /// `/clear` — sohbet geçmişinin tamamını temizle.
    Clear,
    /// `/clear <N>` — son N mesajı sil.
    ClearCount(usize),
    /// `/send <path>` — dosya gönder. Argüman dosya yolu.
    Send(PathBuf),
    /// `/nick <name>` — takma ad ayarla.
    Nick(String),
    /// `/who` — bağlı peer'ları listele.
    Who,
    /// `/on_admin <nick>` — bir peer'a admin rolü ver (admin only).
    OnAdmin(String),
    /// `/off_admin <nick>` — bir peer'ın admin rolünü al (admin only).
    OffAdmin(String),
    /// `/kick <nick>` — bir peer'ı at (admin/mod only).
    Kick(String),
    /// `/mute <nick>` — bir peer'ı sustur (admin/mod only).
    Mute(String),
    /// `/unmute <nick>` — susturmayı kaldır (admin/mod only).
    Unmute(String),
    /// `/role` — kendi rolünü göster.
    Role,
    /// `/role <nick>` — başka bir peer'ın rolünü göster.
    RoleOf(String),
    /// `/config` — config'i göster.
    Config,
    /// `/config set <key> <value>` — config alanını değiştir (admin only).
    ConfigSet(String, String),
    /// Bilinmeyen komut. Orijinal metin korunur.
    Unknown(String),
}

/// Boş olmayan argümanları ayırır. Birden çok boşluğu tek boşluk gibi
/// ele alır.
fn split_args(input: &str) -> Vec<&str> {
    input.split_whitespace().collect()
}

/// Bir girdi satırını parse eder. Komut değilse `None` döner.
///
/// Komut olma kriteri: girdi `/` ile başlar. Aksi halde normal mesajdır.
///
/// # Örnekler
///
/// ```
/// use onionchat::commands::{parse, Command};
/// assert_eq!(parse("/help"), Some(Command::Help));
/// assert_eq!(parse("/quit"), Some(Command::Quit));
/// assert_eq!(parse("merhaba"), None);
/// ```
pub fn parse(input: &str) -> Option<Command> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    // `/` sonrası ilk kelime komut adı.
    let without_slash = &trimmed[1..];
    let parts: Vec<&str> = split_args(without_slash);
    if parts.is_empty() {
        // Sadece "/" — bilinmeyen.
        return Some(Command::Unknown(trimmed.to_string()));
    }
    let cmd = parts[0].to_lowercase();
    let args = &parts[1..];
    let command = match cmd.as_str() {
        "help" | "h" | "?" => Command::Help,
        "quit" | "exit" | "q" => Command::Quit,
        "clear" | "cls" => {
            if args.is_empty() {
                Command::Clear
            } else {
                match args[0].parse::<usize>() {
                    Ok(n) => Command::ClearCount(n),
                    Err(_) => Command::Unknown(format!(
                        "/clear: geçersiz sayı '{}'",
                        args[0]
                    )),
                }
            }
        }
        "send" | "file" => {
            if args.is_empty() {
                Command::Unknown("/send: dosya yolu gerekli".to_string())
            } else {
                Command::Send(PathBuf::from(args[0]))
            }
        }
        "nick" | "nickname" | "name" => {
            if args.is_empty() {
                Command::Unknown("/nick: isim gerekli".to_string())
            } else {
                Command::Nick(args.join(" "))
            }
        }
        "who" | "list" => Command::Who,
        "on_admin" | "onadmin" | "op" => {
            if args.is_empty() {
                Command::Unknown("/on_admin: nick gerekli".to_string())
            } else {
                Command::OnAdmin(args[0].to_string())
            }
        }
        "off_admin" | "offadmin" | "deop" => {
            if args.is_empty() {
                Command::Unknown("/off_admin: nick gerekli".to_string())
            } else {
                Command::OffAdmin(args[0].to_string())
            }
        }
        "kick" => {
            if args.is_empty() {
                Command::Unknown("/kick: nick gerekli".to_string())
            } else {
                Command::Kick(args[0].to_string())
            }
        }
        "mute" => {
            if args.is_empty() {
                Command::Unknown("/mute: nick gerekli".to_string())
            } else {
                Command::Mute(args[0].to_string())
            }
        }
        "unmute" => {
            if args.is_empty() {
                Command::Unknown("/unmute: nick gerekli".to_string())
            } else {
                Command::Unmute(args[0].to_string())
            }
        }
        "role" => {
            if args.is_empty() {
                Command::Role
            } else {
                Command::RoleOf(args[0].to_string())
            }
        }
        "config" | "cfg" => {
            if args.is_empty() {
                Command::Config
            } else if args[0] == "set" && args.len() >= 3 {
                Command::ConfigSet(args[1].to_string(), args[2..].join(" "))
            } else {
                Command::Unknown("/config kullanımı: /config veya /config set <key> <value>".to_string())
            }
        }
        _ => Command::Unknown(format!("/{}: bilinmeyen komut", cmd)),
    };
    Some(command)
}

/// `/help` komutu için yardım metni döndürür. TUI'da gösterilir.
pub fn help_text() -> &'static str {
    "OnionChat komutları:\n\
     \n\
     Temel:\n\
     /help                    Bu yardım mesajını göster\n\
     /quit                    Sohbetten çık (Esc de çalışır)\n\
     /clear [N]               Sohbeti temizle (N=son N mesajı sil)\n\
     /nick <name>             Takma ad ayarla\n\
     /who                     Bağlı peer'ları listele\n\
     /role [nick]             Rol göster (kendinizinkini veya başkasınınkini)\n\
     /send <path>             Bir dosya gönder (E2EE)\n\
     \n\
     Yönetim (admin/mod):\n\
     /on_admin <nick>         Bir peer'a admin ver (admin only)\n\
     /off_admin <nick>        Admin rolünü al (admin only)\n\
     /kick <nick>             Bir peer'ı at (admin/mod)\n\
     /mute <nick>             Bir peer'ı sustur (admin/mod)\n\
     /unmute <nick>           Susturmayı kaldır (admin/mod)\n\
     /config                  Config'i göster\n\
     /config set <key> <val>  Config alanını değiştir (admin only)\n\
     \n\
     Mesaj göndermek için yazıp Enter'a basın.\n\
     Markdown: *bold* _italic_ `code` :emoji:\n\
     Anonimlik: peer'lar IP yerine peer-<id> ile gösterilir."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_command_returns_none() {
        assert_eq!(parse("merhaba"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("   "), None);
        assert_eq!(parse("normal mesaj"), None);
    }

    #[test]
    fn help_command_parses() {
        assert_eq!(parse("/help"), Some(Command::Help));
        assert_eq!(parse("/h"), Some(Command::Help));
        assert_eq!(parse("/?"), Some(Command::Help));
        assert_eq!(parse("/HELP"), Some(Command::Help));
        assert_eq!(parse("/Help"), Some(Command::Help));
    }

    #[test]
    fn quit_command_parses() {
        assert_eq!(parse("/quit"), Some(Command::Quit));
        assert_eq!(parse("/exit"), Some(Command::Quit));
        assert_eq!(parse("/q"), Some(Command::Quit));
        assert_eq!(parse("/QUIT"), Some(Command::Quit));
    }

    #[test]
    fn clear_command_parses() {
        assert_eq!(parse("/clear"), Some(Command::Clear));
        assert_eq!(parse("/cls"), Some(Command::Clear));
    }

    #[test]
    fn send_command_with_path() {
        let result = parse("/send /tmp/foo.txt");
        assert_eq!(result, Some(Command::Send(PathBuf::from("/tmp/foo.txt"))));
    }

    #[test]
    fn send_command_with_relative_path() {
        let result = parse("/send foo.txt");
        assert_eq!(result, Some(Command::Send(PathBuf::from("foo.txt"))));
    }

    #[test]
    fn send_command_without_path_returns_unknown() {
        let result = parse("/send");
        assert!(matches!(result, Some(Command::Unknown(_))));
    }

    #[test]
    fn send_command_alias_file() {
        let result = parse("/file bar.bin");
        assert_eq!(result, Some(Command::Send(PathBuf::from("bar.bin"))));
    }

    #[test]
    fn nick_command_single_word() {
        let result = parse("/nick alice");
        assert_eq!(result, Some(Command::Nick("alice".to_string())));
    }

    #[test]
    fn nick_command_multi_word_joins() {
        let result = parse("/nick John Doe");
        assert_eq!(result, Some(Command::Nick("John Doe".to_string())));
    }

    #[test]
    fn nick_command_without_arg_returns_unknown() {
        let result = parse("/nick");
        assert!(matches!(result, Some(Command::Unknown(_))));
    }

    #[test]
    fn who_command_parses() {
        assert_eq!(parse("/who"), Some(Command::Who));
        assert_eq!(parse("/list"), Some(Command::Who));
    }

    #[test]
    fn unknown_command_returns_unknown() {
        let result = parse("/xyz");
        assert!(matches!(result, Some(Command::Unknown(_))));
        if let Some(Command::Unknown(s)) = result {
            assert!(s.contains("bilinmeyen"));
        }
    }

    #[test]
    fn just_slash_returns_unknown() {
        let result = parse("/");
        assert!(matches!(result, Some(Command::Unknown(_))));
    }

    #[test]
    fn command_with_leading_whitespace_parses() {
        assert_eq!(parse("   /help"), Some(Command::Help));
    }

    #[test]
    fn command_with_trailing_whitespace_parses() {
        assert_eq!(parse("/help   "), Some(Command::Help));
    }

    #[test]
    fn send_with_multiple_args_uses_first() {
        // `/send foo.txt extra` → sadece foo.txt
        let result = parse("/send foo.txt extra");
        assert_eq!(result, Some(Command::Send(PathBuf::from("foo.txt"))));
    }

    #[test]
    fn help_text_contains_all_commands() {
        let text = help_text();
        assert!(text.contains("/help"));
        assert!(text.contains("/quit"));
        assert!(text.contains("/clear"));
        assert!(text.contains("/send"));
        assert!(text.contains("/nick"));
        assert!(text.contains("/who"));
    }

    #[test]
    fn help_text_mentions_markdown() {
        let text = help_text();
        assert!(text.contains("Markdown"));
        assert!(text.contains("*bold*"));
    }

    #[test]
    fn case_insensitive_command() {
        assert_eq!(parse("/HELP"), Some(Command::Help));
        assert_eq!(parse("/Quit"), Some(Command::Quit));
        assert_eq!(parse("/CLEAR"), Some(Command::Clear));
    }

    #[test]
    fn nick_command_turkish_single_word() {
        // Türkçe nick: "Şükrü"
        let result = parse("/nick Şükrü");
        assert_eq!(result, Some(Command::Nick("Şükrü".to_string())));
    }

    #[test]
    fn nick_command_turkish_full_name() {
        // Türkçe tam isim: "Mehmet Şahin"
        let result = parse("/nick Mehmet Şahin");
        assert_eq!(result, Some(Command::Nick("Mehmet Şahin".to_string())));
    }

    #[test]
    fn nick_command_all_turkish_chars() {
        // Tüm Türkçe karakterleri içeren nick
        let result = parse("/nick ŞşĞğÜüÖöÇçİı");
        assert_eq!(result, Some(Command::Nick("ŞşĞğÜüÖöÇçİı".to_string())));
    }

    #[test]
    fn nick_command_istanbul() {
        let result = parse("/nick İstanbul");
        assert_eq!(result, Some(Command::Nick("İstanbul".to_string())));
    }

    #[test]
    fn send_command_turkish_filename() {
        // Türkçe dosya adı (path)
        let result = parse("/send /tmp/Şiirler.txt");
        assert_eq!(result, Some(Command::Send(PathBuf::from("/tmp/Şiirler.txt"))));
    }

    #[test]
    fn send_command_turkish_relative_path() {
        let result = parse("/send İstchar.txt");
        assert_eq!(result, Some(Command::Send(PathBuf::from("İstchar.txt"))));
    }

    #[test]
    fn unknown_command_with_turkish_args() {
        // Türkçe argümanlı bilinmeyen komut
        let result = parse("/bilinmiyor İstanbul");
        assert!(matches!(result, Some(Command::Unknown(_))));
        if let Some(Command::Unknown(s)) = result {
            assert!(s.contains("bilinmeyen"));
        }
    }

    #[test]
    fn non_command_turkish_text_returns_none() {
        // Türkçe mesaj — komut değil
        assert_eq!(parse("Merhaba dünya"), None);
        assert_eq!(parse("Nasılsın?"), None);
        assert_eq!(parse("İyi, teşekkürler"), None);
    }

    #[test]
    fn help_text_contains_turkish_friendly_info() {
        // Help text Türkçe yazılmış — kontrol et
        let text = help_text();
        assert!(text.contains("komut"));
        assert!(text.contains("Mesaj göndermek"));
    }

    // ===== Yeni komut testleri (v0.3) =====

    #[test]
    fn clear_no_args_parses_as_clear() {
        assert_eq!(parse("/clear"), Some(Command::Clear));
        assert_eq!(parse("/cls"), Some(Command::Clear));
    }

    #[test]
    fn clear_with_count_parses_as_clearcount() {
        assert_eq!(parse("/clear 10"), Some(Command::ClearCount(10)));
        assert_eq!(parse("/clear 1"), Some(Command::ClearCount(1)));
        assert_eq!(parse("/clear 1000"), Some(Command::ClearCount(1000)));
    }

    #[test]
    fn clear_with_invalid_count_returns_unknown() {
        assert!(matches!(parse("/clear abc"), Some(Command::Unknown(_))));
        assert!(matches!(parse("/clear -1"), Some(Command::Unknown(_))));
        assert!(matches!(parse("/clear 1.5"), Some(Command::Unknown(_))));
    }

    #[test]
    fn clear_with_zero_count_parses() {
        // 0 geçerli bir sayı — TUI'da "0 mesaj silindi" olarak değerlendirilir
        assert_eq!(parse("/clear 0"), Some(Command::ClearCount(0)));
    }

    #[test]
    fn on_admin_parses() {
        assert_eq!(parse("/on_admin alice"), Some(Command::OnAdmin("alice".to_string())));
        assert_eq!(parse("/onadmin bob"), Some(Command::OnAdmin("bob".to_string())));
        assert_eq!(parse("/op charlie"), Some(Command::OnAdmin("charlie".to_string())));
    }

    #[test]
    fn on_admin_without_arg_returns_unknown() {
        assert!(matches!(parse("/on_admin"), Some(Command::Unknown(_))));
        assert!(matches!(parse("/onadmin"), Some(Command::Unknown(_))));
    }

    #[test]
    fn off_admin_parses() {
        assert_eq!(parse("/off_admin alice"), Some(Command::OffAdmin("alice".to_string())));
        assert_eq!(parse("/offadmin bob"), Some(Command::OffAdmin("bob".to_string())));
        assert_eq!(parse("/deop charlie"), Some(Command::OffAdmin("charlie".to_string())));
    }

    #[test]
    fn off_admin_without_arg_returns_unknown() {
        assert!(matches!(parse("/off_admin"), Some(Command::Unknown(_))));
    }

    #[test]
    fn kick_parses() {
        assert_eq!(parse("/kick alice"), Some(Command::Kick("alice".to_string())));
    }

    #[test]
    fn kick_without_arg_returns_unknown() {
        assert!(matches!(parse("/kick"), Some(Command::Unknown(_))));
    }

    #[test]
    fn mute_parses() {
        assert_eq!(parse("/mute alice"), Some(Command::Mute("alice".to_string())));
    }

    #[test]
    fn mute_without_arg_returns_unknown() {
        assert!(matches!(parse("/mute"), Some(Command::Unknown(_))));
    }

    #[test]
    fn unmute_parses() {
        assert_eq!(parse("/unmute alice"), Some(Command::Unmute("alice".to_string())));
    }

    #[test]
    fn unmute_without_arg_returns_unknown() {
        assert!(matches!(parse("/unmute"), Some(Command::Unknown(_))));
    }

    #[test]
    fn role_no_args_parses_as_role() {
        assert_eq!(parse("/role"), Some(Command::Role));
    }

    #[test]
    fn role_with_nick_parses_as_roleof() {
        assert_eq!(parse("/role alice"), Some(Command::RoleOf("alice".to_string())));
    }

    #[test]
    fn config_no_args_parses_as_config() {
        assert_eq!(parse("/config"), Some(Command::Config));
        assert_eq!(parse("/cfg"), Some(Command::Config));
    }

    #[test]
    fn config_set_with_two_args_returns_unknown() {
        // /config set key  — value eksik
        assert!(matches!(parse("/config set key"), Some(Command::Unknown(_))));
    }

    #[test]
    fn config_set_with_three_args_parses() {
        let result = parse("/config set history.enabled true");
        assert_eq!(
            result,
            Some(Command::ConfigSet("history.enabled".to_string(), "true".to_string()))
        );
    }

    #[test]
    fn config_set_with_multi_word_value() {
        // /config set server.name My Hub
        let result = parse("/config set server.name My Hub");
        assert_eq!(
            result,
            Some(Command::ConfigSet("server.name".to_string(), "My Hub".to_string()))
        );
    }

    #[test]
    fn config_set_with_turkish_value() {
        let result = parse("/config set server.name Türkçe Hub");
        assert_eq!(
            result,
            Some(Command::ConfigSet("server.name".to_string(), "Türkçe Hub".to_string()))
        );
    }

    #[test]
    fn config_invalid_subcommand_returns_unknown() {
        assert!(matches!(parse("/config foo"), Some(Command::Unknown(_))));
        assert!(matches!(parse("/config set"), Some(Command::Unknown(_))));
    }

    #[test]
    fn help_text_contains_new_commands() {
        let text = help_text();
        assert!(text.contains("/clear [N]"));
        assert!(text.contains("/on_admin"));
        assert!(text.contains("/off_admin"));
        assert!(text.contains("/kick"));
        assert!(text.contains("/mute"));
        assert!(text.contains("/unmute"));
        assert!(text.contains("/role"));
        assert!(text.contains("/config"));
        assert!(text.contains("Anonimlik"));
    }
}
