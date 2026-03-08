/// Application state — the single source of truth for the TUI.
use std::collections::HashMap;

use crate::irc::channel::ChannelManager;
use crate::irc::client::{ClientConfig, ClientState, IrcClient};
use crate::irc::message::OutboundMessage;
use crate::irc::user::{NickServStatus, UserManager};

/// Type alias for the IRC inbound stream. Stored in `App` so it is acquired
/// exactly once per connection (the `irc` crate allows only one live stream
/// per `Client` instance).
pub type IrcStream = ::irc::client::ClientStream;

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
// Channel member
// ---------------------------------------------------------------------------

/// A single member of a channel as reported by RPL_NAMREPLY / JOIN / PART.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberEntry {
    /// Display nick (original case).
    pub nick: String,
    pub is_op: bool,
    pub is_voiced: bool,
}

impl MemberEntry {
    pub fn new(nick: impl Into<String>) -> Self {
        Self {
            nick: nick.into(),
            is_op: false,
            is_voiced: false,
        }
    }
    pub fn op(mut self) -> Self {
        self.is_op = true;
        self
    }
    pub fn voiced(mut self) -> Self {
        self.is_voiced = true;
        self
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Top-level application state.
pub struct App {
    pub client: IrcClient,
    pub channel_mgr: ChannelManager,
    pub user_mgr: UserManager,

    /// Inbound IRC message stream. Acquired once on connect; `None` when
    /// disconnected. The `irc` crate permits only a single live stream per
    /// `Client`, so we must not call `client.stream()` more than once.
    pub irc_stream: Option<IrcStream>,

    /// Per-channel message history. Key is lowercase channel name.
    pub buffers: HashMap<String, Vec<BufferLine>>,

    /// The currently-focused channel (None = server buffer).
    pub active_channel: Option<String>,

    /// Members of each joined channel.  Key = lowercase channel name.
    /// Each vec is kept sorted: ops first (alpha), then voiced (alpha), then
    /// regular (alpha).
    pub channel_members: HashMap<String, Vec<MemberEntry>>,

    /// Open private-message conversations, stored as lowercase nicks in
    /// alphabetical order.
    pub private_chats: Vec<String>,

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
        // UserManager::new panics on invalid nick; ClientConfig::new already
        // validated it, so this is safe.
        let user_mgr = UserManager::new(nick).expect("nick validated by ClientConfig");
        Self {
            client: IrcClient::new(config),
            channel_mgr: ChannelManager::new(),
            user_mgr,
            irc_stream: None,
            buffers: HashMap::new(),
            active_channel: None,
            channel_members: HashMap::new(),
            private_chats: Vec::new(),
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

    /// Nick of the current user (server-confirmed via UserManager).
    pub fn nick(&self) -> &str {
        self.user_mgr.current_nick()
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

    /// NickServ authentication status of the local user.
    pub fn nickserv_status(&self) -> NickServStatus {
        self.user_mgr.nickserv_status()
    }

    // -- Channel member list -------------------------------------------------

    /// Replace the member list for a channel with a freshly parsed set.
    /// Keeps the list sorted: ops → voiced → regular, each group alphabetically.
    pub fn set_channel_members(&mut self, channel: &str, mut members: Vec<MemberEntry>) {
        Self::sort_members(&mut members);
        self.channel_members.insert(channel.to_lowercase(), members);
    }

    /// Add a member to a channel's list (if not already present).
    pub fn add_channel_member(&mut self, channel: &str, entry: MemberEntry) {
        let key = channel.to_lowercase();
        let list = self.channel_members.entry(key).or_default();
        let nick_lower = entry.nick.to_lowercase();
        if !list.iter().any(|m| m.nick.to_lowercase() == nick_lower) {
            list.push(entry);
            Self::sort_members(list);
        }
    }

    /// Remove a member from a channel's list by nick (case-insensitive).
    pub fn remove_channel_member(&mut self, channel: &str, nick: &str) {
        let key = channel.to_lowercase();
        let nick_lower = nick.to_lowercase();
        if let Some(list) = self.channel_members.get_mut(&key) {
            list.retain(|m| m.nick.to_lowercase() != nick_lower);
        }
    }

    /// Rename a member across all channel lists (e.g. on a NICK change).
    pub fn rename_channel_member(&mut self, old_nick: &str, new_nick: &str) {
        let old_lower = old_nick.to_lowercase();
        for list in self.channel_members.values_mut() {
            if let Some(m) = list.iter_mut().find(|m| m.nick.to_lowercase() == old_lower) {
                m.nick = new_nick.to_string();
            }
            Self::sort_members(list);
        }
    }

    /// Members of the currently active channel, or an empty slice.
    pub fn active_channel_members(&self) -> &[MemberEntry] {
        self.active_channel
            .as_deref()
            .and_then(|ch| self.channel_members.get(ch))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    fn sort_members(list: &mut Vec<MemberEntry>) {
        list.sort_by(|a, b| {
            // Ops before voiced before regular.
            let rank = |m: &MemberEntry| {
                if m.is_op {
                    0u8
                } else if m.is_voiced {
                    1
                } else {
                    2
                }
            };
            rank(a)
                .cmp(&rank(b))
                .then_with(|| a.nick.to_lowercase().cmp(&b.nick.to_lowercase()))
        });
    }

    // -- Private message tracking --------------------------------------------

    /// Ensure a nick has an open PM conversation buffer (idempotent).
    pub fn open_private_chat(&mut self, nick: &str) {
        let key = nick.to_lowercase();
        // Create a buffer entry if needed.
        self.buffers.entry(key.clone()).or_default();
        if !self.private_chats.contains(&key) {
            self.private_chats.push(key);
            self.private_chats.sort();
        }
    }

    /// Whether the active buffer is a private-message conversation.
    pub fn active_is_pm(&self) -> bool {
        match &self.active_channel {
            Some(ch) => self.private_chats.contains(ch),
            None => false,
        }
    }

    /// Whether the active buffer is a joined channel (not server, not PM).
    pub fn active_is_channel(&self) -> bool {
        match &self.active_channel {
            Some(ch) => !self.private_chats.contains(ch),
            None => false,
        }
    }

    // -- Channel navigation --------------------------------------------------

    /// The ordered navigation list: [None (server), channels..., pm_nicks...].
    /// Each entry is `None` (server) or `Some(lowercase_name)`.
    fn nav_list(&self) -> Vec<Option<String>> {
        let mut list: Vec<Option<String>> = vec![None];
        let mut channels = self.sorted_joined_channels();
        channels.sort();
        for ch in channels {
            list.push(Some(ch));
        }
        for pm in &self.private_chats {
            list.push(Some(pm.clone()));
        }
        list
    }

    /// Switch to the next entry in the navigation list (wraps around).
    pub fn next_channel(&mut self) {
        let nav = self.nav_list();
        if nav.len() <= 1 {
            return;
        }
        let current_idx = nav
            .iter()
            .position(|e| e.as_deref() == self.active_channel.as_deref());
        let next_idx = match current_idx {
            Some(i) => (i + 1) % nav.len(),
            None => 0,
        };
        self.active_channel = nav[next_idx].clone();
    }

    /// Switch to the previous entry in the navigation list (wraps around).
    pub fn prev_channel(&mut self) {
        let nav = self.nav_list();
        if nav.len() <= 1 {
            return;
        }
        let current_idx = nav
            .iter()
            .position(|e| e.as_deref() == self.active_channel.as_deref());
        let prev_idx = match current_idx {
            Some(0) | None => nav.len() - 1,
            Some(i) => i - 1,
        };
        self.active_channel = nav[prev_idx].clone();
    }

    /// Handle a raw outbound message — used when the channel manager produces
    /// a message that must also be sent to the server.
    pub fn queue_outbound(&self, msg: OutboundMessage) -> OutboundMessage {
        // For now just pass-through; the TUI event loop calls client.send().
        msg
    }
}
