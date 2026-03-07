/// Integration tests: channel create / join / leave against a live UnrealIRCd container.
///
/// Run with:
///   cargo test --features integration --test channel_integration

mod common;

#[cfg(feature = "integration")]
mod tests {
    use super::common::{IRC_HOST, IRC_PORT, start_ircd};
    use mercury::irc::channel::ChannelManager;
    use mercury::irc::client::{ClientConfig, ClientState, IrcClient};
    use mercury::irc::message::OutboundMessage;
    use std::time::Duration;
    use futures::StreamExt;
    use ::irc::client::prelude::{Command, Response};

    /// Connect and wait for RPL_WELCOME on a single persistent stream.
    /// Returns `(client, stream)` — the stream must not be re-created.
    async fn connect_and_welcome(nick: &str) -> (IrcClient, ::irc::client::ClientStream) {
        start_ircd();
        let config = ClientConfig::new(IRC_HOST, IRC_PORT, nick);
        let mut client = IrcClient::new(config);
        client.connect().await.expect("should connect");
        assert_eq!(client.state(), ClientState::Connected);

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

        (client, stream)
    }

    // -----------------------------------------------------------------------
    // Feature 2: Create channel
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_integration_create_channel_receives_namreply() {
        let (mut client, mut stream) = connect_and_welcome("merc_create1").await;
        let mut mgr = ChannelManager::new();

        let join_msg = mgr.create_channel("#mercury-int-create")
            .expect("create_channel should produce JOIN message");
        client.send(&join_msg).expect("send JOIN");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let mut got_namreply = false;

        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    if let Command::Response(Response::RPL_NAMREPLY, args) = &msg.command {
                        if args.iter().any(|a| a.contains("mercury-int-create")) {
                            got_namreply = true;
                            mgr.confirm_join("#mercury-int-create");
                            break;
                        }
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error: {}", e),
                Ok(None) | Err(_) => break,
            }
        }

        assert!(got_namreply, "should receive RPL_NAMREPLY after creating channel");
        assert!(mgr.is_joined("#mercury-int-create"), "channel should be marked joined");
        client.disconnect().await.expect("disconnect");
    }

    // -----------------------------------------------------------------------
    // Feature 3: Join channel
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_integration_join_channel_receives_join_echo() {
        let (mut client, mut stream) = connect_and_welcome("merc_join1").await;
        let mut mgr = ChannelManager::new();

        let join_msg = mgr.join("#mercury-int-join")
            .expect("join should produce JOIN message");
        client.send(&join_msg).expect("send JOIN");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let mut got_join = false;

        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    if let Command::JOIN(channel, _, _) = &msg.command {
                        if channel.to_lowercase().contains("mercury-int-join") {
                            got_join = true;
                            mgr.confirm_join(channel);
                            break;
                        }
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error: {}", e),
                Ok(None) | Err(_) => break,
            }
        }

        assert!(got_join, "should receive JOIN echo from server");
        assert!(mgr.is_joined("#mercury-int-join"), "channel state should be Joined");
        client.disconnect().await.expect("disconnect");
    }

    // -----------------------------------------------------------------------
    // Feature 3: Leave channel
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_integration_leave_channel_receives_part_echo() {
        let (mut client, mut stream) = connect_and_welcome("merc_part1").await;
        let mut mgr = ChannelManager::new();

        // Join first
        let join_msg = mgr.join("#mercury-int-part").unwrap();
        client.send(&join_msg).expect("send JOIN");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let mut joined = false;
        let mut got_part = false;

        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    match &msg.command {
                        // Our own JOIN echo confirms we're in the channel
                        Command::JOIN(ch, _, _) if ch.contains("mercury-int-part") => {
                            if !joined {
                                mgr.confirm_join(ch);
                                joined = true;
                                // Now send PART
                                let part_msg = mgr
                                    .leave("#mercury-int-part", Some("integration test"))
                                    .unwrap();
                                client.send(&part_msg).expect("send PART");
                            }
                        }
                        // NAMREPLY also confirms we joined (server may send this before JOIN echo)
                        Command::Response(Response::RPL_NAMREPLY, args)
                            if !joined && args.iter().any(|a| a.contains("mercury-int-part")) =>
                        {
                            mgr.confirm_join("#mercury-int-part");
                            joined = true;
                            let part_msg = mgr
                                .leave("#mercury-int-part", Some("integration test"))
                                .unwrap();
                            client.send(&part_msg).expect("send PART");
                        }
                        // PART echo confirms we left
                        Command::PART(channel, _)
                            if channel.contains("mercury-int-part") =>
                        {
                            got_part = true;
                            mgr.confirm_part(channel);
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error: {}", e),
                Ok(None) | Err(_) => break,
            }
        }

        assert!(joined, "should have joined channel before testing PART");
        assert!(got_part, "should receive PART echo from server");
        assert!(
            !mgr.is_joined("#mercury-int-part"),
            "channel should no longer be tracked"
        );
        client.disconnect().await.expect("disconnect");
    }
}

#[cfg(not(feature = "integration"))]
#[test]
fn integration_tests_require_feature_flag() {
    // Run with --features integration to execute integration tests.
}
