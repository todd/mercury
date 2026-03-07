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
use crate::tui::app::{App, BufferLine, ChatMessage};
use crate::tui::ui::draw;

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
            let _ = app.client.disconnect().await;
        }
        KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
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

            match app.client.connect().await {
                Ok(()) => {
                    app.push_server_msg(format!("Connected to {}:{}", server, port));
                    app.set_status(format!("connected to {}", server));
                }
                Err(e) => {
                    app.push_server_msg(format!("Connection failed: {}", e));
                    app.set_status(format!("connection failed: {}", e));
                }
            }
        }

        "/disconnect" => match app.client.disconnect().await {
            Ok(()) => {
                app.push_server_msg("Disconnected.");
                app.set_status("disconnected".to_string());
            }
            Err(e) => {
                app.set_status(format!("disconnect error: {}", e));
            }
        },

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

        "/quit" => {
            app.push_server_msg("Goodbye.");
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
    let mut stream = match app.client.stream() {
        Ok(s) => s,
        Err(_) => return,
    };

    // Process up to 10 messages per frame tick without blocking
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_millis(1), stream.next()).await {
            Ok(Some(Ok(message))) => {
                process_irc_message(app, message);
            }
            Ok(Some(Err(e))) => {
                app.push_server_msg(format!("IRC error: {}", e));
                let _ = app.client.disconnect().await;
                return;
            }
            Ok(None) => {
                app.push_server_msg("Server closed the connection.");
                let _ = app.client.disconnect().await;
                return;
            }
            Err(_) => {
                // Timeout — no messages available, exit polling for this tick
                return;
            }
        }
    }
}

fn process_irc_message(app: &mut App, message: ::irc::proto::Message) {
    match &message.command {
        Command::JOIN(channel, _, _) => {
            app.channel_mgr.confirm_join(channel);
            app.push_channel_line(
                channel,
                BufferLine::System(format!("You joined {}", channel)),
            );
            app.buffers.entry(channel.to_lowercase()).or_default();
        }

        Command::PART(channel, reason) => {
            app.channel_mgr.confirm_part(channel);
            let msg = match reason {
                Some(r) => format!("You left {}: {}", channel, r),
                None => format!("You left {}", channel),
            };
            app.push_server_msg(msg);
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
                app.push_server_msg(format!("[PM from {}] {}", nick, text));
            }
        }

        Command::PING(s, _) => {
            let _ = app.client.send(&OutboundMessage::Pong { server: s.clone() });
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
                    if args.len() >= 3 {
                        let channel = args[2].clone();
                        app.channel_mgr.confirm_join(&channel);
                        if let Some(names) = args.last() {
                            app.push_channel_line(
                                &channel,
                                BufferLine::System(format!("Members: {}", names)),
                            );
                        }
                    }
                }
                Response::ERR_NICKNAMEINUSE => {
                    app.push_server_msg("Error: nickname already in use.");
                    app.set_status("nick in use".to_string());
                }
                Response::ERR_NOSUCHNICK => {
                    app.push_server_msg("Error: no such nick/channel.");
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
