/// Outbound IRC messages that Mercury can send to a server.
/// Each variant maps 1:1 to an IRCv3 command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundMessage {
    /// JOIN <channel> [<key>]
    Join { channel: String, key: Option<String> },
    /// PART <channel> [:<reason>]
    Part { channel: String, reason: Option<String> },
    /// PRIVMSG <target> :<text>
    PrivMsg { target: String, text: String },
    /// QUIT [:<reason>]
    Quit { reason: Option<String> },
    /// PING <server>
    Ping { server: String },
    /// PONG <server>
    Pong { server: String },

    // -- User management -----------------------------------------------------

    /// NICK <new_nick>  — request a nickname change
    Nick { new_nick: String },
    /// WHOIS [<server>] <nick>  — query user information
    Whois { nick: String },
    /// WHO <mask>  — list users matching a mask (nick, channel, or wildcard)
    Who { mask: String },
    /// PRIVMSG NickServ :<text>  — send a message to NickServ (services)
    NickServ { text: String },

    /// Raw IRC string — escape hatch for commands not yet modelled
    Raw(String),
}

impl OutboundMessage {
    /// Serialize to a raw IRC protocol string (without trailing CRLF).
    pub fn to_irc_string(&self) -> String {
        match self {
            OutboundMessage::Join { channel, key: None } => {
                format!("JOIN {}", channel)
            }
            OutboundMessage::Join { channel, key: Some(k) } => {
                format!("JOIN {} {}", channel, k)
            }
            OutboundMessage::Part { channel, reason: None } => {
                format!("PART {}", channel)
            }
            OutboundMessage::Part { channel, reason: Some(r) } => {
                format!("PART {} :{}", channel, r)
            }
            OutboundMessage::PrivMsg { target, text } => {
                format!("PRIVMSG {} :{}", target, text)
            }
            OutboundMessage::Quit { reason: None } => "QUIT".to_string(),
            OutboundMessage::Quit { reason: Some(r) } => format!("QUIT :{}", r),
            OutboundMessage::Ping { server } => format!("PING {}", server),
            OutboundMessage::Pong { server } => format!("PONG {}", server),
            OutboundMessage::Nick { new_nick } => format!("NICK {}", new_nick),
            OutboundMessage::Whois { nick } => format!("WHOIS {}", nick),
            OutboundMessage::Who { mask } => format!("WHO {}", mask),
            OutboundMessage::NickServ { text } => format!("PRIVMSG NickServ :{}", text),
            OutboundMessage::Raw(s) => s.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_no_key() {
        let m = OutboundMessage::Join { channel: "#foo".into(), key: None };
        assert_eq!(m.to_irc_string(), "JOIN #foo");
    }

    #[test]
    fn join_with_key() {
        let m = OutboundMessage::Join { channel: "#foo".into(), key: Some("bar".into()) };
        assert_eq!(m.to_irc_string(), "JOIN #foo bar");
    }

    #[test]
    fn part_no_reason() {
        let m = OutboundMessage::Part { channel: "#foo".into(), reason: None };
        assert_eq!(m.to_irc_string(), "PART #foo");
    }

    #[test]
    fn part_with_reason() {
        let m = OutboundMessage::Part { channel: "#foo".into(), reason: Some("bye".into()) };
        assert_eq!(m.to_irc_string(), "PART #foo :bye");
    }

    #[test]
    fn privmsg() {
        let m = OutboundMessage::PrivMsg { target: "#foo".into(), text: "hello".into() };
        assert_eq!(m.to_irc_string(), "PRIVMSG #foo :hello");
    }

    #[test]
    fn quit_no_reason() {
        let m = OutboundMessage::Quit { reason: None };
        assert_eq!(m.to_irc_string(), "QUIT");
    }

    #[test]
    fn quit_with_reason() {
        let m = OutboundMessage::Quit { reason: Some("bye".into()) };
        assert_eq!(m.to_irc_string(), "QUIT :bye");
    }

    // -- User management -----------------------------------------------------

    #[test]
    fn nick_change() {
        let m = OutboundMessage::Nick { new_nick: "newme".into() };
        assert_eq!(m.to_irc_string(), "NICK newme");
    }

    #[test]
    fn whois() {
        let m = OutboundMessage::Whois { nick: "alice".into() };
        assert_eq!(m.to_irc_string(), "WHOIS alice");
    }

    #[test]
    fn who_mask() {
        let m = OutboundMessage::Who { mask: "#general".into() };
        assert_eq!(m.to_irc_string(), "WHO #general");
    }

    #[test]
    fn nickserv_message() {
        let m = OutboundMessage::NickServ { text: "IDENTIFY hunter2".into() };
        assert_eq!(m.to_irc_string(), "PRIVMSG NickServ :IDENTIFY hunter2");
    }

    #[test]
    fn nickserv_register() {
        let m = OutboundMessage::NickServ { text: "REGISTER hunter2 user@example.com".into() };
        assert_eq!(m.to_irc_string(), "PRIVMSG NickServ :REGISTER hunter2 user@example.com");
    }
}
