/// Integration tests: user management against a live UnrealIRCd container.
///
/// Covers nick change, WHOIS, and WHO against a real server.
///
/// Run with:
///   cargo test --features integration --test user_integration

mod common;

#[cfg(feature = "integration")]
mod tests {
    use super::common::{IRC_HOST, IRC_PORT, start_ircd};
    use mercury::irc::client::{ClientConfig, IrcClient};
    use mercury::irc::user::UserManager;
    use std::time::Duration;
    use futures::StreamExt;
    use ::irc::client::prelude::{Command, Response};

    /// Connect, drain until RPL_WELCOME, return (client, stream, user_mgr).
    async fn connect_and_welcome(
        nick: &str,
    ) -> (IrcClient, ::irc::client::ClientStream, UserManager) {
        start_ircd();
        let config = ClientConfig::new(IRC_HOST, IRC_PORT, nick);
        let mut client = IrcClient::new(config);
        client.connect().await.expect("connect");

        let mut stream = client.stream().expect("stream");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);

        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    if let Command::Response(Response::RPL_WELCOME, _) = msg.command {
                        break;
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error waiting for WELCOME: {}", e),
                Ok(None) => panic!("stream ended before WELCOME"),
                Err(_) => panic!("timed out waiting for WELCOME"),
            }
        }

        let user_mgr = UserManager::new(nick).expect("valid nick");
        (client, stream, user_mgr)
    }

    // -----------------------------------------------------------------------
    // Nick change
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_nick_change_confirmed_by_server() {
        let (mut client, mut stream, mut user_mgr) =
            connect_and_welcome("merc_usr1").await;

        let msg = user_mgr.request_nick_change("merc_usr1b").unwrap();
        client.send(&msg).expect("send NICK");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut confirmed = false;

        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(m))) => {
                    if let Command::NICK(new_nick) = &m.command {
                        user_mgr.confirm_nick_change("merc_usr1", new_nick);
                        if user_mgr.current_nick() == "merc_usr1b" {
                            confirmed = true;
                            break;
                        }
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error: {}", e),
                Ok(None) | Err(_) => break,
            }
        }

        assert!(confirmed, "server should echo NICK change");
        assert_eq!(user_mgr.current_nick(), "merc_usr1b");
        assert!(user_mgr.pending_nick().is_none());

        client.disconnect().await.expect("disconnect");
    }

    #[tokio::test]
    async fn test_nick_in_use_returns_error_response() {
        // Connect client A and hold the nick, then have client B attempt to
        // change to that nick. The server must respond with ERR_NICKNAMEINUSE.
        //
        // We use a single connection here: client A holds "merc_niu_hold".
        // Client B connects with a separate nick and then requests the same
        // nick via a NICK command *after* the handshake completes — this way
        // the ERR arrives on the already-established stream, not during
        // registration where the irc crate's nick-cycling logic would interfere.
        let (mut client_a, _stream_a, _) = connect_and_welcome("merc_niu_hold").await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let (mut client_b, mut stream_b, mut user_mgr_b) =
            connect_and_welcome("merc_niu_taker").await;

        // Now request client A's nick from an already-registered connection.
        let msg = user_mgr_b.request_nick_change("merc_niu_hold").unwrap();
        client_b.send(&msg).expect("send NICK");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut got_error = false;

        loop {
            match tokio::time::timeout_at(deadline, stream_b.next()).await {
                Ok(Some(Ok(m))) => {
                    match &m.command {
                        Command::Response(Response::ERR_NICKNAMEINUSE, _) => {
                            got_error = true;
                            break;
                        }
                        // Ignore any lingering server messages (MOTD etc.)
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error: {}", e),
                Ok(None) | Err(_) => break,
            }
        }

        assert!(got_error, "server should reject duplicate nick with ERR_NICKNAMEINUSE");
        // client_b's nick must remain what we connected with.
        assert_eq!(user_mgr_b.current_nick(), "merc_niu_taker");

        client_a.disconnect().await.expect("disconnect a");
        client_b.disconnect().await.expect("disconnect b");
    }

    // -----------------------------------------------------------------------
    // WHOIS
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_whois_returns_user_info() {
        let (mut client, mut stream, mut user_mgr) =
            connect_and_welcome("merc_usr3").await;

        // WHOIS ourselves — always works.
        let msg = user_mgr.build_whois("merc_usr3").unwrap();
        client.send(&msg).expect("send WHOIS");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut got_whois_user = false;

        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(m))) => {
                    match &m.command {
                        // 311 RPL_WHOISUSER
                        Command::Response(Response::RPL_WHOISUSER, args) => {
                            if args.get(1).map(|s| s.as_str()) == Some("merc_usr3") {
                                let user = args.get(2).map(|s| s.as_str()).unwrap_or("");
                                let host = args.get(3).map(|s| s.as_str()).unwrap_or("");
                                let realname = args.last().map(|s| s.as_str()).unwrap_or("");
                                user_mgr.handle_whois_user("merc_usr3", user, host, realname);
                                got_whois_user = true;
                            }
                        }
                        // 318 RPL_ENDOFWHOIS — stop after end of reply
                        Command::Response(Response::RPL_ENDOFWHOIS, _) => break,
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error: {}", e),
                Ok(None) | Err(_) => break,
            }
        }

        assert!(got_whois_user, "should receive RPL_WHOISUSER for ourselves");
        let info = user_mgr.whois_info("merc_usr3").unwrap();
        assert_eq!(info.nick, "merc_usr3");
        assert!(!info.host.is_empty(), "host should be populated");

        client.disconnect().await.expect("disconnect");
    }

    // -----------------------------------------------------------------------
    // WHO
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_who_returns_entries() {
        let (mut client, mut stream, mut user_mgr) =
            connect_and_welcome("merc_usr4").await;

        // WHO ourselves by nick.
        let msg = user_mgr.build_who("merc_usr4").unwrap();
        client.send(&msg).expect("send WHO");
        user_mgr.clear_who_results();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(m))) => {
                    match &m.command {
                        // 352 RPL_WHOREPLY: channel user host server nick flags :hops realname
                        Command::Response(Response::RPL_WHOREPLY, args) if args.len() >= 7 => {
                            let nick  = &args[5];
                            let user  = &args[2];
                            let host  = &args[3];
                            let srv   = &args[4];
                            let flags = &args[6];
                            let rn    = args.last()
                                .and_then(|s| s.splitn(2, ' ').nth(1))
                                .unwrap_or("");
                            user_mgr.handle_who_reply(nick, user, host, srv, flags, rn);
                        }
                        // 315 RPL_ENDOFWHO
                        Command::Response(Response::RPL_ENDOFWHO, _) => break,
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error: {}", e),
                Ok(None) | Err(_) => break,
            }
        }

        let results = user_mgr.who_results();
        assert!(!results.is_empty(), "WHO should return at least one entry");
        assert!(
            results.iter().any(|e| e.nick.eq_ignore_ascii_case("merc_usr4")),
            "WHO results should include ourselves"
        );

        client.disconnect().await.expect("disconnect");
    }
}

#[cfg(not(feature = "integration"))]
#[test]
fn integration_tests_require_feature_flag() {
    // Run with --features integration to execute integration tests.
}
