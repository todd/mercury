/// Unit tests for channel management logic (create, join, leave).
/// Tests operate on the ChannelManager state machine — no live network required.
use mercury::irc::channel::{ChannelManager, ChannelState};
use mercury::irc::message::OutboundMessage;

// ---------------------------------------------------------------------------
// Channel name validation
// ---------------------------------------------------------------------------

#[test]
fn test_valid_channel_name_with_hash() {
    assert!(ChannelManager::is_valid_channel_name("#mercury"));
}

#[test]
fn test_valid_channel_name_with_ampersand() {
    assert!(ChannelManager::is_valid_channel_name("&local"));
}

#[test]
fn test_invalid_channel_name_no_prefix() {
    assert!(!ChannelManager::is_valid_channel_name("mercury"),
        "channel name without # or & prefix should be invalid");
}

#[test]
fn test_invalid_channel_name_empty() {
    assert!(!ChannelManager::is_valid_channel_name(""),
        "empty channel name should be invalid");
}

#[test]
fn test_invalid_channel_name_with_space() {
    assert!(!ChannelManager::is_valid_channel_name("#my channel"),
        "channel name with space should be invalid");
}

#[test]
fn test_invalid_channel_name_with_comma() {
    assert!(!ChannelManager::is_valid_channel_name("#foo,bar"),
        "channel name with comma should be invalid");
}

#[test]
fn test_invalid_channel_name_too_long() {
    let long_name = format!("#{}", "a".repeat(200));
    assert!(!ChannelManager::is_valid_channel_name(&long_name),
        "channel name exceeding 200 chars should be invalid");
}

// ---------------------------------------------------------------------------
// ChannelManager initial state
// ---------------------------------------------------------------------------

#[test]
fn test_new_channel_manager_has_no_channels() {
    let mgr = ChannelManager::new();
    assert!(mgr.joined_channels().is_empty());
}

// ---------------------------------------------------------------------------
// Feature 2: Create channel (build JOIN message)
// ---------------------------------------------------------------------------

#[test]
fn test_create_channel_produces_join_message() {
    let mut mgr = ChannelManager::new();
    let msg = mgr.create_channel("#mercury").expect("create_channel should succeed");
    assert_eq!(msg, OutboundMessage::Join { channel: "#mercury".to_string(), key: None });
}

#[test]
fn test_create_channel_with_key_produces_join_with_key() {
    let mut mgr = ChannelManager::new();
    let msg = mgr.create_channel_with_key("#secret", "password123")
        .expect("create_channel_with_key should succeed");
    assert_eq!(msg, OutboundMessage::Join {
        channel: "#secret".to_string(),
        key: Some("password123".to_string()),
    });
}

#[test]
fn test_create_channel_rejects_invalid_name() {
    let mut mgr = ChannelManager::new();
    let result = mgr.create_channel("no-prefix");
    assert!(result.is_err(), "create_channel with invalid name should fail");
}

// ---------------------------------------------------------------------------
// Feature 3: Join channel
// ---------------------------------------------------------------------------

#[test]
fn test_join_channel_produces_join_message() {
    let mut mgr = ChannelManager::new();
    let msg = mgr.join("#general").expect("join should succeed");
    assert_eq!(msg, OutboundMessage::Join { channel: "#general".to_string(), key: None });
}

#[test]
fn test_join_channel_rejects_invalid_name() {
    let mut mgr = ChannelManager::new();
    let result = mgr.join("invalid");
    assert!(result.is_err());
}

#[test]
fn test_confirm_join_tracks_channel_state() {
    let mut mgr = ChannelManager::new();
    mgr.join("#general").unwrap();
    // Simulate server confirming the join
    mgr.confirm_join("#general");
    assert!(mgr.is_joined("#general"), "channel should be tracked after confirmed join");
    assert_eq!(mgr.joined_channels().len(), 1);
}

#[test]
fn test_join_already_joined_channel_is_noop() {
    let mut mgr = ChannelManager::new();
    mgr.join("#general").unwrap();
    mgr.confirm_join("#general");
    // Trying to join again should be idempotent — same JOIN message is fine to send
    // but local state should not duplicate
    let result = mgr.join("#general");
    assert!(result.is_ok(), "joining an already-joined channel should be Ok");
    // Still only one entry
    assert_eq!(mgr.joined_channels().len(), 1);
}

#[test]
fn test_channel_state_before_confirm_is_joining() {
    let mut mgr = ChannelManager::new();
    mgr.join("#general").unwrap();
    assert_eq!(mgr.channel_state("#general"), Some(ChannelState::Joining));
}

#[test]
fn test_channel_state_after_confirm_is_joined() {
    let mut mgr = ChannelManager::new();
    mgr.join("#general").unwrap();
    mgr.confirm_join("#general");
    assert_eq!(mgr.channel_state("#general"), Some(ChannelState::Joined));
}

// ---------------------------------------------------------------------------
// Feature 3: Leave channel
// ---------------------------------------------------------------------------

#[test]
fn test_leave_joined_channel_produces_part_message() {
    let mut mgr = ChannelManager::new();
    mgr.join("#general").unwrap();
    mgr.confirm_join("#general");
    let msg = mgr.leave("#general", None).expect("leave should succeed");
    assert_eq!(msg, OutboundMessage::Part {
        channel: "#general".to_string(),
        reason: None,
    });
}

#[test]
fn test_leave_channel_with_reason() {
    let mut mgr = ChannelManager::new();
    mgr.join("#general").unwrap();
    mgr.confirm_join("#general");
    let msg = mgr.leave("#general", Some("Goodbye!")).expect("leave with reason should succeed");
    assert_eq!(msg, OutboundMessage::Part {
        channel: "#general".to_string(),
        reason: Some("Goodbye!".to_string()),
    });
}

#[test]
fn test_leave_channel_not_joined_returns_error() {
    let mut mgr = ChannelManager::new();
    let result = mgr.leave("#general", None);
    assert!(result.is_err(), "leaving a channel you're not in should error");
}

#[test]
fn test_confirm_part_removes_channel_from_tracking() {
    let mut mgr = ChannelManager::new();
    mgr.join("#general").unwrap();
    mgr.confirm_join("#general");
    mgr.leave("#general", None).unwrap();
    mgr.confirm_part("#general");
    assert!(!mgr.is_joined("#general"), "channel should be removed after confirmed part");
    assert!(mgr.joined_channels().is_empty());
}

#[test]
fn test_multiple_channels_tracked_independently() {
    let mut mgr = ChannelManager::new();

    mgr.join("#alpha").unwrap();
    mgr.confirm_join("#alpha");

    mgr.join("#beta").unwrap();
    mgr.confirm_join("#beta");

    assert_eq!(mgr.joined_channels().len(), 2);
    assert!(mgr.is_joined("#alpha"));
    assert!(mgr.is_joined("#beta"));

    mgr.leave("#alpha", None).unwrap();
    mgr.confirm_part("#alpha");

    assert_eq!(mgr.joined_channels().len(), 1);
    assert!(!mgr.is_joined("#alpha"));
    assert!(mgr.is_joined("#beta"));
}

// ---------------------------------------------------------------------------
// OutboundMessage IRC serialization
// ---------------------------------------------------------------------------

#[test]
fn test_join_message_serializes_to_irc() {
    let msg = OutboundMessage::Join { channel: "#mercury".to_string(), key: None };
    assert_eq!(msg.to_irc_string(), "JOIN #mercury");
}

#[test]
fn test_join_message_with_key_serializes_to_irc() {
    let msg = OutboundMessage::Join {
        channel: "#secret".to_string(),
        key: Some("pass".to_string()),
    };
    assert_eq!(msg.to_irc_string(), "JOIN #secret pass");
}

#[test]
fn test_part_message_serializes_to_irc() {
    let msg = OutboundMessage::Part { channel: "#mercury".to_string(), reason: None };
    assert_eq!(msg.to_irc_string(), "PART #mercury");
}

#[test]
fn test_part_message_with_reason_serializes_to_irc() {
    let msg = OutboundMessage::Part {
        channel: "#mercury".to_string(),
        reason: Some("Goodbye!".to_string()),
    };
    assert_eq!(msg.to_irc_string(), "PART #mercury :Goodbye!");
}
