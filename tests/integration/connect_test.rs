/// Integration tests: connect / disconnect against a live UnrealIRCd container.
///
/// Run with:
///   cargo test --features integration --test connect_integration
///
/// Requires Docker with UnrealIRCd running (start with):
///   docker-compose -f docker/docker-compose.yml up -d

mod common;

#[cfg(feature = "integration")]
mod tests {
    use super::common::{IRC_HOST, IRC_PORT, IRC_TLS_PORT, start_ircd};
    use mercury::irc::client::{ClientConfig, ClientState, IrcClient};
    use std::time::Duration;
    use futures::StreamExt;
    use ::irc::client::prelude::{Command, Response};

    /// Plain (non-TLS) client on port 6667.
    fn make_plain_client(nick: &str) -> IrcClient {
        let config = ClientConfig::new(IRC_HOST, IRC_PORT, nick).plain();
        IrcClient::new(config)
    }

    /// Helper: read the stream until RPL_WELCOME arrives or the deadline passes.
    async fn wait_for_welcome(
        stream: &mut ::irc::client::ClientStream,
        deadline: tokio::time::Instant,
    ) -> bool {
        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    if let Command::Response(Response::RPL_WELCOME, _) = msg.command {
                        return true;
                    }
                }
                Ok(Some(Err(e))) => panic!("stream error: {}", e),
                Ok(None) | Err(_) => return false,
            }
        }
    }

    #[tokio::test]
    async fn test_integration_connect_receives_welcome() {
        start_ircd();
        let mut client = make_plain_client("merc_conn1");

        client.connect().await.expect("should connect to live IRCd");
        assert_eq!(client.state(), ClientState::Connected);

        let mut stream = client.stream().expect("should get stream");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        assert!(
            wait_for_welcome(&mut stream, deadline).await,
            "should receive RPL_WELCOME (001) on connect"
        );

        client.disconnect().await.expect("should disconnect cleanly");
        assert_eq!(client.state(), ClientState::Disconnected);
    }

    /// Regression test: `client.stream()` must only be called once per
    /// connected `Client`. Calling it a second time returns `Err` because the
    /// irc crate's internal async channel is consumed on the first call.
    ///
    /// The original bug caused every frame tick in `poll_irc_messages` to call
    /// `stream()` again, which orphaned the sender half of the channel so that
    /// any subsequent `send()` (e.g. JOIN) failed with "async channel closed".
    #[tokio::test]
    async fn test_stream_can_only_be_called_once() {
        start_ircd();
        let mut client = make_plain_client("merc_stream1");
        client.connect().await.expect("connect");

        // First call succeeds and returns the live stream.
        let _stream = client.stream().expect("first stream() call should succeed");

        // Second call on the same connected client must return Err — the
        // channel has already been handed out.
        assert!(
            client.stream().is_err(),
            "second stream() call on same client should return Err"
        );

        client.disconnect().await.expect("disconnect");
    }

    #[tokio::test]
    async fn test_integration_disconnect_is_clean() {
        start_ircd();
        let mut client = make_plain_client("merc_conn2");
        client.connect().await.expect("connect");
        assert_eq!(client.state(), ClientState::Connected);
        client.disconnect().await.expect("disconnect");
        assert_eq!(client.state(), ClientState::Disconnected);
        // Second disconnect is a no-op
        client.disconnect().await.expect("second disconnect is no-op");
        assert_eq!(client.state(), ClientState::Disconnected);
    }

    /// TLS connection test — connects on port 6697 with a self-signed cert.
    ///
    /// The UnrealIRCd test container generates a self-signed certificate at
    /// build time (see docker/unrealircd/Dockerfile), so certificate validation
    /// must be bypassed with `accept_invalid_certs()`.  This mirrors what a
    /// user would do with `/connect <server> --no-verify` against a server that
    /// lacks a CA-signed certificate.
    #[tokio::test]
    async fn test_integration_connect_tls_receives_welcome() {
        start_ircd();

        let config = ClientConfig::new(IRC_HOST, IRC_TLS_PORT, "merc_tls1")
            .accept_invalid_certs(); // self-signed cert in test container
        let mut client = IrcClient::new(config);

        assert!(client.is_tls(), "client should be configured for TLS");

        client
            .connect()
            .await
            .expect("should connect over TLS to live IRCd");
        assert_eq!(client.state(), ClientState::Connected);

        let mut stream = client.stream().expect("should get stream");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        assert!(
            wait_for_welcome(&mut stream, deadline).await,
            "should receive RPL_WELCOME (001) over TLS"
        );

        client.disconnect().await.expect("should disconnect cleanly");
        assert_eq!(client.state(), ClientState::Disconnected);
    }
}

#[cfg(not(feature = "integration"))]
#[test]
fn integration_tests_require_feature_flag() {
    // Run with --features integration to execute integration tests.
}
