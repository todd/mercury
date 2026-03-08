/// Unit tests for App navigation, channel member management, and PM tracking.
/// No live network required.
use mercury::tui::app::{App, MemberEntry};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn app() -> App {
    App::new_disconnected("localhost", 6667, "me")
}

// ---------------------------------------------------------------------------
// Channel member add / remove / rename
// ---------------------------------------------------------------------------

#[test]
fn test_add_channel_member_appears_in_list() {
    let mut app = app();
    app.add_channel_member("#test", MemberEntry::new("alice"));
    let members = app.channel_members.get("#test").unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].nick, "alice");
}

#[test]
fn test_add_channel_member_is_idempotent() {
    let mut app = app();
    app.add_channel_member("#test", MemberEntry::new("alice"));
    app.add_channel_member("#test", MemberEntry::new("alice"));
    let members = app.channel_members.get("#test").unwrap();
    assert_eq!(members.len(), 1);
}

#[test]
fn test_add_channel_member_case_insensitive_dedup() {
    let mut app = app();
    app.add_channel_member("#test", MemberEntry::new("Alice"));
    app.add_channel_member("#test", MemberEntry::new("alice"));
    let members = app.channel_members.get("#test").unwrap();
    assert_eq!(members.len(), 1);
}

#[test]
fn test_remove_channel_member() {
    let mut app = app();
    app.add_channel_member("#test", MemberEntry::new("alice"));
    app.add_channel_member("#test", MemberEntry::new("bob"));
    app.remove_channel_member("#test", "alice");
    let members = app.channel_members.get("#test").unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].nick, "bob");
}

#[test]
fn test_remove_channel_member_case_insensitive() {
    let mut app = app();
    app.add_channel_member("#test", MemberEntry::new("Alice"));
    app.remove_channel_member("#test", "ALICE");
    let members = app.channel_members.get("#test").unwrap();
    assert!(members.is_empty());
}

#[test]
fn test_rename_channel_member_across_channels() {
    let mut app = app();
    app.add_channel_member("#a", MemberEntry::new("alice"));
    app.add_channel_member("#b", MemberEntry::new("alice"));
    app.rename_channel_member("alice", "ali");
    let a = app.channel_members.get("#a").unwrap();
    let b = app.channel_members.get("#b").unwrap();
    assert_eq!(a[0].nick, "ali");
    assert_eq!(b[0].nick, "ali");
}

// ---------------------------------------------------------------------------
// sort_members ordering: ops → voiced → regular, each group alphabetically
// ---------------------------------------------------------------------------

#[test]
fn test_set_channel_members_sorts_ops_first() {
    let mut app = app();
    let members = vec![
        MemberEntry::new("bob"),
        MemberEntry::new("alice").op(),
        MemberEntry::new("carol"),
    ];
    app.set_channel_members("#test", members);
    let list = app.channel_members.get("#test").unwrap();
    assert_eq!(list[0].nick, "alice");
    assert!(list[0].is_op);
    assert_eq!(list[1].nick, "bob");
    assert_eq!(list[2].nick, "carol");
}

#[test]
fn test_set_channel_members_voiced_between_ops_and_regular() {
    let mut app = app();
    let members = vec![
        MemberEntry::new("reg"),
        MemberEntry::new("voiced").voiced(),
        MemberEntry::new("op").op(),
    ];
    app.set_channel_members("#test", members);
    let list = app.channel_members.get("#test").unwrap();
    assert!(list[0].is_op);
    assert!(list[1].is_voiced);
    assert!(!list[2].is_op && !list[2].is_voiced);
}

#[test]
fn test_set_channel_members_alpha_within_groups() {
    let mut app = app();
    let members = vec![
        MemberEntry::new("zop").op(),
        MemberEntry::new("aop").op(),
        MemberEntry::new("zreg"),
        MemberEntry::new("areg"),
    ];
    app.set_channel_members("#test", members);
    let list = app.channel_members.get("#test").unwrap();
    assert_eq!(list[0].nick, "aop");
    assert_eq!(list[1].nick, "zop");
    assert_eq!(list[2].nick, "areg");
    assert_eq!(list[3].nick, "zreg");
}

// ---------------------------------------------------------------------------
// active_channel_members
// ---------------------------------------------------------------------------

#[test]
fn test_active_channel_members_returns_empty_for_server_buffer() {
    let app = app();
    assert!(app.active_channel_members().is_empty());
}

#[test]
fn test_active_channel_members_returns_list_for_active_channel() {
    let mut app = app();
    app.set_channel_members("#test", vec![MemberEntry::new("alice")]);
    app.set_active_channel(Some("#test".to_string()));
    let members = app.active_channel_members();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].nick, "alice");
}

// ---------------------------------------------------------------------------
// Private message tracking
// ---------------------------------------------------------------------------

#[test]
fn test_open_private_chat_adds_to_list() {
    let mut app = app();
    app.open_private_chat("alice");
    assert!(app.private_chats.contains(&"alice".to_string()));
}

#[test]
fn test_open_private_chat_is_idempotent() {
    let mut app = app();
    app.open_private_chat("alice");
    app.open_private_chat("alice");
    assert_eq!(app.private_chats.len(), 1);
}

#[test]
fn test_open_private_chat_sorted_alpha() {
    let mut app = app();
    app.open_private_chat("zara");
    app.open_private_chat("alice");
    assert_eq!(app.private_chats[0], "alice");
    assert_eq!(app.private_chats[1], "zara");
}

#[test]
fn test_active_is_pm_true_when_pm_active() {
    let mut app = app();
    app.open_private_chat("alice");
    app.set_active_channel(Some("alice".to_string()));
    assert!(app.active_is_pm());
}

#[test]
fn test_active_is_pm_false_for_server_buffer() {
    let app = app();
    assert!(!app.active_is_pm());
}

#[test]
fn test_active_is_channel_true_for_channel() {
    let mut app = app();
    // Simulate a joined channel by setting active directly (no IRC needed).
    app.set_active_channel(Some("#rust".to_string()));
    assert!(app.active_is_channel());
    assert!(!app.active_is_pm());
}

#[test]
fn test_active_is_channel_false_for_server_buffer() {
    let app = app();
    assert!(!app.active_is_channel());
}

#[test]
fn test_active_is_channel_false_for_pm() {
    let mut app = app();
    app.open_private_chat("bob");
    app.set_active_channel(Some("bob".to_string()));
    assert!(!app.active_is_channel());
}

// ---------------------------------------------------------------------------
// Channel navigation: next_channel / prev_channel with wraparound
// ---------------------------------------------------------------------------

/// Build an app with two joined channels and one PM, then exercise nav.
fn nav_app() -> App {
    let mut app = app();
    // Simulate joined channels via channel manager (direct push to avoid IRC).
    // We use the fact that nav_list calls sorted_joined_channels which reads from channel_mgr.
    // Workaround: set active and open buffers manually; use channel_mgr internals via join msg.
    // The simplest approach: use open_private_chat for PMs and directly call
    // channel_mgr.join to set up channels for nav.

    // Join two channels using ChannelManager (which just records the join).
    let _ = app.channel_mgr.join("#alpha");
    app.channel_mgr.confirm_join("#alpha");
    let _ = app.channel_mgr.join("#beta");
    app.channel_mgr.confirm_join("#beta");

    // Open a PM conversation.
    app.open_private_chat("zara");

    app
}

#[test]
fn test_nav_starts_at_server_buffer() {
    let app = nav_app();
    // Default active_channel is None = server buffer.
    assert!(app.active_channel.is_none());
}

#[test]
fn test_next_channel_moves_from_server_to_first_channel() {
    let mut app = nav_app();
    app.next_channel();
    // nav order: None, #alpha, #beta, zara  → next from None is #alpha
    assert_eq!(app.active_channel.as_deref(), Some("#alpha"));
}

#[test]
fn test_next_channel_wraps_around_to_server() {
    let mut app = nav_app();
    // Advance to last entry (zara) then next should wrap to server (None).
    app.set_active_channel(Some("zara".to_string()));
    app.next_channel();
    assert!(app.active_channel.is_none());
}

#[test]
fn test_prev_channel_from_server_wraps_to_last() {
    let mut app = nav_app();
    // At server buffer, prev should wrap to last entry (zara).
    app.prev_channel();
    assert_eq!(app.active_channel.as_deref(), Some("zara"));
}

#[test]
fn test_prev_channel_moves_backwards() {
    let mut app = nav_app();
    app.set_active_channel(Some("#beta".to_string()));
    app.prev_channel();
    assert_eq!(app.active_channel.as_deref(), Some("#alpha"));
}

#[test]
fn test_next_channel_no_crash_with_only_server() {
    let mut app = app();
    // Only one entry (server), next/prev should be no-ops.
    app.next_channel();
    assert!(app.active_channel.is_none());
    app.prev_channel();
    assert!(app.active_channel.is_none());
}

// ---------------------------------------------------------------------------
// nav_list ordering: server first, then channels (alpha), then PMs (alpha)
// ---------------------------------------------------------------------------

#[test]
fn test_nav_list_order_server_channels_pms() {
    let mut app = app();
    let _ = app.channel_mgr.join("#beta");
    app.channel_mgr.confirm_join("#beta");
    let _ = app.channel_mgr.join("#alpha");
    app.channel_mgr.confirm_join("#alpha");
    app.open_private_chat("zara");
    app.open_private_chat("alice");

    // Navigate through all entries and record order.
    let mut order: Vec<Option<String>> = vec![app.active_channel.clone()]; // starts at server
    let total = 1 + 2 + 2; // server + 2 channels + 2 PMs
    for _ in 0..(total - 1) {
        app.next_channel();
        order.push(app.active_channel.clone());
    }

    assert_eq!(order[0], None, "first entry must be server buffer");
    assert_eq!(
        order[1].as_deref(),
        Some("#alpha"),
        "channels come before PMs"
    );
    assert_eq!(order[2].as_deref(), Some("#beta"));
    assert_eq!(order[3].as_deref(), Some("alice"), "PMs sorted alpha");
    assert_eq!(order[4].as_deref(), Some("zara"));
}

// ---------------------------------------------------------------------------
// Scroll state
// ---------------------------------------------------------------------------

#[test]
fn test_active_scroll_offset_defaults_to_zero() {
    let app = app();
    assert_eq!(app.active_scroll_offset(), 0);
}

#[test]
fn test_scroll_up_increases_offset() {
    let mut app = app();
    app.scroll_up(3);
    assert_eq!(app.active_scroll_offset(), 3);
}

#[test]
fn test_scroll_up_multiple_steps_accumulate() {
    let mut app = app();
    app.scroll_up(2);
    app.scroll_up(5);
    assert_eq!(app.active_scroll_offset(), 7);
}

#[test]
fn test_scroll_down_decreases_offset() {
    let mut app = app();
    app.scroll_up(10);
    app.scroll_down(4);
    assert_eq!(app.active_scroll_offset(), 6);
}

#[test]
fn test_scroll_down_clamps_at_zero() {
    let mut app = app();
    app.scroll_up(3);
    app.scroll_down(100);
    assert_eq!(app.active_scroll_offset(), 0);
}

#[test]
fn test_scroll_offset_is_per_buffer_independent() {
    let mut app = app();
    // Scroll server buffer up.
    app.scroll_up(5);
    assert_eq!(app.active_scroll_offset(), 5);

    // Switch to a channel — offset for the channel starts at 0.
    let _ = app.channel_mgr.join("#test");
    app.channel_mgr.confirm_join("#test");
    app.set_active_channel(Some("#test".to_string()));
    assert_eq!(app.active_scroll_offset(), 0, "channel buffer starts at 0");

    // Scroll the channel buffer.
    app.scroll_up(2);
    assert_eq!(app.active_scroll_offset(), 2);

    // Switch back to server — its offset is still 5.
    app.set_active_channel(None);
    assert_eq!(
        app.active_scroll_offset(),
        5,
        "server buffer offset preserved"
    );
}

#[test]
fn test_switching_channel_via_next_resets_to_bottom() {
    let mut app = app();
    // Scroll the server buffer.
    app.scroll_up(10);
    assert_eq!(app.active_scroll_offset(), 10);

    // Navigate to next buffer — this is a *different* buffer, starting at 0.
    let _ = app.channel_mgr.join("#alpha");
    app.channel_mgr.confirm_join("#alpha");
    app.next_channel();
    assert_eq!(
        app.active_scroll_offset(),
        0,
        "newly visited channel starts at bottom"
    );
}

#[test]
fn test_switching_channel_via_prev_resets_to_bottom() {
    let mut app = app();
    let _ = app.channel_mgr.join("#alpha");
    app.channel_mgr.confirm_join("#alpha");
    app.set_active_channel(Some("#alpha".to_string()));
    app.scroll_up(7);

    // prev wraps back to server buffer (which starts at 0 independently).
    app.prev_channel();
    assert_eq!(app.active_scroll_offset(), 0);
}
