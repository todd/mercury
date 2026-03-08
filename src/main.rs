mod error;
mod irc;
mod tui;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ::irc::client::prelude::{Command, Prefix, Response};
use ratatui::{Terminal, backend::CrosstermBackend};
use tracing::error;
use tracing_subscriber::EnvFilter;

use crate::irc::channel::ChannelManager;
use crate::irc::message::OutboundMessage;
use crate::irc::user::{NickServStatus, UserManager};
use crate::tui::app::{App, BufferLine, ChatMessage, MemberEntry};
use crate::tui::ui::draw;

/// Acquire the IRC stream exactly once after a successful connect and store it
/// on `app`. Logs an error message if the stream cannot be obtained.
fn attach_stream(app: &mut App) {
    match app.client.stream() {
        Ok(s) => app.irc_stream = Some(s),
        Err(e) => app.push_server_msg(format!("Warning: could not open IRC stream: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging (RUST_LOG=debug mercury for verbose output)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();

    // Initialise terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Build app — default disconnected state
    let mut app = App::new_disconnected("localhost", 6667, "mercury");
    app.push_server_msg("Welcome to Mercury. Type /connect <server> [port] [nick] to connect.");
    app.push_server_msg("Commands: /connect  /join  /create  /part  /quit  /help");

    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        error!("fatal: {}", e);
        eprintln!("Error: {e}");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Main application loop
// ---------------------------------------------------------------------------

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        // Poll for terminal events (non-blocking with short timeout)
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(app, key.code, key.modifiers).await;
            }
        }

        // Poll for IRC messages if connected
        if app.client.state() == crate::irc::client::ClientState::Connected {
            poll_irc_messages(app).await;
        }

        if app.should_quit {
            break;
        }

        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Key event handling
// ---------------------------------------------------------------------------

async fn handle_key_event(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        // Quit via Ctrl-C or Ctrl-Q
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            app.irc_stream = None;
            let _ = app.client.disconnect().await;
        }
        KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            app.irc_stream = None;
            let _ = app.client.disconnect().await;
        }

        // Enter — submit input
        KeyCode::Enter => {
            let line = app.input_take();
            if !line.is_empty() {
                handle_input_line(app, line).await;
            }
        }

        // Backspace
        KeyCode::Backspace => {
            app.input_backspace();
        }

        // Alt+Up — previous channel / PM in nav list
        KeyCode::Up if modifiers.contains(KeyModifiers::ALT) => {
            app.prev_channel();
        }

        // Alt+Down — next channel / PM in nav list
        KeyCode::Down if modifiers.contains(KeyModifiers::ALT) => {
            app.next_channel();
        }

        // Up arrow (no modifier) — scroll message pane up one line
        KeyCode::Up => {
            app.scroll_up(1);
        }

        // Down arrow (no modifier) — scroll message pane down one line
        KeyCode::Down => {
            app.scroll_down(1);
        }

        // Page Up — scroll message pane up one page
        KeyCode::PageUp => {
            let page = app.message_pane_height.saturating_sub(2) as usize;
            app.scroll_up(page.max(1));
        }

        // Page Down — scroll message pane down one page
        KeyCode::PageDown => {
            let page = app.message_pane_height.saturating_sub(2) as usize;
            app.scroll_down(page.max(1));
        }

        // Regular characters
        KeyCode::Char(c) => {
            app.input_push(c);
        }

        // Escape — clear input
        KeyCode::Esc => {
            let _ = app.input_take();
            app.clear_status();
        }

        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Command / message dispatch
// ---------------------------------------------------------------------------

async fn handle_input_line(app: &mut App, line: String) {
    if line.starts_with('/') {
        handle_command(app, &line).await;
    } else {
        // Send as PRIVMSG to current channel
        if let Some(channel) = app.active_channel.clone() {
            let msg = OutboundMessage::PrivMsg {
                target: channel.clone(),
                text: line.clone(),
            };
            match app.client.send(&msg) {
                Ok(()) => {
                    app.push_channel_line(
                        &channel,
                        BufferLine::Chat(ChatMessage {
                            nick: "me".to_string(),
                            text: line,
                        }),
                    );
                }
                Err(e) => {
                    app.set_status(format!("send error: {e}"));
                }
            }
        } else {
            app.set_status("not in a channel — use /join #channel".to_string());
        }
    }
}

async fn handle_command(app: &mut App, line: &str) {
    let parts: Vec<&str> = line.splitn(4, ' ').collect();
    let cmd = parts[0].to_lowercase();

    match cmd.as_str() {
        "/connect" => {
            let server = parts.get(1).copied().unwrap_or("localhost");
            let port: u16 = parts
                .get(2)
                .and_then(|p| p.parse().ok())
                .unwrap_or(6667);
            let nick = parts.get(3).copied().unwrap_or("mercury");

            app.push_server_msg(format!("Connecting to {}:{}…", server, port));

            let config = crate::irc::client::ClientConfig::new(server, port, nick);
            app.client = crate::irc::client::IrcClient::new(config);
            app.channel_mgr = ChannelManager::new();
            app.user_mgr = UserManager::new(nick).expect("nick validated by ClientConfig");
            app.irc_stream = None;

            match app.client.connect().await {
                Ok(()) => {
                    attach_stream(app);
                    app.push_server_msg(format!("Connected to {}:{}", server, port));
                    app.set_status(format!("connected to {}", server));
                }
                Err(e) => {
                    app.push_server_msg(format!("Connection failed: {}", e));
                    app.set_status(format!("connection failed: {}", e));
                }
            }
        }

        "/disconnect" => {
            app.irc_stream = None;
            match app.client.disconnect().await {
                Ok(()) => {
                    app.push_server_msg("Disconnected.");
                    app.set_status("disconnected".to_string());
                }
                Err(e) => {
                    app.set_status(format!("disconnect error: {}", e));
                }
            }
        }

        "/join" => {
            let channel = match parts.get(1) {
                Some(c) => *c,
                None => {
                    app.set_status("usage: /join #channel".to_string());
                    return;
                }
            };
            match app.channel_mgr.join(channel) {
                Ok(msg) => match app.client.send(&msg) {
                    Ok(()) => {
                        app.push_server_msg(format!("Joining {}…", channel));
                        app.set_active_channel(Some(channel.to_string()));
                    }
                    Err(e) => {
                        app.set_status(format!("join error: {}", e));
                    }
                },
                Err(e) => {
                    app.set_status(format!("invalid channel: {}", e));
                }
            }
        }

        "/part" | "/leave" => {
            let channel = parts
                .get(1)
                .map(|s| s.to_string())
                .or_else(|| app.active_channel.clone());
            let reason = parts.get(2).map(|s| *s);

            if let Some(ch) = channel {
                match app.channel_mgr.leave(&ch, reason) {
                    Ok(msg) => match app.client.send(&msg) {
                        Ok(()) => {
                            app.push_server_msg(format!("Leaving {}…", ch));
                            let lower = ch.to_lowercase();
                            if app.active_channel.as_deref() == Some(&lower) {
                                app.set_active_channel(None);
                            }
                        }
                        Err(e) => {
                            app.set_status(format!("part error: {}", e));
                        }
                    },
                    Err(e) => {
                        app.set_status(format!("part error: {}", e));
                    }
                }
            } else {
                app.set_status("usage: /part [#channel] [reason]".to_string());
            }
        }

        "/create" => {
            let channel = match parts.get(1) {
                Some(c) => *c,
                None => {
                    app.set_status("usage: /create #channel".to_string());
                    return;
                }
            };
            match app.channel_mgr.create_channel(channel) {
                Ok(msg) => match app.client.send(&msg) {
                    Ok(()) => {
                        app.push_server_msg(format!("Creating/joining {}…", channel));
                        app.set_active_channel(Some(channel.to_string()));
                    }
                    Err(e) => {
                        app.set_status(format!("create error: {}", e));
                    }
                },
                Err(e) => {
                    app.set_status(format!("invalid channel name: {}", e));
                }
            }
        }

        // -- User management -------------------------------------------------

        "/nick" => {
            let new_nick = match parts.get(1) {
                Some(n) => *n,
                None => { app.set_status("usage: /nick <new_nick>".to_string()); return; }
            };
            match app.user_mgr.request_nick_change(new_nick) {
                Ok(msg) => match app.client.send(&msg) {
                    Ok(()) => {
                        app.push_server_msg(format!("Requesting nick change to {}…", new_nick));
                    }
                    Err(e) => { app.set_status(format!("nick error: {}", e)); }
                },
                Err(e) => { app.set_status(format!("invalid nick: {}", e)); }
            }
        }

        "/whois" => {
            let target = match parts.get(1) {
                Some(n) => *n,
                None => { app.set_status("usage: /whois <nick>".to_string()); return; }
            };
            match app.user_mgr.build_whois(target) {
                Ok(msg) => match app.client.send(&msg) {
                    Ok(()) => {
                        app.push_server_msg(format!("Querying WHOIS for {}…", target));
                    }
                    Err(e) => { app.set_status(format!("whois error: {}", e)); }
                },
                Err(e) => { app.set_status(format!("whois error: {}", e)); }
            }
        }

        "/who" => {
            let mask = parts.get(1).copied().unwrap_or("*");
            match app.user_mgr.build_who(mask) {
                Ok(msg) => match app.client.send(&msg) {
                    Ok(()) => {
                        app.user_mgr.clear_who_results();
                        app.push_server_msg(format!("WHO {}…", mask));
                    }
                    Err(e) => { app.set_status(format!("who error: {}", e)); }
                },
                Err(e) => { app.set_status(format!("who error: {}", e)); }
            }
        }

        // /msg <nick> <text> — send a private message
        "/msg" => {
            let target = match parts.get(1) {
                Some(t) => *t,
                None => { app.set_status("usage: /msg <nick> <text>".to_string()); return; }
            };
            let text = parts.get(2).copied().unwrap_or("").to_string()
                + parts.get(3).map(|s| format!(" {}", s)).as_deref().unwrap_or("");
            if text.trim().is_empty() {
                app.set_status("usage: /msg <nick> <text>".to_string());
                return;
            }
            let msg = OutboundMessage::PrivMsg { target: target.to_string(), text: text.clone() };
            match app.client.send(&msg) {
                Ok(()) => {
                    app.open_private_chat(target);
                    app.push_channel_line(
                        &target.to_lowercase(),
                        BufferLine::Chat(ChatMessage {
                            nick: app.nick().to_string(),
                            text,
                        }),
                    );
                }
                Err(e) => { app.set_status(format!("msg error: {}", e)); }
            }
        }

        // /ns <command> — send a raw NickServ command
        // /nickserv <command> — alias
        "/ns" | "/nickserv" => {
            // Collect everything after the command word as the NickServ text.
            let text = line
                .splitn(2, ' ')
                .nth(1)
                .unwrap_or("")
                .trim()
                .to_string();
            if text.is_empty() {
                app.push_server_msg("NickServ commands:");
                app.push_server_msg("  /ns IDENTIFY <password>");
                app.push_server_msg("  /ns REGISTER <password> <email>");
                app.push_server_msg("  /ns GHOST <nick> <password>");
                app.push_server_msg("  /ns INFO <nick>");
                return;
            }
            let msg = app.user_mgr.build_nickserv(&text);
            match app.client.send(&msg) {
                Ok(()) => { app.push_server_msg(format!("[NickServ] {}", text)); }
                Err(e) => { app.set_status(format!("nickserv error: {}", e)); }
            }
        }

        "/quit" => {
            app.push_server_msg("Goodbye.");
            app.irc_stream = None;
            let _ = app.client.disconnect().await;
            app.should_quit = true;
        }

        "/help" => {
            app.push_server_msg("Commands:");
            app.push_server_msg("  /connect <server> [port] [nick]  — connect to server");
            app.push_server_msg("  /disconnect                      — disconnect from server");
            app.push_server_msg("  /join #channel                   — join a channel");
            app.push_server_msg("  /create #channel                 — create (join) a new channel");
            app.push_server_msg("  /part [#channel] [reason]        — leave a channel");
            app.push_server_msg("  /nick <new_nick>                 — change your nickname");
            app.push_server_msg("  /whois <nick>                    — show info about a user");
            app.push_server_msg("  /who [mask]                      — list users matching mask");
            app.push_server_msg("  /msg <nick> <text>               — send a private message");
            app.push_server_msg("  /ns <command>                    — send a NickServ command");
            app.push_server_msg("  /ns IDENTIFY <password>          — authenticate with NickServ");
            app.push_server_msg("  /ns REGISTER <password> <email>  — register nick with NickServ");
            app.push_server_msg("  /quit                            — exit Mercury");
            app.push_server_msg("  Ctrl-C / Ctrl-Q                  — force quit");
        }

        _ => {
            app.set_status(format!("unknown command: {}", parts[0]));
        }
    }
}

// ---------------------------------------------------------------------------
// IRC inbound message processing
// ---------------------------------------------------------------------------

async fn poll_irc_messages(app: &mut App) {
    // The stream is acquired once at connect time. If it is absent, there is
    // nothing to poll (not yet connected, or already disconnected).
    if app.irc_stream.is_none() {
        return;
    }

    // Process up to 10 messages per frame tick without blocking.
    for _ in 0..10 {
        let result = {
            let stream = app.irc_stream.as_mut().expect("checked above");
            tokio::time::timeout(Duration::from_millis(1), stream.next()).await
        };
        match result {
            Ok(Some(Ok(message))) => {
                process_irc_message(app, message);
            }
            Ok(Some(Err(e))) => {
                app.push_server_msg(format!("IRC error: {}", e));
                app.irc_stream = None;
                let _ = app.client.disconnect().await;
                return;
            }
            Ok(None) => {
                app.push_server_msg("Server closed the connection.");
                app.irc_stream = None;
                let _ = app.client.disconnect().await;
                return;
            }
            Err(_) => {
                // Timeout — no messages available this tick.
                return;
            }
        }
    }
}

fn process_irc_message(app: &mut App, message: ::irc::proto::Message) {
    match &message.command {
        Command::JOIN(channel, _, _) => {
            let joining_nick = message
                .prefix
                .as_ref()
                .and_then(|p| match p {
                    Prefix::Nickname(n, _, _) => Some(n.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            let is_ours = joining_nick.eq_ignore_ascii_case(app.user_mgr.current_nick());
            if is_ours {
                app.channel_mgr.confirm_join(channel);
                app.push_channel_line(
                    channel,
                    BufferLine::System(format!("You joined {}", channel)),
                );
                app.buffers.entry(channel.to_lowercase()).or_default();
            } else {
                app.add_channel_member(channel, MemberEntry::new(joining_nick));
                app.push_channel_line(
                    channel,
                    BufferLine::System(format!("{} joined {}", joining_nick, channel)),
                );
            }
        }

        Command::PART(channel, reason) => {
            let parting_nick = message
                .prefix
                .as_ref()
                .and_then(|p| match p {
                    Prefix::Nickname(n, _, _) => Some(n.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            let is_ours = parting_nick.is_empty()
                || parting_nick.eq_ignore_ascii_case(app.user_mgr.current_nick());
            if is_ours {
                app.channel_mgr.confirm_part(channel);
                let msg = match reason {
                    Some(r) => format!("You left {}: {}", channel, r),
                    None => format!("You left {}", channel),
                };
                app.push_server_msg(msg);
            } else {
                app.remove_channel_member(channel, parting_nick);
                let msg = match reason {
                    Some(r) => format!("{} left {}: {}", parting_nick, channel, r),
                    None => format!("{} left {}", parting_nick, channel),
                };
                app.push_channel_line(channel, BufferLine::System(msg));
            }
        }

        Command::PRIVMSG(target, text) => {
            let nick = message
                .prefix
                .as_ref()
                .and_then(|p| match p {
                    Prefix::Nickname(n, _, _) => Some(n.as_str()),
                    _ => None,
                })
                .unwrap_or("?");

            if target.starts_with('#') || target.starts_with('&') {
                app.push_channel_line(
                    target,
                    BufferLine::Chat(ChatMessage {
                        nick: nick.to_string(),
                        text: text.clone(),
                    }),
                );
            } else {
                // Private message directed at us — open a PM buffer.
                app.open_private_chat(nick);
                app.push_channel_line(
                    nick,
                    BufferLine::Chat(ChatMessage {
                        nick: nick.to_string(),
                        text: text.clone(),
                    }),
                );
            }
        }

        Command::NOTICE(target, text) => {
            let from_nick = message
                .prefix
                .as_ref()
                .and_then(|p| match p {
                    Prefix::Nickname(n, _, _) => Some(n.as_str()),
                    _ => None,
                })
                .unwrap_or("");

            // Detect NickServ notices and update auth status.
            if from_nick.eq_ignore_ascii_case("NickServ") {
                let lower = text.to_lowercase();
                if lower.contains("password accepted")
                    || lower.contains("you are now identified")
                    || lower.contains("you are already identified")
                {
                    app.user_mgr.set_nickserv_status(NickServStatus::Authenticated);
                } else if lower.contains("this nickname is registered")
                    || lower.contains("please choose a different")
                {
                    app.user_mgr.set_nickserv_status(NickServStatus::Unauthenticated);
                }
            }

            // Display the notice.
            let display = format!("-{}-  {}", from_nick, text);
            if target.starts_with('#') || target.starts_with('&') {
                app.push_channel_line(target, BufferLine::System(display));
            } else {
                app.push_server_msg(display);
            }
        }

        Command::PING(s, _) => {
            let _ = app.client.send(&OutboundMessage::Pong { server: s.clone() });
        }

        // NICK echo — either our own nick change or another user's.
        Command::NICK(new_nick) => {
            let old_nick = message
                .prefix
                .as_ref()
                .and_then(|p| match p {
                    Prefix::Nickname(n, _, _) => Some(n.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            // Detect whether this is our own nick change before mutating state.
            let is_ours = old_nick.eq_ignore_ascii_case(app.user_mgr.current_nick())
                || app.user_mgr.pending_nick()
                    .map(|p| p.eq_ignore_ascii_case(new_nick))
                    .unwrap_or(false);
            app.user_mgr.confirm_nick_change(&old_nick, new_nick);
            app.rename_channel_member(&old_nick, new_nick);
            if is_ours {
                app.push_server_msg(format!("You are now known as {}", new_nick));
                app.set_status(format!("nick: {}", new_nick));
            } else {
                app.push_server_msg(format!("{} is now known as {}", old_nick, new_nick));
            }
        }

        Command::Response(resp, args) => {
            match resp {
                Response::RPL_WELCOME => {
                    if let Some(text) = args.last() {
                        app.push_server_msg(format!("* {}", text));
                    }
                }
                Response::RPL_MOTDSTART | Response::RPL_MOTD | Response::RPL_ENDOFMOTD => {
                    if let Some(text) = args.last() {
                        app.push_server_msg(text.as_str());
                    }
                }
                Response::RPL_NAMREPLY => {
                    // args: [me, channel_type, channel, "nick1 @nick2 +nick3"]
                    if args.len() >= 4 {
                        let channel = args[2].clone();
                        app.channel_mgr.confirm_join(&channel);
                        if let Some(names) = args.last() {
                            let members: Vec<MemberEntry> = names
                                .split_whitespace()
                                .map(|token| {
                                    if let Some(nick) = token.strip_prefix('@') {
                                        MemberEntry::new(nick).op()
                                    } else if let Some(nick) = token.strip_prefix('+') {
                                        MemberEntry::new(nick).voiced()
                                    } else {
                                        MemberEntry::new(token)
                                    }
                                })
                                .collect();
                            app.set_channel_members(&channel, members);
                        }
                    }
                }

                // -- WHOIS replies -------------------------------------------
                // 311: <nick> <user> <host> * :<realname>
                Response::RPL_WHOISUSER => {
                    if args.len() >= 4 {
                        let nick = &args[1];
                        let user = &args[2];
                        let host = &args[3];
                        let realname = args.last().map(|s| s.as_str()).unwrap_or("");
                        app.user_mgr.handle_whois_user(nick, user, host, realname);
                        app.push_server_msg(format!(
                            "[whois] {} ({}@{}) — {}",
                            nick, user, host, realname
                        ));
                    }
                }
                // 312: <nick> <server> :<server info>
                Response::RPL_WHOISSERVER => {
                    if args.len() >= 3 {
                        let nick = &args[1];
                        let server = &args[2];
                        let info = args.last().map(|s| s.as_str()).unwrap_or("");
                        app.user_mgr.handle_whois_server(nick, server, info);
                        app.push_server_msg(format!("[whois] {} via {} ({})", nick, server, info));
                    }
                }
                // 319: <nick> :<channel list>
                Response::RPL_WHOISCHANNELS => {
                    if args.len() >= 2 {
                        let nick = &args[1];
                        let channels = args.last().map(|s| s.as_str()).unwrap_or("");
                        app.user_mgr.handle_whois_channels(nick, channels);
                        app.push_server_msg(format!("[whois] {} channels: {}", nick, channels));
                    }
                }
                // 318: end of WHOIS
                Response::RPL_ENDOFWHOIS => {}

                // -- WHO replies ---------------------------------------------
                // 352: <channel> <user> <host> <server> <nick> <flags> :<hops> <realname>
                Response::RPL_WHOREPLY => {
                    // args: [me, channel, user, host, server, nick, flags, "hops realname"]
                    if args.len() >= 7 {
                        let nick = &args[5];
                        let user = &args[2];
                        let host = &args[3];
                        let server = &args[4];
                        let flags = &args[6];
                        let realname = args
                            .last()
                            .and_then(|s| s.splitn(2, ' ').nth(1))
                            .unwrap_or("");
                        app.user_mgr.handle_who_reply(nick, user, host, server, flags, realname);
                        let away = if flags.starts_with('G') { " (away)" } else { "" };
                        app.push_server_msg(format!(
                            "[who] {}{}  {}@{}  {}",
                            nick, away, user, host, realname
                        ));
                    }
                }
                // 315: end of WHO list
                Response::RPL_ENDOFWHO => {}

                // -- Nick errors ---------------------------------------------
                Response::ERR_NICKNAMEINUSE => {
                    let attempted = args.get(1).map(|s| s.as_str()).unwrap_or("?");
                    app.push_server_msg(format!("Error: nickname '{}' is already in use.", attempted));
                    app.set_status("nick in use".to_string());
                    // Roll back any pending nick change.
                    let current = app.user_mgr.current_nick().to_string();
                    let _ = app.user_mgr.request_nick_change(&current);
                }
                Response::ERR_ERRONEOUSNICKNAME => {
                    let attempted = args.get(1).map(|s| s.as_str()).unwrap_or("?");
                    app.push_server_msg(format!("Error: '{}' is not a valid nickname.", attempted));
                    app.set_status("erroneous nick".to_string());
                }
                Response::ERR_NONICKNAMEGIVEN => {
                    app.push_server_msg("Error: no nickname given.");
                }

                Response::ERR_NOSUCHNICK => {
                    let target = args.get(1).map(|s| s.as_str()).unwrap_or("?");
                    app.push_server_msg(format!("Error: no such nick/channel '{}'.", target));
                }
                _ => {
                    let text = args.join(" ");
                    if !text.is_empty() {
                        app.push_server_msg(format!("[{:?}] {}", resp, text));
                    }
                }
            }
        }

        _ => {}
    }
}
