/// Channel management: state tracking and message generation for
/// create/join/leave operations.
use std::collections::HashMap;

use crate::error::{MercuryError, Result};
use crate::irc::message::OutboundMessage;

/// Maximum channel name length per RFC 2812 / IRCv3.
const MAX_CHANNEL_NAME_LEN: usize = 200;

/// The lifecycle state of a single channel from the client's perspective.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelState {
    /// JOIN sent to server, awaiting confirmation (RPL_NAMREPLY / JOIN echo).
    Joining,
    /// Server confirmed membership.
    Joined,
    /// PART sent to server, awaiting echo.
    Parting,
}

/// Manages the set of channels a client is currently in or transitioning through.
pub struct ChannelManager {
    /// Map of lowercase channel name → current state.
    channels: HashMap<String, ChannelState>,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Channel name validation
    // -----------------------------------------------------------------------

    /// Returns `true` if `name` is a valid IRC channel name per RFC 2812 §1.3.
    ///
    /// Rules:
    /// - Must start with `#` or `&`
    /// - Must not contain NUL, BEL, CR, LF, space, or comma
    /// - Length between 2 and 200 characters
    pub fn is_valid_channel_name(name: &str) -> bool {
        if name.len() < 2 || name.len() > MAX_CHANNEL_NAME_LEN {
            return false;
        }
        let first = name.chars().next().unwrap();
        if first != '#' && first != '&' {
            return false;
        }
        // Forbidden characters
        for ch in name.chars() {
            if matches!(ch, '\0' | '\x07' | '\r' | '\n' | ' ' | ',') {
                return false;
            }
        }
        true
    }

    // -----------------------------------------------------------------------
    // Feature 2: Create channel
    // -----------------------------------------------------------------------

    /// Build a `JOIN` message for a new channel (IRC channel creation == first JOIN).
    ///
    /// Returns an `OutboundMessage::Join` that the caller should send via `IrcClient`.
    /// Local state transitions to `Joining`.
    pub fn create_channel(&mut self, name: &str) -> Result<OutboundMessage> {
        self.create_channel_with_key_opt(name, None)
    }

    /// Build a `JOIN` message for a new channel with an optional key.
    pub fn create_channel_with_key(&mut self, name: &str, key: &str) -> Result<OutboundMessage> {
        self.create_channel_with_key_opt(name, Some(key.to_string()))
    }

    fn create_channel_with_key_opt(
        &mut self,
        name: &str,
        key: Option<String>,
    ) -> Result<OutboundMessage> {
        if !Self::is_valid_channel_name(name) {
            return Err(MercuryError::InvalidChannelName {
                name: name.to_string(),
                reason: "must start with # or & and contain no forbidden characters".to_string(),
            });
        }
        let lower = name.to_lowercase();
        // Mark as Joining unless already tracked (idempotent)
        self.channels.entry(lower).or_insert(ChannelState::Joining);
        Ok(OutboundMessage::Join {
            channel: name.to_string(),
            key,
        })
    }

    // -----------------------------------------------------------------------
    // Feature 3: Join channel
    // -----------------------------------------------------------------------

    /// Build a `JOIN` message to join an existing channel.
    ///
    /// If already `Joined`, still returns `Ok` (idempotent — caller can re-send).
    pub fn join(&mut self, name: &str) -> Result<OutboundMessage> {
        if !Self::is_valid_channel_name(name) {
            return Err(MercuryError::InvalidChannelName {
                name: name.to_string(),
                reason: "must start with # or & and contain no forbidden characters".to_string(),
            });
        }
        let lower = name.to_lowercase();
        // If already Joined, keep state but still allow sending JOIN (server will echo)
        self.channels.entry(lower).or_insert(ChannelState::Joining);
        Ok(OutboundMessage::Join {
            channel: name.to_string(),
            key: None,
        })
    }

    /// Called when the server echoes our JOIN (confirms membership).
    pub fn confirm_join(&mut self, name: &str) {
        let lower = name.to_lowercase();
        self.channels.insert(lower, ChannelState::Joined);
    }

    // -----------------------------------------------------------------------
    // Feature 3: Leave channel
    // -----------------------------------------------------------------------

    /// Build a `PART` message for the given channel.
    ///
    /// Returns an error if we are not currently in that channel.
    pub fn leave(&mut self, name: &str, reason: Option<&str>) -> Result<OutboundMessage> {
        let lower = name.to_lowercase();
        match self.channels.get(&lower) {
            None | Some(ChannelState::Joining) => {
                // Treat Joining-but-not-confirmed as "not in channel"
                if !self.channels.contains_key(&lower) {
                    return Err(MercuryError::NotInChannel {
                        channel: name.to_string(),
                    });
                }
                // If we're mid-join we still allow PART (server handles it)
            }
            Some(ChannelState::Joined) | Some(ChannelState::Parting) => {}
        }

        self.channels.insert(lower, ChannelState::Parting);
        Ok(OutboundMessage::Part {
            channel: name.to_string(),
            reason: reason.map(|r| r.to_string()),
        })
    }

    /// Called when the server echoes our PART (confirms we left).
    pub fn confirm_part(&mut self, name: &str) {
        let lower = name.to_lowercase();
        self.channels.remove(&lower);
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Returns `true` if the channel is in the `Joined` state.
    pub fn is_joined(&self, name: &str) -> bool {
        matches!(
            self.channels.get(&name.to_lowercase()),
            Some(ChannelState::Joined)
        )
    }

    /// Returns the current state of a channel, or `None` if unknown.
    pub fn channel_state(&self, name: &str) -> Option<ChannelState> {
        self.channels.get(&name.to_lowercase()).cloned()
    }

    /// Returns the list of channel names currently in `Joined` state.
    pub fn joined_channels(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter_map(|(k, v)| {
                if *v == ChannelState::Joined {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}
