/// User management: local nick tracking, nick validation, NickServ
/// authentication status, and caching of WHOIS / WHO results received from
/// the server.
use crate::error::{MercuryError, Result};
use crate::irc::message::OutboundMessage;

// ---------------------------------------------------------------------------
// NickServ authentication status
// ---------------------------------------------------------------------------

/// The authentication state of the local user with NickServ (or equivalent
/// network services).
///
/// Transitions:
///   connect / own nick change  →  Unregistered
///   server: "This nickname is registered …"  →  Unauthenticated
///   server: "Password accepted" / "now identified"  →  Authenticated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NickServStatus {
    /// Nick is not registered with services, or no services are present.
    #[default]
    Unregistered,
    /// Nick is registered but we have not yet successfully identified.
    Unauthenticated,
    /// Successfully identified with services.
    Authenticated,
}

// ---------------------------------------------------------------------------
// Nick validation
// ---------------------------------------------------------------------------

/// Maximum nick length per RFC 2812 §2.3.1.
const MAX_NICK_LEN: usize = 30;

/// Returns `true` if `nick` is a valid IRC nickname.
///
/// Rules (RFC 2812 §2.3.1):
/// - 1–30 characters
/// - First character: letter or one of `[]\\^_{|}`
/// - Remaining characters: letter, digit, `-`, or one of `[]\\^_{|}`
pub fn is_valid_nick(nick: &str) -> bool {
    if nick.is_empty() || nick.len() > MAX_NICK_LEN {
        return false;
    }
    let mut chars = nick.chars();
    let first = chars.next().unwrap();
    if !is_nick_start(first) {
        return false;
    }
    chars.all(is_nick_rest)
}

fn is_nick_start(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '[' | ']' | '\\' | '^' | '_' | '{' | '|' | '}')
}

fn is_nick_rest(c: char) -> bool {
    is_nick_start(c) || c.is_ascii_digit() || c == '-'
}

// ---------------------------------------------------------------------------
// Whois result
// ---------------------------------------------------------------------------

/// Information about a remote user, populated from server WHOIS replies.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WhoisInfo {
    /// The nick that was queried.
    pub nick: String,
    /// Reported username (ident).
    pub username: String,
    /// Reported hostname.
    pub host: String,
    /// Reported real name.
    pub realname: String,
    /// Server the user is connected to.
    pub server: String,
    /// Server description / info string.
    pub server_info: String,
    /// Channels the user is in (from RPL_WHOISCHANNELS).
    pub channels: Vec<String>,
    /// Whether the user is identified to services (IRCv3 `account-notify` /
    /// RPL_WHOISACCOUNT / RPL_WHOISREGNICK).
    pub is_identified: bool,
    /// NickServ account name, if reported.
    pub account: Option<String>,
}

// ---------------------------------------------------------------------------
// Who result
// ---------------------------------------------------------------------------

/// A single entry from a WHO reply (RPL_WHOREPLY / RPL_WHOSPCRPL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhoEntry {
    pub nick: String,
    pub username: String,
    pub host: String,
    pub server: String,
    pub realname: String,
    pub away: bool,
}

// ---------------------------------------------------------------------------
// UserManager
// ---------------------------------------------------------------------------

/// Tracks the local user's nickname, pending nick changes, NickServ auth
/// status, and cached WHOIS / WHO results from the server.
pub struct UserManager {
    /// The nick we are currently using (confirmed by server).
    current_nick: String,
    /// A nick change request that has been sent but not yet confirmed by the
    /// server (i.e. we haven't received the NICK echo back yet).
    pending_nick: Option<String>,
    /// Current NickServ authentication status.
    nickserv_status: NickServStatus,
    /// Cached WHOIS results, keyed by lowercase nick.
    whois_cache: std::collections::HashMap<String, WhoisInfo>,
    /// Most recent WHO results.
    who_results: Vec<WhoEntry>,
}

impl UserManager {
    /// Create a new `UserManager` with the given starting nick.
    ///
    /// Returns `Err` if `nick` is not a valid IRC nickname.
    pub fn new(nick: &str) -> Result<Self> {
        if !is_valid_nick(nick) {
            return Err(MercuryError::InvalidNick {
                nick: nick.to_string(),
                reason: "must be 1–30 chars, starting with a letter or []\\^_{|}".to_string(),
            });
        }
        Ok(Self {
            current_nick: nick.to_string(),
            pending_nick: None,
            nickserv_status: NickServStatus::Unregistered,
            whois_cache: std::collections::HashMap::new(),
            who_results: Vec::new(),
        })
    }

    // -- Nick management -----------------------------------------------------

    /// The nick currently in use (server-confirmed).
    pub fn current_nick(&self) -> &str {
        &self.current_nick
    }

    /// A pending nick that has been requested but not yet confirmed.
    pub fn pending_nick(&self) -> Option<&str> {
        self.pending_nick.as_deref()
    }

    // -- NickServ status -----------------------------------------------------

    /// Current NickServ authentication status.
    pub fn nickserv_status(&self) -> NickServStatus {
        self.nickserv_status
    }

    /// Update the NickServ authentication status.
    pub fn set_nickserv_status(&mut self, status: NickServStatus) {
        self.nickserv_status = status;
    }

    /// Build a `NICK` message to request a nick change, and record the
    /// pending nick. Returns `Err` if `new_nick` is invalid.
    pub fn request_nick_change(&mut self, new_nick: &str) -> Result<OutboundMessage> {
        if !is_valid_nick(new_nick) {
            return Err(MercuryError::InvalidNick {
                nick: new_nick.to_string(),
                reason: "must be 1–30 chars, starting with a letter or []\\^_{|}".to_string(),
            });
        }
        self.pending_nick = Some(new_nick.to_string());
        Ok(OutboundMessage::Nick {
            new_nick: new_nick.to_string(),
        })
    }

    /// Called when the server echoes a NICK change (either ours or another
    /// user's). If it matches our pending nick, we update `current_nick`.
    pub fn confirm_nick_change(&mut self, old_nick: &str, new_nick: &str) {
        // Invalidate any cached whois for the old nick.
        self.whois_cache.remove(&old_nick.to_lowercase());

        if self
            .pending_nick
            .as_deref()
            .map(|p| p.eq_ignore_ascii_case(new_nick))
            .unwrap_or(false)
            || self.current_nick.eq_ignore_ascii_case(old_nick)
        {
            self.current_nick = new_nick.to_string();
            self.pending_nick = None;
            // A nick change means our authentication no longer applies.
            self.nickserv_status = NickServStatus::Unregistered;
        }
    }

    // -- WHOIS ---------------------------------------------------------------

    /// Build a `WHOIS` message for the given nick.
    pub fn build_whois(&self, nick: &str) -> Result<OutboundMessage> {
        if nick.is_empty() {
            return Err(MercuryError::InvalidNick {
                nick: nick.to_string(),
                reason: "nick must not be empty".to_string(),
            });
        }
        Ok(OutboundMessage::Whois {
            nick: nick.to_string(),
        })
    }

    /// Update the whois cache with information from RPL_WHOISUSER (311).
    /// Fields: nick, username, host, realname.
    pub fn handle_whois_user(&mut self, nick: &str, username: &str, host: &str, realname: &str) {
        let entry = self
            .whois_cache
            .entry(nick.to_lowercase())
            .or_insert_with(|| WhoisInfo {
                nick: nick.to_string(),
                ..Default::default()
            });
        entry.username = username.to_string();
        entry.host = host.to_string();
        entry.realname = realname.to_string();
    }

    /// Update the whois cache with server information from RPL_WHOISSERVER (312).
    pub fn handle_whois_server(&mut self, nick: &str, server: &str, server_info: &str) {
        let entry = self
            .whois_cache
            .entry(nick.to_lowercase())
            .or_insert_with(|| WhoisInfo {
                nick: nick.to_string(),
                ..Default::default()
            });
        entry.server = server.to_string();
        entry.server_info = server_info.to_string();
    }

    /// Update the whois cache with channel membership from RPL_WHOISCHANNELS (319).
    pub fn handle_whois_channels(&mut self, nick: &str, channels_str: &str) {
        let entry = self
            .whois_cache
            .entry(nick.to_lowercase())
            .or_insert_with(|| WhoisInfo {
                nick: nick.to_string(),
                ..Default::default()
            });
        entry.channels = channels_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
    }

    /// Update the whois cache with account information from RPL_WHOISACCOUNT (330)
    /// or RPL_WHOISREGNICK (307).
    pub fn handle_whois_account(&mut self, nick: &str, account: &str) {
        let entry = self
            .whois_cache
            .entry(nick.to_lowercase())
            .or_insert_with(|| WhoisInfo {
                nick: nick.to_string(),
                ..Default::default()
            });
        entry.is_identified = true;
        entry.account = Some(account.to_string());
    }

    /// Retrieve a cached WHOIS result for `nick`, if available.
    pub fn whois_info(&self, nick: &str) -> Option<&WhoisInfo> {
        self.whois_cache.get(&nick.to_lowercase())
    }

    // -- WHO -----------------------------------------------------------------

    /// Build a `WHO` message for the given mask (nick, channel, or wildcard).
    pub fn build_who(&self, mask: &str) -> Result<OutboundMessage> {
        if mask.is_empty() {
            return Err(MercuryError::InvalidNick {
                nick: mask.to_string(),
                reason: "WHO mask must not be empty".to_string(),
            });
        }
        Ok(OutboundMessage::Who {
            mask: mask.to_string(),
        })
    }

    /// Record a single WHO reply entry (RPL_WHOREPLY 352).
    /// Replaces any previous entry for the same nick.
    pub fn handle_who_reply(
        &mut self,
        nick: &str,
        username: &str,
        host: &str,
        server: &str,
        flags: &str,
        realname: &str,
    ) {
        // Remove stale entry for this nick if present.
        self.who_results
            .retain(|e| !e.nick.eq_ignore_ascii_case(nick));
        self.who_results.push(WhoEntry {
            nick: nick.to_string(),
            username: username.to_string(),
            host: host.to_string(),
            server: server.to_string(),
            // 'H' = here (not away), 'G' = gone (away)
            away: flags.starts_with('G'),
            realname: realname.to_string(),
        });
    }

    /// Return all stored WHO results, in the order they were received.
    pub fn who_results(&self) -> &[WhoEntry] {
        &self.who_results
    }

    /// Clear stored WHO results (call before issuing a new WHO).
    pub fn clear_who_results(&mut self) {
        self.who_results.clear();
    }

    // -- NickServ / services -------------------------------------------------

    /// Build a `PRIVMSG NickServ :IDENTIFY <password>` message.
    pub fn build_identify(&self, password: &str) -> OutboundMessage {
        OutboundMessage::NickServ {
            text: format!("IDENTIFY {}", password),
        }
    }

    /// Build a `PRIVMSG NickServ :REGISTER <password> <email>` message.
    pub fn build_register(&self, password: &str, email: &str) -> OutboundMessage {
        OutboundMessage::NickServ {
            text: format!("REGISTER {} {}", password, email),
        }
    }

    /// Build an arbitrary `PRIVMSG NickServ :<text>` message.
    pub fn build_nickserv(&self, text: &str) -> OutboundMessage {
        OutboundMessage::NickServ {
            text: text.to_string(),
        }
    }
}
