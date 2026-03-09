/// Unit tests for IrcClient connect/disconnect logic.
/// These tests operate at the configuration and state level — no live network required.
use mercury::irc::client::{ClientConfig, ClientState, IrcClient};

// ---------------------------------------------------------------------------
// TLS configuration
// ---------------------------------------------------------------------------

#[test]
fn test_client_config_tls_on_by_default() {
    let config = ClientConfig::new("irc.example.com", 6697, "mercury_test");
    assert!(config.is_tls(), "TLS should be on by default");
}

#[test]
fn test_client_config_plain_disables_tls() {
    let config = ClientConfig::new("irc.example.com", 6667, "mercury_test").plain();
    assert!(!config.is_tls(), "plain() should disable TLS");
}

#[test]
fn test_client_config_accept_invalid_certs_default_false() {
    let config = ClientConfig::new("irc.example.com", 6697, "mercury_test");
    assert!(
        !config.danger_accept_invalid_certs(),
        "cert validation should be on by default"
    );
}

#[test]
fn test_client_config_accept_invalid_certs_builder() {
    let config = ClientConfig::new("irc.example.com", 6697, "mercury_test")
        .accept_invalid_certs();
    assert!(
        config.danger_accept_invalid_certs(),
        "accept_invalid_certs() should set the danger flag"
    );
}

#[test]
fn test_irc_config_tls_flag_propagates() {
    let config = ClientConfig::new("irc.example.com", 6697, "mercury_test");
    let irc_cfg = config.to_irc_config();
    assert_eq!(
        irc_cfg.use_tls,
        Some(true),
        "use_tls=true must reach the irc crate config"
    );
}

#[test]
fn test_irc_config_plain_flag_propagates() {
    let config = ClientConfig::new("irc.example.com", 6667, "mercury_test").plain();
    let irc_cfg = config.to_irc_config();
    assert_eq!(
        irc_cfg.use_tls,
        Some(false),
        "use_tls=false must reach the irc crate config after plain()"
    );
}

#[test]
fn test_irc_config_danger_flag_propagates() {
    let config = ClientConfig::new("irc.example.com", 6697, "mercury_test")
        .accept_invalid_certs();
    let irc_cfg = config.to_irc_config();
    assert_eq!(
        irc_cfg.dangerously_accept_invalid_certs,
        Some(true),
        "dangerously_accept_invalid_certs must reach the irc crate config"
    );
}

#[test]
fn test_irc_client_is_tls_reflects_config() {
    let tls_config = ClientConfig::new("irc.example.com", 6697, "mercury_test");
    let plain_config = ClientConfig::new("irc.example.com", 6667, "mercury_test").plain();
    assert!(IrcClient::new(tls_config).is_tls());
    assert!(!IrcClient::new(plain_config).is_tls());
}

// ---------------------------------------------------------------------------
// Configuration validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_client_config_valid() {
    let config = ClientConfig::new("irc.example.com", 6667, "mercury_test");
    assert_eq!(config.server(), "irc.example.com");
    assert_eq!(config.port(), 6667);
    assert_eq!(config.nick(), "mercury_test");
}

#[test]
fn test_client_config_default_port() {
    // with_defaults uses 6697 — the standard IRC-over-TLS port — because TLS
    // is on by default.
    let config = ClientConfig::with_defaults("irc.example.com", "mercury_test");
    assert_eq!(config.port(), 6697);
}

#[test]
fn test_client_config_rejects_empty_server() {
    let result = ClientConfig::try_new("", 6667, "mercury_test");
    assert!(result.is_err(), "empty server should be rejected");
}

#[test]
fn test_client_config_rejects_empty_nick() {
    let result = ClientConfig::try_new("irc.example.com", 6667, "");
    assert!(result.is_err(), "empty nick should be rejected");
}

#[test]
fn test_client_config_rejects_nick_with_spaces() {
    let result = ClientConfig::try_new("irc.example.com", 6667, "my nick");
    assert!(result.is_err(), "nick with spaces should be rejected");
}

#[test]
fn test_client_config_rejects_invalid_port_zero() {
    let result = ClientConfig::try_new("irc.example.com", 0, "mercury_test");
    assert!(result.is_err(), "port 0 should be rejected");
}

// ---------------------------------------------------------------------------
// Initial state tests
// ---------------------------------------------------------------------------

#[test]
fn test_new_client_is_disconnected() {
    let config = ClientConfig::new("irc.example.com", 6667, "mercury_test");
    let client = IrcClient::new(config);
    assert_eq!(client.state(), ClientState::Disconnected);
}

#[test]
fn test_new_client_has_no_current_server() {
    let config = ClientConfig::new("irc.example.com", 6667, "mercury_test");
    let client = IrcClient::new(config);
    assert!(client.current_server().is_none());
}

// ---------------------------------------------------------------------------
// State transition tests (no live network)
// ---------------------------------------------------------------------------

#[test]
fn test_client_state_display() {
    assert_eq!(ClientState::Disconnected.to_string(), "Disconnected");
    assert_eq!(ClientState::Connecting.to_string(), "Connecting");
    assert_eq!(ClientState::Connected.to_string(), "Connected");
    assert_eq!(ClientState::Disconnecting.to_string(), "Disconnecting");
}

#[tokio::test]
async fn test_disconnect_when_not_connected_is_noop() {
    let config = ClientConfig::new("irc.example.com", 6667, "mercury_test");
    let mut client = IrcClient::new(config);
    // Disconnecting when already disconnected should be Ok (idempotent)
    let result = client.disconnect().await;
    assert!(result.is_ok(), "disconnect when not connected should be a no-op");
    assert_eq!(client.state(), ClientState::Disconnected);
}

#[tokio::test]
async fn test_connect_to_unreachable_host_returns_error() {
    // Port 1 on localhost is almost certainly not an IRC server.
    let config = ClientConfig::new("127.0.0.1", 1, "mercury_test");
    let mut client = IrcClient::new(config);
    let result = client.connect().await;
    assert!(result.is_err(), "connecting to unreachable host should fail");
    assert_eq!(
        client.state(),
        ClientState::Disconnected,
        "state should revert to Disconnected after failed connect"
    );
}
