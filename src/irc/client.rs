/// IRC client: connection management wrapping the `irc` crate.
use std::fmt;

use irc::client::prelude::*;
use tracing::{debug, info, warn};

use crate::error::{MercuryError, Result};
use crate::irc::message::OutboundMessage;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration required to connect to an IRC server.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    server: String,
    port: u16,
    nick: String,
    realname: String,
    username: String,
    use_tls: bool,
}

impl ClientConfig {
    /// Construct a config, panicking on invalid input. Use for tests and
    /// hardcoded defaults where values are known-good.
    pub fn new(server: &str, port: u16, nick: &str) -> Self {
        Self::try_new(server, port, nick)
            .expect("ClientConfig::new called with invalid arguments")
    }

    /// Construct with default port 6667.
    pub fn with_defaults(server: &str, nick: &str) -> Self {
        Self::new(server, 6667, nick)
    }

    /// Fallible constructor — returns `Err` on invalid input.
    pub fn try_new(server: &str, port: u16, nick: &str) -> Result<Self> {
        if server.is_empty() {
            return Err(MercuryError::InvalidChannelName {
                name: server.to_string(),
                reason: "server hostname must not be empty".to_string(),
            });
        }
        if port == 0 {
            return Err(MercuryError::InvalidChannelName {
                name: port.to_string(),
                reason: "port must be > 0".to_string(),
            });
        }
        if nick.is_empty() || nick.contains(' ') {
            return Err(MercuryError::InvalidChannelName {
                name: nick.to_string(),
                reason: "nick must be non-empty and contain no spaces".to_string(),
            });
        }
        Ok(Self {
            server: server.to_string(),
            port,
            nick: nick.to_string(),
            realname: "Mercury IRC Client".to_string(),
            username: nick.to_string(),
            use_tls: false,
        })
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn nick(&self) -> &str {
        &self.nick
    }

    /// Build the `irc` crate's `Config` from our config.
    pub(crate) fn to_irc_config(&self) -> Config {
        Config {
            server: Some(self.server.clone()),
            port: Some(self.port),
            nickname: Some(self.nick.clone()),
            realname: Some(self.realname.clone()),
            username: Some(self.username.clone()),
            use_tls: Some(self.use_tls),
            // Disable default channel joining
            channels: vec![],
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}

impl fmt::Display for ClientState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientState::Disconnected => write!(f, "Disconnected"),
            ClientState::Connecting => write!(f, "Connecting"),
            ClientState::Connected => write!(f, "Connected"),
            ClientState::Disconnecting => write!(f, "Disconnecting"),
        }
    }
}

// ---------------------------------------------------------------------------
// IrcClient
// ---------------------------------------------------------------------------

/// High-level IRC client. Wraps the `irc` crate's `Client` and manages
/// connection lifecycle and state transitions.
pub struct IrcClient {
    config: ClientConfig,
    state: ClientState,
    inner: Option<Client>,
}

impl IrcClient {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            state: ClientState::Disconnected,
            inner: None,
        }
    }

    /// Current connection state.
    pub fn state(&self) -> ClientState {
        self.state.clone()
    }

    /// The server we are (or last were) connected to, if any.
    pub fn current_server(&self) -> Option<&str> {
        match self.state {
            ClientState::Disconnected => None,
            _ => Some(self.config.server()),
        }
    }

    /// The current nick.
    pub fn nick(&self) -> &str {
        self.config.nick()
    }

    /// Connect to the configured IRC server.
    ///
    /// Transitions: `Disconnected → Connecting → Connected`
    /// On failure: reverts to `Disconnected`.
    pub async fn connect(&mut self) -> Result<()> {
        if self.state != ClientState::Disconnected {
            return Err(MercuryError::AlreadyConnected {
                server: self.config.server().to_string(),
            });
        }

        self.state = ClientState::Connecting;
        info!(server = %self.config.server(), port = self.config.port(), "connecting");

        let irc_config = self.config.to_irc_config();
        match Client::from_config(irc_config).await {
            Ok(client) => {
                match client.identify() {
                    Ok(()) => {
                        self.state = ClientState::Connected;
                        self.inner = Some(client);
                        info!(server = %self.config.server(), "connected");
                        Ok(())
                    }
                    Err(e) => {
                        self.state = ClientState::Disconnected;
                        Err(MercuryError::Irc(e))
                    }
                }
            }
            Err(e) => {
                self.state = ClientState::Disconnected;
                warn!(error = %e, "connection failed");
                Err(MercuryError::Irc(e))
            }
        }
    }

    /// Disconnect from the IRC server, sending a QUIT message.
    ///
    /// Idempotent — safe to call when already disconnected.
    pub async fn disconnect(&mut self) -> Result<()> {
        if self.state == ClientState::Disconnected {
            return Ok(());
        }

        self.state = ClientState::Disconnecting;
        if let Some(client) = &self.inner {
            // Best-effort QUIT; ignore errors (connection may already be dead)
            let _ = client.send_quit("Mercury IRC Client");
        }
        self.inner = None;
        self.state = ClientState::Disconnected;
        info!("disconnected");
        Ok(())
    }

    /// Send an `OutboundMessage` to the server.
    ///
    /// Returns `Err(NotConnected)` if not currently connected.
    pub fn send(&self, msg: &OutboundMessage) -> Result<()> {
        let client = self.inner.as_ref().ok_or(MercuryError::NotConnected)?;
        let raw = msg.to_irc_string();
        debug!(message = %raw, "sending");
        client
            .send(Command::Raw(raw, vec![]))
            .map_err(MercuryError::Irc)
    }

    /// Returns a stream of inbound `irc::client::ClientStream` messages.
    ///
    /// Returns `Err(NotConnected)` if not connected.
    pub fn stream(&mut self) -> Result<irc::client::ClientStream> {
        let client = self.inner.as_mut().ok_or(MercuryError::NotConnected)?;
        Ok(client.stream()?)
    }
}
