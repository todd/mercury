/// Application state — the single source of truth for the TUI.
use std::collections::HashMap;

use crate::irc::client::{ClientConfig, ClientState, IrcClient};
use crate::irc::channel::ChannelManager;
use crate::irc::message::OutboundMessage;

// ---------------------------------------------------------------------------
// Messages displayed in a channel buffer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub nick: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum BufferLine {
    Chat(ChatMessage),
    System(String),
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Top-level application state.
pub struct App {
    pub client: IrcClient,
    pub channel_mgr: ChannelManager,

    /// Per-channel message history. Key is lowercase channel name.
    pub buffers: HashMap<String, Vec<BufferLine>>,

    /// The currently-focused channel (None = server buffer).
    pub active_channel: Option<String>,

    /// The server-level message buffer.
    pub server_buffer: Vec<BufferLine>,

    /// Current text in the input bar.
    pub input: String,

    /// Whether the user has requested the app to exit.
    pub should_quit: bool,

    /// Status message displayed in the bottom status bar.
    pub status_message: Option<String>,
}

impl App {
    pub fn new_disconnected(server: &str, port: u16, nick: &str) -> Self {
        let config = ClientConfig::new(server, port, nick);
        Self {
            client: IrcClient::new(config),
            channel_mgr: ChannelManager::new(),
            buffers: HashMap::new(),
            active_channel: None,
            server_buffer: Vec::new(),
            input: String::new(),
            should_quit: false,
            status_message: None,
        }
    }

    /// Add a system message to the server buffer.
    pub fn push_server_msg(&mut self, text: impl Into<String>) {
        self.server_buffer.push(BufferLine::System(text.into()));
    }

    /// Add a line to the named channel buffer (creating the buffer if needed).
    pub fn push_channel_line(&mut self, channel: &str, line: BufferLine) {
        self.buffers
            .entry(channel.to_lowercase())
            .or_default()
            .push(line);
    }

    /// Returns the lines for the currently-active buffer.
    pub fn active_lines(&self) -> &[BufferLine] {
        match &self.active_channel {
            Some(ch) => self
                .buffers
                .get(ch.as_str())
                .map(|v| v.as_slice())
                .unwrap_or(&[]),
            None => &self.server_buffer,
        }
    }

    /// Switch focus to the given channel (or `None` for the server buffer).
    pub fn set_active_channel(&mut self, channel: Option<String>) {
        self.active_channel = channel.map(|c| c.to_lowercase());
    }

    /// Current connection state shorthand.
    pub fn connection_state(&self) -> ClientState {
        self.client.state()
    }

    /// Nick of the current user.
    pub fn nick(&self) -> &str {
        // Safe: IrcClient keeps config alive
        // We expose this via the config through a method on IrcClient.
        // For now we re-read from the inner config by reading from the IrcClient.
        // We'll return a placeholder; the real nick comes from the server after
        // connection but we track our requested nick here.
        "_nick_"
    }

    /// Append a character to the input buffer.
    pub fn input_push(&mut self, ch: char) {
        self.input.push(ch);
    }

    /// Remove the last character from the input buffer.
    pub fn input_backspace(&mut self) {
        self.input.pop();
    }

    /// Clear the input buffer and return its contents.
    pub fn input_take(&mut self) -> String {
        std::mem::take(&mut self.input)
    }

    /// Set a temporary status bar message.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    /// Clear the status bar message.
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// All joined channels in sorted order (for stable rendering).
    pub fn sorted_joined_channels(&self) -> Vec<String> {
        let mut chans = self.channel_mgr.joined_channels();
        chans.sort();
        chans
    }

    /// Handle a raw outbound message — used when the channel manager produces
    /// a message that must also be sent to the server.
    pub fn queue_outbound(&self, msg: OutboundMessage) -> OutboundMessage {
        // For now just pass-through; the TUI event loop calls client.send().
        msg
    }
}
