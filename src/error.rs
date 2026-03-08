use thiserror::Error;

#[derive(Error, Debug)]
pub enum MercuryError {
    #[error("IRC error: {0}")]
    Irc(#[from] irc::error::Error),

    #[error("Not connected to a server")]
    NotConnected,

    #[error("Already connected to {server}")]
    AlreadyConnected { server: String },

    #[error("Invalid channel name '{name}': {reason}")]
    InvalidChannelName { name: String, reason: String },

    #[error("Not in channel '{channel}'")]
    NotInChannel { channel: String },

    #[error("Invalid nickname '{nick}': {reason}")]
    InvalidNick { nick: String, reason: String },

    #[error("Already in channel '{channel}'")]
    AlreadyInChannel { channel: String },

    #[error("Connection to {server}:{port} failed: {source}")]
    ConnectionFailed {
        server: String,
        port: u16,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal channel error: {0}")]
    ChannelSend(String),
}

pub type Result<T> = std::result::Result<T, MercuryError>;
