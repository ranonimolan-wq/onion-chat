// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 OnionChat Developers. All rights reserved.

//! Terminal kullanıcı arayüzü (TUI) — `ratatui` + `crossterm` tabanlı sohbet.
//!
//! Bu modül sohbet TUI'sini çizer ve yönetir. Klavye girdilerini işler,
//! ağdan gelen mesajları gösterir, kullanıcının yazdığı mesajları peer'a
//! gönderir.
//!
//! ## Layout
//!
//! ```text
//! ┌─ OnionChat v0.1 — Tor P2P E2EE Chat ────────────┐
//! ├─ Sohbet ────────────────────────────────────────┤
//! │ [12:34] < merhaba                                │
//! │ [12:34] > selam nasılsın                         │
//! │ [12:35] < iyiyim, teşekkürler                    │
//! │                                                  │
//! ├─ Mesaj > _____________________________________ │
//! │ [listen] peer=127.0.0.1:42458 | bağlı           │
//! └──────────────────────────────────────────────────┘
//! ```
//!
//! ## Klavye kısayolları
//!
//! - `Enter` — mesajı gönder
//! - `Esc` veya `Ctrl+C` — çık
//! - `↑` / `↓` — sohbet geçmişinde kaydır
//! - `←` / `→` — imleci hareket ettir
//! - `Backspace` — imleçten önceki karakteri sil
//! - `Home` / `End` — imleci satır başına / sonuna

use anyhow::Result;
use clap::ValueEnum;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::execute;
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Span, Spans};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Terminal;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

use crate::chat;
use crate::network::{Connection, Sender};

/// TUI başlatma modu. Listen tarafı mı connect tarafı mı olduğunu belirtir.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum UiMode {
    Listen,
    Connect,
}

/// Bir sohbet satırı. Gönderilen, alınan veya sistem mesajı olabilir.
#[derive(Debug, Clone)]
pub struct ChatLine {
    pub kind: ChatLineKind,
    pub text: String,
    pub timestamp: SystemTime,
}

/// Sohbet satırı türü. Renk ve önek buna göre seçilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatLineKind {
    /// Kullanıcı tarafından gönderilen mesaj. Yeşil, `>` öneki.
    Sent,
    /// Peer'dan alınan mesaj. Cyan, `<` öneki.
    Received,
    /// Sistem mesajı (bağlantı kuruldu, koptu, vb.). Sarı, italik, `*` öneki.
    System,
}

impl ChatLine {
    /// Yeni gönderilen mesaj.
    pub fn sent(text: impl Into<String>) -> Self {
        Self {
            kind: ChatLineKind::Sent,
            text: text.into(),
            timestamp: SystemTime::now(),
        }
    }

    /// Yeni alınan mesaj.
    pub fn received(text: impl Into<String>) -> Self {
        Self {
            kind: ChatLineKind::Received,
            text: text.into(),
            timestamp: SystemTime::now(),
        }
    }

    /// Yeni sistem mesajı.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            kind: ChatLineKind::System,
            text: text.into(),
            timestamp: SystemTime::now(),
        }
    }
}

/// TUI uygulaması durumu. Tüm state burada toplanır; çizim ve klavye
/// handler'ları bu struct'ı okur/yazar.
pub struct ChatState {
    /// Bağlantı modu (listen veya connect).
    pub mode: UiMode,
    /// Peer adresi (status bar'da gösterilir).
    pub peer_addr: SocketAddr,
    /// Sohbet geçmişi. Yeni mesajlar sona eklenir.
    pub messages: Vec<ChatLine>,
    /// Kullanıcının yazdığı metin.
    pub input: String,
    /// İmleç pozisyonu (bayt offset, UTF-8 çok baytlı karakterlerde
    /// karakter sınırında olmalı).
    pub cursor_pos: usize,
    /// Kaç satır yukarı kaydırıldı. 0 = en altta (otomatik takip).
    pub scroll_offset: usize,
    /// Bağlantı aktif mi?
    pub connected: bool,
    /// Çıkış bayrağı. True olduğunda event loop biter.
    pub quit: bool,
}

impl ChatState {
    /// Yeni state oluştur. Bağlantı kurulduktan sonra çağrılır.
    pub fn new(mode: UiMode, peer_addr: SocketAddr) -> Self {
        Self {
            mode,
            peer_addr,
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            connected: true,
            quit: false,
        }
    }

    /// Gönderilen mesaj ekle ve otomatik en alta kaydır.
    pub fn add_sent(&mut self, text: impl Into<String>) {
        self.messages.push(ChatLine::sent(text));
        self.scroll_offset = 0;
    }

    /// Alınan mesaj ekle ve otomatik en alta kaydır.
    pub fn add_received(&mut self, text: impl Into<String>) {
        self.messages.push(ChatLine::received(text));
        self.scroll_offset = 0;
    }

    /// Sistem mesajı ekle (bağlantı durumu, hatalar, vb.).
    pub fn add_system(&mut self, text: impl Into<String>) {
        self.messages.push(ChatLine::system(text));
        self.scroll_offset = 0;
    }

    /// Bir satır yukarı kaydır.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Bir satır aşağı kaydır. 0'ın altına inmez.
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// İmleç pozisyonuna bir karakter ekle.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// İmleçten önceki karakteri sil. İmleç 0 ise hiçbir şey yapma.
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos].chars().last().unwrap();
            let prev_len = prev.len_utf8();
            self.cursor_pos -= prev_len;
            self.input
                .replace_range(self.cursor_pos..self.cursor_pos + prev_len, "");
        }
    }

    /// İmleci bir karakter sola taşı. 0 ise durur.
    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos].chars().last().unwrap();
            self.cursor_pos -= prev.len_utf8();
        }
    }

    /// İmleci bir karakter sağa taşı. Sonda ise durur.
    pub fn move_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            let next = self.input[self.cursor_pos..].chars().next().unwrap();
            self.cursor_pos += next.len_utf8();
        }
    }

    /// İmleci satır başına.
    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// İmleci satır sonuna.
    pub fn move_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    /// Input'u al ve temizle. Gönderim için kullanılır.
    pub fn take_input(&mut self) -> String {
        let s = std::mem::take(&mut self.input);
        self.cursor_pos = 0;
        s
    }
}

/// Zaman damgasını `HH:MM` formatına çevirir (UTC).
fn format_timestamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    format!("{:02}:{:02}", hours, mins)
}

/// Bir `ChatLine`'ı renkli `Spans`'a çevirir.
fn line_to_spans(line: &ChatLine) -> Spans<'_> {
    use crate::markdown;
    let ts = format_timestamp(line.timestamp);
    let ts_span = Span::styled(format!("[{}] ", ts), Style::default().fg(Color::DarkGray));
    match line.kind {
        ChatLineKind::Sent => {
            let mut spans: Vec<Span<'_>> = vec![
                ts_span,
                Span::styled(
                    "> ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            spans.extend(markdown::render_spans(&line.text));
            Spans::from(spans)
        }
        ChatLineKind::Received => {
            let mut spans: Vec<Span<'_>> = vec![
                ts_span,
                Span::styled(
                    "< ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            spans.extend(markdown::render_spans(&line.text));
            Spans::from(spans)
        }
        ChatLineKind::System => Spans::from(vec![
            ts_span,
            Span::styled(
                format!("* {}", line.text),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]),
    }
}

/// Veriin yüksekliğine göre görünür chat satırlarını döndürür.
/// `scroll_offset` 0 ise en yeni mesajlar altta görünür.
fn visible_lines<'a>(state: &'a ChatState, height: usize) -> Vec<Spans<'a>> {
    if height == 0 || state.messages.is_empty() {
        return vec![Spans::from(Span::raw(""))];
    }
    let total = state.messages.len();
    let scroll = state.scroll_offset.min(total.saturating_sub(1));
    let bottom_idx = total - 1 - scroll;
    let top_idx = bottom_idx.saturating_sub(height.saturating_sub(1));
    (top_idx..=bottom_idx)
        .map(|i| line_to_spans(&state.messages[i]))
        .collect()
}

/// Eski `run` fonksiyonu — geriye dönük uyumluluk için korundu.
/// Artık `main.rs` tarafından doğrudan çağrılmıyor; chat TUI'si için
/// `run_chat_tui` kullanın.
#[allow(dead_code)]
pub async fn run(
    _mode: UiMode,
    _listen: Option<String>,
    _connect: Option<String>,
    _file: Option<PathBuf>,
    _anon: bool,
) -> Result<()> {
    eprintln!("OnionChat TUI modu: argüman gerekir.");
    eprintln!("  onionchat --listen 8080         # dinleyici");
    eprintln!("  onionchat --connect 1.2.3.4:8080 # sohbet için bağlan");
    Ok(())
}

/// Sohbet TUI'sini çalıştırır. Verilen `Connection`'ı split eder,
/// ağ okuma ve klavye okuma görevlerini başlatır, ana event loop'u
/// yürütür. Kullanıcı Esc'ye basana veya bağlantı kopana kadar döner.
///
/// `initial_message` verilirse, TUI açıldığında ilk alınan mesaj olarak
/// geçmişe eklenir. Bu, listen tarafının peek ettiği ilk mesajı
/// geçirmek için kullanılır.
pub async fn run_chat_tui(
    conn: Connection,
    mode: UiMode,
    peer_addr: SocketAddr,
    initial_message: Option<String>,
) -> Result<()> {
    // Split connection into sender (write half) and receiver (read half).
    let (sender, mut receiver) = conn.split();

    // Network reader task: reads from receiver stream, sends to channel.
    // None signals stream end (peer disconnected).
    let (net_tx, mut net_rx) = mpsc::channel::<Option<Vec<u8>>>(32);
    tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            if net_tx.send(Some(msg)).await.is_err() {
                break;
            }
        }
        // Stream ended — peer disconnected
        let _ = net_tx.send(None).await;
    });

    // Keyboard reader task: blocking poll on crossterm, sends KeyEvents.
    // spawn_blocking kullanıyoruz çünkü event::poll blocking.
    let (kbd_tx, mut kbd_rx) = mpsc::channel::<KeyEvent>(32);
    tokio::task::spawn_blocking(move || {
        loop {
            if let Ok(true) = crossterm::event::poll(Duration::from_millis(100))
                && let Ok(Event::Key(key)) = crossterm::event::read()
                && kbd_tx.blocking_send(key).is_err()
            {
                break;
            }
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // State
    let mut state = ChatState::new(mode, peer_addr);
    if let Some(text) = initial_message {
        state.add_received(text);
    }
    state.add_system("Bağlantı kuruldu. Mesaj yazıp Enter'a basın. Esc ile çıkın.");

    // Main event loop
    let result = chat_event_loop(&mut terminal, &mut state, &sender, &mut net_rx, &mut kbd_rx)
        .await;

    // Restore terminal (hata olsa bile)
    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

/// Hub (multi-peer) sohbet TUI'sini çalıştırır. `network::listen`
/// akışından gelen `(Sender, Receiver)` çiftlerini dinamik olarak kabul
/// eder. Her peer'dan gelen mesajlar tek ekranda gösterilir; kullanıcının
/// yazdığı mesaj tüm peer'lara broadcast edilir (star topoloji).
///
/// Hub operator (sunucuyu başlatan) her zaman admin rolüne sahiptir.
/// Peer'lar `PeerRegistry` ile takip edilir — anonimlik için gerçek IP
/// yerine `peer-<id>` kullanılır.
///
/// `config` tüm ayarları içerir (history, roles, anonymity, permissions).
/// `config_path` config'in kaydedileceği dosya yolu (`/config set` için).
pub async fn run_chat_tui_hub(
    listen_addr: SocketAddr,
    config: crate::config::Config,
    config_path: std::path::PathBuf,
) -> Result<()> {
    use crate::network;
    use crate::roles;
    use futures::StreamExt;

    // Hub state.
    let mut state = ChatState::new(UiMode::Listen, listen_addr);
    state.add_system(format!("Hub modu başlatıldı: {}", config.server.name));
    state.add_system(format!("Dinleme: {}", listen_addr));
    if config.anonymity.strip_metadata {
        state.add_system("Anonimlik: peer adresleri gizli (peer-<id> ile gösterilir)");
    }
    state.add_system("Sen admin'sin (hub operator). /help ile komutları gör.");

    // Peer registry — ID + nick + role tracking.
    let registry: std::sync::Arc<tokio::sync::Mutex<roles::PeerRegistry>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(roles::PeerRegistry::new()));

    // Peer ID → Sender mapping (broadcast için).
    let senders: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<u64, Sender>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));

    // Config (runtime'da değiştirilebilir).
    let config_arc: std::sync::Arc<tokio::sync::Mutex<crate::config::Config>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(config));

    // Event channel.
    let (net_tx, mut net_rx) = mpsc::channel::<HubEvent>(64);

    // Listener task.
    let listen_addr_clone = listen_addr;
    let net_tx_clone = net_tx.clone();
    let registry_clone = registry.clone();
    let senders_clone = senders.clone();
    let config_for_listener = config_arc.clone();
    tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(listen_addr_clone).await {
            Ok(l) => l,
            Err(e) => {
                let _ = net_tx_clone.send(HubEvent::ListenerError(e.to_string())).await;
                return;
            }
        };
        let _ = net_tx_clone.send(HubEvent::ListenerReady).await;
        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    // Max peers kontrolü.
                    let cfg = config_for_listener.lock().await;
                    let max = cfg.server.max_peers;
                    drop(cfg);
                    let current = senders_clone.lock().await.len();
                    if current >= max {
                        tracing::warn!("max_peers={} aşıldı, bağlantı reddedildi", max);
                        drop(stream);
                        continue;
                    }

                    // ECDHE el sıkışması.
                    let conn = match network::Connection::from_stream(stream).await {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!("ECDHE failed for {}: {}", peer_addr, e);
                            continue;
                        }
                    };
                    let (sender, mut receiver) = conn.split();

                    // Peer registry'e ekle, ID al.
                    let peer_id = registry_clone.lock().await.add(peer_addr);
                    senders_clone.lock().await.insert(peer_id, sender);

                    let net_tx2 = net_tx_clone.clone();
                    tokio::spawn(async move {
                        let _ = net_tx2.send(HubEvent::PeerConnected(peer_id, peer_addr)).await;
                        while let Some(msg) = receiver.next().await {
                            let _ = net_tx2.send(HubEvent::PeerMessage(peer_id, msg)).await;
                        }
                        let _ = net_tx2.send(HubEvent::PeerDisconnected(peer_id)).await;
                    });
                }
                Err(e) => {
                    tracing::warn!("accept failed: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    });

    // Keyboard reader task.
    let (kbd_tx, mut kbd_rx) = mpsc::channel::<KeyEvent>(32);
    tokio::task::spawn_blocking(move || {
        loop {
            if let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(100))
                && let Ok(Event::Key(key)) = crossterm::event::read()
                && kbd_tx.blocking_send(key).is_err()
            {
                break;
            }
        }
    });

    // Terminal setup.
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = hub_event_loop(
        &mut terminal,
        &mut state,
        &mut net_rx,
        &mut kbd_rx,
        &registry,
        &senders,
        &config_arc,
        &config_path,
    )
    .await;

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

/// Hub olayları — peer ID bazlı (adres değil, anonimlik).
enum HubEvent {
    /// Listener hazır, dinleme başladı.
    ListenerReady,
    /// Listener başlatılamadı.
    ListenerError(String),
    /// Yeni peer bağlandı. (peer_id, addr)
    PeerConnected(u64, std::net::SocketAddr),
    /// Peer'dan mesaj geldi. (peer_id, msg)
    PeerMessage(u64, Vec<u8>),
    /// Peer bağlantısı koptu. (peer_id)
    PeerDisconnected(u64),
}

/// Hub ana event loop'u. Config-driven, rol-tabanlı komut işler.
#[allow(clippy::too_many_arguments)]
async fn hub_event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state: &mut ChatState,
    net_rx: &mut mpsc::Receiver<HubEvent>,
    kbd_rx: &mut mpsc::Receiver<KeyEvent>,
    registry: &std::sync::Arc<tokio::sync::Mutex<crate::roles::PeerRegistry>>,
    senders: &std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<u64, Sender>>>,
    config: &std::sync::Arc<tokio::sync::Mutex<crate::config::Config>>,
    config_path: &std::path::Path,
) -> Result<()> {
    use crate::chat;
    use crate::commands;
    use crate::config as config_mod;
    use crate::history;
    use crate::roles;

    // History config-driven. Eğer history kapalıysa, dosya açma/yükleme yok.
    let history_path;
    let mut history_file: Option<tokio::fs::File>;
    {
        let cfg = config.lock().await;
        if cfg.history.enabled {
            history_path = config_mod::expand_tilde(&cfg.history.path);
            match history::open_for_append(&history_path).await {
                Ok(f) => {
                    history_file = Some(f);
                    // Eski mesajları yükle.
                    let old = history::load_recent(&history_path, cfg.history.max_messages_loaded).await?;
                    for entry in old {
                        let timestamp = entry.to_system_time();
                        let kind = match entry.kind.as_str() {
                            "sent" => ChatLineKind::Sent,
                            "received" => ChatLineKind::Received,
                            _ => ChatLineKind::System,
                        };
                        state.messages.push(ChatLine {
                            kind,
                            text: entry.text,
                            timestamp,
                        });
                    }
                    state.add_system(format!(
                        "History yüklendi: {} (son {} mesaj)",
                        history_path.display(),
                        cfg.history.max_messages_loaded
                    ));
                }
                Err(e) => {
                    state.add_system(format!("History açılamadı: {}", e));
                    history_file = None;
                }
            }
        } else {
            history_path = config_mod::expand_tilde(&cfg.history.path);
            history_file = None;
            state.add_system("History KAPALI (config: history.enabled=false)");
        }
    }

    loop {
        terminal.draw(|f| draw_chat(f, state))?;

        tokio::select! {
            Some(event) = net_rx.recv() => {
                match event {
                    HubEvent::ListenerReady => {
                        state.add_system("Dinleme hazır, peer bekleniyor...");
                    }
                    HubEvent::ListenerError(e) => {
                        state.add_system(format!("Dinleme hatası: {}", e));
                        state.quit = true;
                    }
                    HubEvent::PeerConnected(peer_id, _addr) => {
                        let cfg = config.lock().await;
                        let show_addr = cfg.anonymity.show_peer_addresses;
                        drop(cfg);

                        let reg = registry.lock().await;
                        let display = reg.get(peer_id)
                            .map(|p| p.display_name(show_addr))
                            .unwrap_or_else(|| format!("peer-{:x}", peer_id));
                        drop(reg);

                        state.add_system(format!("Peer bağlandı: {}", display));

                        // History'ye kaydet (enabled ise).
                        if let Some(hf) = history_file.as_mut() {
                            let entry = history::HistoryEntry::new(
                                "system",
                                format!("peer connected: {}", display),
                            );
                            let _ = history::append_entry(hf, &entry).await;
                        }
                    }
                    HubEvent::PeerDisconnected(peer_id) => {
                        let cfg = config.lock().await;
                        let show_addr = cfg.anonymity.show_peer_addresses;
                        drop(cfg);

                        let display = {
                            let reg = registry.lock().await;
                            reg.get(peer_id)
                                .map(|p| p.display_name(show_addr))
                                .unwrap_or_else(|| format!("peer-{:x}", peer_id))
                        };

                        // Registry'den ve senders'dan kaldır.
                        registry.lock().await.remove(peer_id);
                        senders.lock().await.remove(&peer_id);

                        state.add_system(format!("Peer ayrıldı: {}", display));

                        if let Some(hf) = history_file.as_mut() {
                            let entry = history::HistoryEntry::new(
                                "system",
                                format!("peer disconnected: {}", display),
                            );
                            let _ = history::append_entry(hf, &entry).await;
                        }
                    }
                    HubEvent::PeerMessage(peer_id, msg) => {
                        // Önce "clear" komutu mu kontrol et — peer kendi
                        // /clear yapmış olabilir (hub zaten clear yapmış,
                        // bu durumda ekran zaten temiz, ama yine de işle).
                        if chat::is_clear_message(&msg) {
                            // Peer clear komutu gönderdi — hub ekranını
                            // temizle ve diğer peer'lara da broadcast et.
                            state.messages.clear();
                            state.add_system("Peer /clear komutu gönderdi, ekran temizlendi.");
                            let senders_guard = senders.lock().await;
                            for (&id, sender) in senders_guard.iter() {
                                if id != peer_id {
                                    let _ = chat::send_clear_split(sender).await;
                                }
                            }
                            continue;
                        }
                        match chat::decode_chat(&msg) {
                            Ok(Some(text)) => {
                                // Peer'ın mute durumunu kontrol et.
                                let (display, muted) = {
                                    let cfg = config.lock().await;
                                    let show_addr = cfg.anonymity.show_peer_addresses;
                                    let reg = registry.lock().await;
                                    let p = reg.get(peer_id);
                                    (
                                        p.map(|pi| pi.display_name(show_addr))
                                            .unwrap_or_else(|| format!("peer-{:x}", peer_id)),
                                        p.map(|pi| pi.muted).unwrap_or(false),
                                    )
                                };

                                if muted {
                                    // Mute edilmiş peer'ın mesajı kabul edilir
                                    // ama broadcast edilmez. Hub görür.
                                    state.add_received(format!("[{}] (muted) {}", display, text));
                                } else {
                                    state.add_received(format!("[{}] {}", display, text));

                                    // History'ye kaydet.
                                    if let Some(hf) = history_file.as_mut() {
                                        let entry = history::HistoryEntry::new("received", &text);
                                        let _ = history::append_entry(hf, &entry).await;
                                    }

                                    // Star topoloji: diğer peer'lara broadcast.
                                    let senders_guard = senders.lock().await;
                                    for (&id, sender) in senders_guard.iter() {
                                        if id != peer_id {
                                            let _ = chat::send_chat_split(sender, &text).await;
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                state.add_system(format!("peer-{:x}: dosya transferi (desteklenmiyor)", peer_id));
                            }
                            Err(e) => state.add_system(format!("Geçersiz mesaj [peer-{:x}]: {}", peer_id, e)),
                        }
                    }
                }
            }
            Some(key) = kbd_rx.recv() => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    state.quit = true;
                    continue;
                }
                match key.code {
                    KeyCode::Enter => {
                        let text = state.take_input();
                        if text.is_empty() { continue; }

                        // Hub operator her zaman admin.
                        let operator_role = roles::Role::Admin;

                        // Slash komutu mu?
                        if let Some(cmd) = commands::parse(&text) {
                            match cmd {
                                commands::Command::Help => {
                                    state.add_system(commands::help_text());
                                }
                                commands::Command::Quit => {
                                    state.quit = true;
                                }
                                commands::Command::Clear => {
                                    state.messages.clear();
                                    state.add_system("Geçmiş temizlendi (tümü).");
                                    // History dosyasını da temizle (enabled ise).
                                    let cfg = config.lock().await;
                                    if cfg.history.enabled {
                                        let _ = history::clear(&history_path).await;
                                        history_file = history::open_for_append(&history_path).await.ok();
                                    }
                                    drop(cfg);
                                    // Tüm peer'lara "clear" komutu broadcast et.
                                    // Peer'lar bu mesajı alınca kendi ekranlarını
                                    // temizler (hub ile senkron).
                                    let senders_guard = senders.lock().await;
                                    for sender in senders_guard.values() {
                                        let _ = chat::send_clear_split(sender).await;
                                    }
                                }
                                commands::Command::ClearCount(n) => {
                                    let before = state.messages.len();
                                    if n >= before {
                                        state.messages.clear();
                                        state.add_system(format!("{} mesaj silindi (tümü).", before));
                                    } else {
                                        state.messages.truncate(before - n);
                                        state.add_system(format!("Son {} mesaj silindi.", n));
                                    }
                                    // /clear <N> de peer'lara broadcast (tümü temizlenir).
                                    // N sayısı peer'a iletilmez — peer tüm ekranını temizler.
                                    // Bu, hub ile peer arasında basit senkron sağlar.
                                    let senders_guard = senders.lock().await;
                                    for sender in senders_guard.values() {
                                        let _ = chat::send_clear_split(sender).await;
                                    }
                                }
                                commands::Command::Who => {
                                    let cfg = config.lock().await;
                                    let show_addr = cfg.anonymity.show_peer_addresses;
                                    drop(cfg);
                                    let reg = registry.lock().await;
                                    state.add_system(format!("Bağlı peer sayısı: {}", reg.len()));
                                    for p in reg.list() {
                                        let display = p.display_name(show_addr);
                                        state.add_system(format!("  {} [{}] {}",
                                            display, p.role,
                                            if p.muted { "(muted)" } else { "" }));
                                    }
                                }
                                commands::Command::Role => {
                                    state.add_system(format!("Senin rolün: {}", operator_role));
                                }
                                commands::Command::RoleOf(nick) => {
                                    let reg = registry.lock().await;
                                    match reg.find_by_nick(&nick) {
                                        Some(p) => {
                                            state.add_system(format!("{} rolü: {}", nick, p.role));
                                        }
                                        None => {
                                            state.add_system(format!("'{}' nick'i bulunamadı", nick));
                                        }
                                    }
                                }
                                commands::Command::OnAdmin(nick) => {
                                    // Önce registry'den peer ID'yi al, sonra lock'ı bırak.
                                    let peer_id_opt = {
                                        let mut reg = registry.lock().await;
                                        match reg.find_by_nick_mut(&nick) {
                                            Some(p) => {
                                                p.set_role(roles::Role::Admin);
                                                Some(p.id)
                                            }
                                            None => None,
                                        }
                                    };
                                    match peer_id_opt {
                                        Some(id) => {
                                            state.add_system(format!("{} artık admin!", nick));
                                            // Peer'a bildir (lock serbest).
                                            let senders_guard = senders.lock().await;
                                            if let Some(sender) = senders_guard.get(&id) {
                                                let _ = chat::send_chat_split(
                                                    sender,
                                                    &format!("[sistem] {} admin oldun!", nick),
                                                )
                                                .await;
                                            }
                                        }
                                        None => {
                                            state.add_system(format!("'{}' nick'i bulunamadı", nick));
                                        }
                                    }
                                }
                                commands::Command::OffAdmin(nick) => {
                                    let mut reg = registry.lock().await;
                                    match reg.find_by_nick_mut(&nick) {
                                        Some(p) => {
                                            p.set_role(roles::Role::User);
                                            state.add_system(format!("{} admin rolü alındı, user oldu.", nick));
                                        }
                                        None => {
                                            state.add_system(format!("'{}' nick'i bulunamadı", nick));
                                        }
                                    }
                                }
                                commands::Command::Kick(nick) => {
                                    let reg = registry.lock().await;
                                    match reg.find_by_nick(&nick) {
                                        Some(p) => {
                                            let id = p.id;
                                            drop(reg);
                                            // Sender'ı kaldır (bağlantı kapanır).
                                            senders.lock().await.remove(&id);
                                            registry.lock().await.remove(id);
                                            state.add_system(format!("{} atıldı.", nick));
                                        }
                                        None => {
                                            drop(reg);
                                            state.add_system(format!("'{}' nick'i bulunamadı", nick));
                                        }
                                    }
                                }
                                commands::Command::Mute(nick) => {
                                    let mut reg = registry.lock().await;
                                    match reg.find_by_nick_mut(&nick) {
                                        Some(p) => {
                                            p.set_muted(true);
                                            state.add_system(format!("{} susturuldu.", nick));
                                        }
                                        None => {
                                            state.add_system(format!("'{}' nick'i bulunamadı", nick));
                                        }
                                    }
                                }
                                commands::Command::Unmute(nick) => {
                                    let mut reg = registry.lock().await;
                                    match reg.find_by_nick_mut(&nick) {
                                        Some(p) => {
                                            p.set_muted(false);
                                            state.add_system(format!("{} susturması kaldırıldı.", nick));
                                        }
                                        None => {
                                            state.add_system(format!("'{}' nick'i bulunamadı", nick));
                                        }
                                    }
                                }
                                commands::Command::Config => {
                                    let cfg = config.lock().await;
                                    let json = config_mod::to_pretty_json(&cfg).unwrap_or_else(|e| e.to_string());
                                    state.add_system(format!("Config:\n{}", json));
                                }
                                commands::Command::ConfigSet(key, value) => {
                                    let mut cfg = config.lock().await;
                                    match config_mod::set_field(&mut cfg, &key, &value) {
                                        Ok(()) => {
                                            state.add_system(format!("Config güncellendi: {} = {}", key, value));
                                            // Dosyaya kaydet.
                                            if let Err(e) = config_mod::save(config_path, &cfg).await {
                                                state.add_system(format!("Config kaydetme hatası: {}", e));
                                            }
                                        }
                                        Err(e) => {
                                            state.add_system(format!("Config hatası: {}", e));
                                        }
                                    }
                                }
                                commands::Command::Send(_) => {
                                    state.add_system("Hub modunda /send desteklenmiyor (broadcast mesaj gönderin)");
                                }
                                commands::Command::Nick(name) => {
                                    // Hub operator nick ayarlayamaz (hub kimliği sabit).
                                    state.add_system(format!("Hub operator nick'i değiştiremez (sen hub'sun). İstenen: {}", name));
                                }
                                commands::Command::Unknown(msg) => {
                                    state.add_system(msg);
                                }
                            }
                        } else {
                            // Normal mesaj — tüm peer'lara broadcast.
                            let senders_guard = senders.lock().await;
                            if senders_guard.is_empty() {
                                state.add_system("Bağlı peer yok, mesaj gönderilemedi.");
                            } else {
                                for sender in senders_guard.values() {
                                    let _ = chat::send_chat_split(sender, &text).await;
                                }
                                state.add_sent(text.clone());
                                if let Some(hf) = history_file.as_mut() {
                                    let entry = history::HistoryEntry::new("sent", &text);
                                    let _ = history::append_entry(hf, &entry).await;
                                }
                            }
                        }
                    }
                    KeyCode::Char(c) if c == '\x7f' || c == '\x08' => {
                        // Terminal Backspace için \x7f (DEL) veya \x08 (BS)
                        // gönderebilir. Her ikisini de backspace olarak ele al.
                        state.backspace();
                    }
                    KeyCode::Char(c) => state.insert_char(c),
                    KeyCode::Backspace => state.backspace(),
                    KeyCode::Left => state.move_left(),
                    KeyCode::Right => state.move_right(),
                    KeyCode::Up => state.scroll_up(),
                    KeyCode::Down => state.scroll_down(),
                    KeyCode::Home => state.move_home(),
                    KeyCode::End => state.move_end(),
                    KeyCode::Esc => state.quit = true,
                    _ => {}
                }
            }
        }

        if state.quit {
            break;
        }
    }
    Ok(())
}

/// Ana event loop. Ağ mesajları, klavye girdileri ve render'ı yönetir.
async fn chat_event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state: &mut ChatState,
    sender: &Sender,
    net_rx: &mut mpsc::Receiver<Option<Vec<u8>>>,
    kbd_rx: &mut mpsc::Receiver<KeyEvent>,
) -> Result<()> {
    use crate::history;

    // History dosyasını aç ve eski mesajları yükle.
    let history_path = history::default_history_path();
    let mut history_file = history::open_for_append(&history_path).await?;
    let old = history::load_recent(&history_path, 50).await?;
    for entry in old {
        let timestamp = entry.to_system_time();
        let kind = match entry.kind.as_str() {
            "sent" => ChatLineKind::Sent,
            "received" => ChatLineKind::Received,
            _ => ChatLineKind::System,
        };
        state.messages.push(ChatLine {
            kind,
            text: entry.text,
            timestamp,
        });
    }

    loop {
        // Render
        terminal.draw(|f| draw_chat(f, state))?;

        // Wait for next event (network veya keyboard)
        tokio::select! {
            Some(msg_opt) = net_rx.recv() => {
                match msg_opt {
                    None => {
                        state.connected = false;
                        state.add_system("Peer bağlantıyı kapattı. Esc ile çıkın.");
                        let entry = history::HistoryEntry::new("system", "peer disconnected");
                        let _ = history::append_entry(&mut history_file, &entry).await;
                    }
                    Some(msg) => {
                        // Önce "clear" komutu mu kontrol et — hub clear
                        // göndermiş olabilir. Bu durumda ekranı temizle.
                        if chat::is_clear_message(&msg) {
                            state.messages.clear();
                            state.add_system("Hub ekranı temizledi (/clear broadcast).");
                            continue;
                        }
                        match chat::decode_chat(&msg) {
                            Ok(Some(text)) => {
                                state.add_received(text.clone());
                                let entry = history::HistoryEntry::new("received", &text);
                                let _ = history::append_entry(&mut history_file, &entry).await;
                            }
                            Ok(None) => {
                                state.add_system(
                                    "Peer dosya transferi başlattı (chat modunda desteklenmiyor)."
                                );
                            }
                            Err(e) => state.add_system(format!("Geçersiz mesaj: {}", e)),
                        }
                    }
                }
            }
            Some(key) = kbd_rx.recv() => {
                handle_key(key, state, sender, &mut history_file, &history_path).await?;
            }
        }

        if state.quit {
            break;
        }
    }
    Ok(())
}

/// Klavye olayını işle. Mesaj gönderme, komutlar, düzenleme, kaydırma, çıkma.
///
/// `history_file` ve `history_path` mesajları diske yazmak ve `/clear`
/// komutu için kullanılır.
async fn handle_key(
    key: KeyEvent,
    state: &mut ChatState,
    sender: &Sender,
    history_file: &mut tokio::fs::File,
    history_path: &std::path::Path,
) -> Result<()> {
    use crate::commands;
    use crate::history;

    // Ctrl+C → quit
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.quit = true;
        return Ok(());
    }

    match key.code {
        KeyCode::Enter => {
            let text = state.take_input();
            if text.is_empty() {
                return Ok(());
            }

            // Slash komutu mu?
            if let Some(cmd) = commands::parse(&text) {
                match cmd {
                    commands::Command::Help => {
                        state.add_system(commands::help_text());
                    }
                    commands::Command::Quit => {
                        state.quit = true;
                    }
                    commands::Command::Clear => {
                        state.messages.clear();
                        state.add_system("Geçmiş temizlendi.");
                        let _ = history::clear(history_path).await;
                        *history_file = history::open_for_append(history_path).await?;
                        // Peer'a clear komutu gönder (tek-peer modunda).
                        if state.connected {
                            let _ = chat::send_clear_split(sender).await;
                        }
                    }
                    commands::Command::ClearCount(n) => {
                        let before = state.messages.len();
                        if n >= before {
                            state.messages.clear();
                            state.add_system(format!("{} mesaj silindi (tümü).", before));
                        } else {
                            state.messages.truncate(before - n);
                            state.add_system(format!("Son {} mesaj silindi.", n));
                        }
                        // Peer'a clear gönder (tüm ekran temizlenir).
                        if state.connected {
                            let _ = chat::send_clear_split(sender).await;
                        }
                    }
                    commands::Command::Who => {
                        state.add_system("Tek-peer modunda (hub modu için --multi kullanın)");
                    }
                    commands::Command::Role => {
                        state.add_system("Tek-peer modunda rol sistemi yok (hub: --multi)");
                    }
                    commands::Command::RoleOf(_) => {
                        state.add_system("Tek-peer modunda rol sistemi yok (hub: --multi)");
                    }
                    commands::Command::OnAdmin(_) | commands::Command::OffAdmin(_)
                    | commands::Command::Kick(_) | commands::Command::Mute(_)
                    | commands::Command::Unmute(_) => {
                        state.add_system("Bu komut sadece hub modunda (--multi) çalışır");
                    }
                    commands::Command::Config | commands::Command::ConfigSet(_, _) => {
                        state.add_system("Config yönetimi sadece hub modunda (--multi) çalışır");
                    }
                    commands::Command::Send(path) => {
                        // Tek-peer modunda dosya gönderimi.
                        if state.connected {
                            state.add_system(format!(
                                "/send {} — dosya transferi tek-peer modunda yakında",
                                path.display()
                            ));
                        } else {
                            state.add_system("Bağlı peer yok, dosya gönderilemedi.");
                        }
                    }
                    commands::Command::Nick(_) => {
                        state.add_system("/nick henüz desteklenmiyor");
                    }
                    commands::Command::Unknown(msg) => {
                        state.add_system(msg);
                    }
                }
            } else if state.connected {
                // Normal mesaj — peer'a gönder.
                match chat::send_chat_split(sender, &text).await {
                    Ok(()) => {
                        state.add_sent(text.clone());
                        let entry = history::HistoryEntry::new("sent", &text);
                        let _ = history::append_entry(history_file, &entry).await;
                    }
                    Err(e) => state.add_system(format!("Gönderme hatası: {}", e)),
                }
            } else {
                state.add_system("Bağlı peer yok, mesaj gönderilemedi.");
            }
        }
        KeyCode::Char(c) if c == '\x7f' || c == '\x08' => {
            // Terminal Backspace için \x7f (DEL) veya \x08 (BS)
            state.backspace();
        }
        KeyCode::Char(c) => state.insert_char(c),
        KeyCode::Backspace => state.backspace(),
        KeyCode::Left => state.move_left(),
        KeyCode::Right => state.move_right(),
        KeyCode::Up => state.scroll_up(),
        KeyCode::Down => state.scroll_down(),
        KeyCode::Home => state.move_home(),
        KeyCode::End => state.move_end(),
        KeyCode::Esc => state.quit = true,
        _ => {}
    }
    Ok(())
}

/// Tek bir çerçeve çizer. Layout: title (3) | history (flex) | input (3) | status (1).
fn draw_chat(
    f: &mut ratatui::Frame<CrosstermBackend<std::io::Stdout>>,
    state: &ChatState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(5),    // history
            Constraint::Length(3), // input
            Constraint::Length(1), // status
        ])
        .split(f.size());

    // Title
    let title = Paragraph::new("OnionChat v0.1 — Tor P2P E2EE Chat")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // History
    let history_height = chunks[1].height.saturating_sub(2) as usize; // kenarlıklar hariç
    let lines = visible_lines(state, history_height);
    let history = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Sohbet "))
        .wrap(Wrap { trim: false });
    f.render_widget(history, chunks[1]);

    // Input
    let input = Paragraph::new(state.input.as_str())
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL).title(" Mesaj > "));
    f.render_widget(input, chunks[2]);

    // Cursor — input kutusunda imleci göster
    let display_pos = state.input[..state.cursor_pos].chars().count() as u16;
    let input_x = chunks[2].x + 1 + display_pos;
    let input_y = chunks[2].y + 1;
    f.set_cursor(input_x, input_y);

    // Status
    let mode_str = match state.mode {
        UiMode::Listen => "listen",
        UiMode::Connect => "connect",
    };
    let conn_str = if state.connected { "bağlı" } else { "koptu" };
    let status_text = format!(
        " [{}] peer={} | {} | Enter: gönder | Esc: çık | \u{2191}\u{2193}: kaydır",
        mode_str, state.peer_addr, conn_str
    );
    let status = Paragraph::new(status_text).style(Style::default().fg(Color::Yellow));
    f.render_widget(status, chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn test_state() -> ChatState {
        ChatState::new(
            UiMode::Listen,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        )
    }

    #[test]
    fn insert_char_appends_to_input() {
        let mut s = test_state();
        s.insert_char('a');
        s.insert_char('b');
        s.insert_char('c');
        assert_eq!(s.input, "abc");
        assert_eq!(s.cursor_pos, 3);
    }

    #[test]
    fn insert_char_handles_utf8() {
        let mut s = test_state();
        s.insert_char('m');
        s.insert_char('e');
        s.insert_char('r');
        s.insert_char('h');
        s.insert_char('a');
        s.insert_char('b');
        s.insert_char('a');
        s.insert_char(' ');
        s.insert_char('d'); // Türkçe 'ü' yerine ASCII test
        assert_eq!(s.input, "merhaba d");
        assert_eq!(s.cursor_pos, "merhaba d".len());
    }

    #[test]
    fn insert_char_turkish_lowercase_chars() {
        // Tüm Türkçe küçük harfler: ş ğ ü ö ç ı
        let mut s = test_state();
        s.insert_char('ş');
        s.insert_char('ğ');
        s.insert_char('ü');
        s.insert_char('ö');
        s.insert_char('ç');
        s.insert_char('ı');
        assert_eq!(s.input, "şğüöçı");
        // Her karakter 2 bayt (UTF-8) → 6 × 2 = 12 bayt
        assert_eq!(s.cursor_pos, 12);
    }

    #[test]
    fn insert_char_turkish_uppercase_chars() {
        // Tüm Türkçe büyük harfler: Ş Ğ Ü Ö Ç İ
        let mut s = test_state();
        s.insert_char('Ş');
        s.insert_char('Ğ');
        s.insert_char('Ü');
        s.insert_char('Ö');
        s.insert_char('Ç');
        s.insert_char('İ');
        assert_eq!(s.input, "ŞĞÜÖÇİ");
        assert_eq!(s.cursor_pos, 12); // 6 × 2 bayt
    }

    #[test]
    fn insert_char_turkish_mixed_with_ascii() {
        // ASCII + Türkçe karışık
        let mut s = test_state();
        s.insert_char('M');
        s.insert_char('e');
        s.insert_char('r');
        s.insert_char('h');
        s.insert_char('a');
        s.insert_char('b');
        s.insert_char('a');
        s.insert_char(' ');
        s.insert_char('d');
        s.insert_char('ü');
        s.insert_char('n');
        s.insert_char('y');
        s.insert_char('a');
        assert_eq!(s.input, "Merhaba dünya");
        // 'ü' 2 bayt, gerisi 1 bayt → 12 + 1 = 13 bayt
        assert_eq!(s.cursor_pos, "Merhaba dünya".len());
    }

    #[test]
    fn insert_char_turkish_word_istanbul() {
        let mut s = test_state();
        for c in "İstanbul".chars() {
            s.insert_char(c);
        }
        assert_eq!(s.input, "İstanbul");
        // 'İ' 2 bayt, gerisi 1 bayt → 2 + 7 = 9 bayt
        assert_eq!(s.cursor_pos, 9);
    }

    #[test]
    fn backspace_at_cursor_middle_removes_before_cursor() {
        // Cursor ortadayken backspace — cursor'dan önceki karakter silinir.
        // "hello" → cursor 'e' ile 'l' arasinda (pos 2) → backspace → "helo"
        // Bekle: cursor pos 2 = "h" ve "e" arasi. backspace "h"'i siler → "ello"
        // Aslında: pos 2 'e'den sonra. backspace 'e'yi siler → "hllo"
        let mut s = test_state();
        s.input = "hello".to_string();
        s.cursor_pos = 2; // 'h'|'e'|'l'... → cursor 'e' sonrasinda
        s.backspace();
        assert_eq!(s.input, "hllo");
        assert_eq!(s.cursor_pos, 1);
    }

    #[test]
    fn backspace_turkish_at_cursor_middle() {
        // "aşb" → cursor 'ş' sonrasinda (pos 3) → backspace 'ş' siler → "ab"
        let mut s = test_state();
        s.input = "aşb".to_string(); // 'a'(1) + 'ş'(2) + 'b'(1) = 4 bayt
        s.cursor_pos = 3; // 'ş' sonrasinda, 'b' oncesinde
        s.backspace();
        assert_eq!(s.input, "ab");
        assert_eq!(s.cursor_pos, 1);
    }

    #[test]
    fn backspace_multiple_times_clears_input() {
        // Tüm input'u tek tek sil
        let mut s = test_state();
        s.input = "abc".to_string();
        s.cursor_pos = 3;
        s.backspace();
        s.backspace();
        s.backspace();
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
        // Bir daha backspace — hiçbir şey olmaz
        s.backspace();
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn backspace_after_insert_at_middle() {
        // "abc" → cursor 'b' oncesinde (pos 1) → 'X' insert → "aXbc"
        // → backspace → "abc" (X silinir, cursor pos 1)
        let mut s = test_state();
        s.input = "abc".to_string();
        s.cursor_pos = 1;
        s.insert_char('X');
        assert_eq!(s.input, "aXbc");
        assert_eq!(s.cursor_pos, 2);
        s.backspace();
        assert_eq!(s.input, "abc");
        assert_eq!(s.cursor_pos, 1);
    }

    #[test]
    fn backspace_turkish_after_insert_at_middle() {
        // "abc" → cursor pos 1 → 'ş' insert → "aşbc"
        // → backspace → "abc"
        let mut s = test_state();
        s.input = "abc".to_string();
        s.cursor_pos = 1;
        s.insert_char('ş');
        assert_eq!(s.input, "aşbc");
        assert_eq!(s.cursor_pos, 3); // 'a'(1) + 'ş'(2) = 3
        s.backspace();
        assert_eq!(s.input, "abc");
        assert_eq!(s.cursor_pos, 1);
    }

    #[test]
    fn backspace_empty_input_does_nothing() {
        let mut s = test_state();
        s.backspace();
        s.backspace();
        s.backspace();
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn backspace_single_turkish_char() {
        let mut s = test_state();
        s.input = "ş".to_string();
        s.cursor_pos = 2;
        s.backspace();
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn backspace_preserves_other_turkish_chars() {
        // "şğ" → backspace 'ğ' siler → "ş"
        let mut s = test_state();
        s.input = "şğ".to_string();
        s.cursor_pos = 4;
        s.backspace();
        assert_eq!(s.input, "ş");
        assert_eq!(s.cursor_pos, 2);
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut s = test_state();
        s.input = "abc".to_string();
        s.cursor_pos = 3;
        s.backspace();
        assert_eq!(s.input, "ab");
        assert_eq!(s.cursor_pos, 2);
    }

    #[test]
    fn backspace_at_zero_does_nothing() {
        let mut s = test_state();
        s.input = "abc".to_string();
        s.cursor_pos = 0;
        s.backspace();
        assert_eq!(s.input, "abc");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn backspace_handles_utf8() {
        let mut s = test_state();
        s.input = "abç".to_string(); // 'ç' = 2 bayt
        s.cursor_pos = 4; // 'a' + 'b' + 'ç'(2) = 4 bayt
        s.backspace();
        assert_eq!(s.input, "ab");
        assert_eq!(s.cursor_pos, 2);
    }

    #[test]
    fn backspace_turkish_s_cedilla() {
        // 'ş' = 2 bayt
        let mut s = test_state();
        s.input = "şşş".to_string(); // 6 bayt
        s.cursor_pos = 6;
        s.backspace();
        assert_eq!(s.input, "şş");
        assert_eq!(s.cursor_pos, 4);
    }

    #[test]
    fn backspace_turkish_g_breve() {
        // 'ğ' = 2 bayt
        let mut s = test_state();
        s.input = "ağb".to_string(); // 'a'(1) + 'ğ'(2) + 'b'(1) = 4 bayt
        s.cursor_pos = 4;
        s.backspace();
        assert_eq!(s.input, "ağ");
        assert_eq!(s.cursor_pos, 3);
    }

    #[test]
    fn backspace_turkish_i_dotless() {
        // 'ı' = 2 bayt
        let mut s = test_state();
        s.input = "ıııı".to_string(); // 8 bayt
        s.cursor_pos = 8;
        s.backspace();
        s.backspace();
        assert_eq!(s.input, "ıı");
        assert_eq!(s.cursor_pos, 4);
    }

    #[test]
    fn backspace_turkish_capital_i_with_dot() {
        // 'İ' = 2 bayt, "İstanbul" = 9 bayt
        let mut s = test_state();
        s.input = "İstanbul".to_string();
        s.cursor_pos = 9;
        // ASCII karakterleri tek tek sil (l, u, b, n, a, t, s)
        s.backspace(); // 'l' → "İstanbu" (8 bayt)
        assert_eq!(s.input, "İstanbu");
        assert_eq!(s.cursor_pos, 8);
        s.backspace(); // 'u' → "İstanb"
        s.backspace(); // 'b' → "İstan"
        s.backspace(); // 'n' → "İsta"
        s.backspace(); // 'a' → "İst"
        s.backspace(); // 't' → "İs"
        s.backspace(); // 's' → "İ"
        assert_eq!(s.input, "İ");
        assert_eq!(s.cursor_pos, 2);
        // Şimdi 'İ' (2 bayt) sil
        s.backspace();
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn backspace_mixed_turkish_and_ascii() {
        // "Merhaba dünya" — 'ü' 2 bayt, gerisi 1 bayt
        // M,e,r,h,a,b,a,space,d,ü(2),n,y,a = 13 karakter, 14 bayt
        let text = "Merhaba dünya";
        let mut s = test_state();
        s.input = text.to_string();
        s.cursor_pos = text.len(); // 14
        s.backspace(); // 'a' sil → "Merhaba düny" (13 bayt)
        assert_eq!(s.input, "Merhaba düny");
        s.backspace(); // 'y' sil → "Merhaba dün" (12 bayt)
        assert_eq!(s.input, "Merhaba dün");
        s.backspace(); // 'n' sil → "Merhaba dü" (11 bayt)
        assert_eq!(s.input, "Merhaba dü");
        s.backspace(); // 'ü' sil (2 bayt) → "Merhaba d" (9 bayt)
        assert_eq!(s.input, "Merhaba d");
        assert_eq!(s.cursor_pos, "Merhaba d".len()); // 9
    }

    #[test]
    fn backspace_all_turkish_special_chars_one_by_one() {
        // Her Türkçe özel karakteri tek tek sil
        let mut s = test_state();
        s.input = "şğöüçİ".to_string(); // 12 bayt
        s.cursor_pos = 12;
        s.backspace(); // 'İ' sil
        assert_eq!(s.input, "şğöüç");
        s.backspace(); // 'ç' sil
        assert_eq!(s.input, "şğöü");
        s.backspace(); // 'ü' sil
        assert_eq!(s.input, "şğö");
        s.backspace(); // 'ö' sil
        assert_eq!(s.input, "şğ");
        s.backspace(); // 'ğ' sil
        assert_eq!(s.input, "ş");
        s.backspace(); // 'ş' sil
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn move_left_right_navigates_cursor() {
        let mut s = test_state();
        s.input = "abc".to_string();
        s.cursor_pos = 3;
        s.move_left();
        assert_eq!(s.cursor_pos, 2);
        s.move_left();
        assert_eq!(s.cursor_pos, 1);
        s.move_right();
        assert_eq!(s.cursor_pos, 2);
    }

    #[test]
    fn move_left_turkish_char_2_bytes() {
        // 'ş' = 2 bayt → move_left 2 bayt geri gitmeli
        let mut s = test_state();
        s.input = "aşb".to_string(); // 'a'(1) + 'ş'(2) + 'b'(1) = 4 bayt
        s.cursor_pos = 4;
        s.move_left(); // 'b' üzerinden → 3
        assert_eq!(s.cursor_pos, 3);
        s.move_left(); // 'ş' üzerinden → 1 (2 bayt geri)
        assert_eq!(s.cursor_pos, 1);
    }

    #[test]
    fn move_right_turkish_char_2_bytes() {
        // 'ş' = 2 bayt → move_right 2 bayt ileri gitmeli
        let mut s = test_state();
        s.input = "aşb".to_string();
        s.cursor_pos = 0;
        s.move_right(); // 'a' üzerinden → 1
        assert_eq!(s.cursor_pos, 1);
        s.move_right(); // 'ş' üzerinden → 3 (2 bayt ileri)
        assert_eq!(s.cursor_pos, 3);
    }

    #[test]
    fn move_left_right_through_all_turkish_chars() {
        // "şğöüç" — hepsi 2 bayt
        let mut s = test_state();
        s.input = "şğöüç".to_string();
        s.cursor_pos = 10;
        // Sondan başa doğru her karakteri geç
        s.move_left(); // 'ç' → 8
        assert_eq!(s.cursor_pos, 8);
        s.move_left(); // 'ü' → 6
        assert_eq!(s.cursor_pos, 6);
        s.move_left(); // 'ö' → 4
        assert_eq!(s.cursor_pos, 4);
        s.move_left(); // 'ğ' → 2
        assert_eq!(s.cursor_pos, 2);
        s.move_left(); // 'ş' → 0
        assert_eq!(s.cursor_pos, 0);
        // Şimdi sağa doğru
        s.move_right(); // → 2
        assert_eq!(s.cursor_pos, 2);
        s.move_right(); // → 4
        assert_eq!(s.cursor_pos, 4);
    }

    #[test]
    fn move_left_at_zero_does_nothing() {
        let mut s = test_state();
        s.cursor_pos = 0;
        s.move_left();
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn move_right_at_end_does_nothing() {
        let mut s = test_state();
        s.input = "abc".to_string();
        s.cursor_pos = 3;
        s.move_right();
        assert_eq!(s.cursor_pos, 3);
    }

    #[test]
    fn move_left_at_zero_with_turkish_input() {
        let mut s = test_state();
        s.input = "şğö".to_string();
        s.cursor_pos = 0;
        s.move_left();
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn move_right_at_end_with_turkish_input() {
        let mut s = test_state();
        s.input = "şğö".to_string();
        s.cursor_pos = 6; // 3 × 2 bayt
        s.move_right();
        assert_eq!(s.cursor_pos, 6);
    }

    #[test]
    fn home_end_with_turkish_input() {
        // "Merhaba dünya" — 'ü' 2 bayt → toplam 14 bayt
        let text = "Merhaba dünya";
        let mut s = test_state();
        s.input = text.to_string();
        s.cursor_pos = 5;
        s.move_home();
        assert_eq!(s.cursor_pos, 0);
        s.move_end();
        assert_eq!(s.cursor_pos, text.len()); // 14
    }

    #[test]
    fn take_input_clears_and_returns() {
        let mut s = test_state();
        s.input = "merhaba".to_string();
        s.cursor_pos = 7;
        let taken = s.take_input();
        assert_eq!(taken, "merhaba");
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn take_input_turkish_text() {
        let text = "Merhaba dünya";
        let mut s = test_state();
        s.input = text.to_string();
        s.cursor_pos = text.len();
        let taken = s.take_input();
        assert_eq!(taken, text);
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn take_input_all_turkish_chars() {
        let mut s = test_state();
        let text = "Şş Ğğ Üü Öö Çç İı";
        s.input = text.to_string();
        s.cursor_pos = text.len();
        let taken = s.take_input();
        assert_eq!(taken, text);
        assert_eq!(s.input, "");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn add_sent_resets_scroll() {
        let mut s = test_state();
        s.scroll_offset = 5;
        s.add_sent("test");
        assert_eq!(s.scroll_offset, 0);
        assert_eq!(s.messages.len(), 1);
        assert_eq!(s.messages[0].kind, ChatLineKind::Sent);
        assert_eq!(s.messages[0].text, "test");
    }

    #[test]
    fn add_received_resets_scroll() {
        let mut s = test_state();
        s.scroll_offset = 3;
        s.add_received("hello");
        assert_eq!(s.scroll_offset, 0);
        assert_eq!(s.messages[0].kind, ChatLineKind::Received);
    }

    #[test]
    fn scroll_up_increments_offset() {
        let mut s = test_state();
        s.scroll_up();
        s.scroll_up();
        assert_eq!(s.scroll_offset, 2);
    }

    #[test]
    fn scroll_down_decrements_offset() {
        let mut s = test_state();
        s.scroll_offset = 5;
        s.scroll_down();
        assert_eq!(s.scroll_offset, 4);
    }

    #[test]
    fn scroll_down_at_zero_stays_zero() {
        let mut s = test_state();
        s.scroll_down();
        assert_eq!(s.scroll_offset, 0);
    }

    #[test]
    fn home_end_move_cursor_to_extremes() {
        let mut s = test_state();
        s.input = "merhaba".to_string();
        s.cursor_pos = 3;
        s.move_home();
        assert_eq!(s.cursor_pos, 0);
        s.move_end();
        assert_eq!(s.cursor_pos, 7);
    }

    #[test]
    fn format_timestamp_returns_hh_mm_format() {
        // Bu test sadece formatın çalıştığını doğrular; belirli bir saat
        // değerini test etmez çünkü SystemTime::now() UTC'dir.
        let ts = SystemTime::now();
        let formatted = format_timestamp(ts);
        assert_eq!(formatted.len(), 5);
        assert_eq!(formatted.chars().nth(2), Some(':'));
    }

    #[test]
    fn visible_lines_empty_state_returns_one_empty_line() {
        let s = test_state();
        let lines = visible_lines(&s, 10);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn visible_lines_returns_last_n_messages() {
        let mut s = test_state();
        for i in 0..10 {
            s.add_received(format!("msg{}", i));
        }
        let lines = visible_lines(&s, 3);
        assert_eq!(lines.len(), 3);
        // Son 3 mesaj: msg7, msg8, msg9
    }

    #[test]
    fn visible_lines_respects_scroll_offset() {
        let mut s = test_state();
        for i in 0..10 {
            s.add_received(format!("msg{}", i));
        }
        s.scroll_offset = 2; // 2 satır yukarı
        let lines = visible_lines(&s, 3);
        assert_eq!(lines.len(), 3);
        // msg5, msg6, msg7 (msg7,8,9 yerine)
    }

    #[test]
    fn line_to_spans_sent_has_arrow_prefix() {
        let line = ChatLine::sent("hello");
        let spans = line_to_spans(&line);
        // Spans içeriğini doğrulamak zor, sadece panic olmamasını kontrol et.
        assert!(!spans.0.is_empty());
    }

    #[test]
    fn chat_line_factory_constructors_set_kind() {
        let sent = ChatLine::sent("x");
        assert_eq!(sent.kind, ChatLineKind::Sent);
        let recv = ChatLine::received("y");
        assert_eq!(recv.kind, ChatLineKind::Received);
        let sys = ChatLine::system("z");
        assert_eq!(sys.kind, ChatLineKind::System);
    }
}
