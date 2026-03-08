/// Unit tests for UserManager: nick validation, nick changes, WHOIS/WHO
/// result caching, and NickServ message construction.
/// No live network required.
use mercury::irc::message::OutboundMessage;
use mercury::irc::user::{is_valid_nick, NickServStatus, UserManager};
use mercury::tui::app::App;

// ---------------------------------------------------------------------------
// Nick validation
// ---------------------------------------------------------------------------

#[test]
fn test_valid_nick_simple() {
    assert!(is_valid_nick("alice"));
}

#[test]
fn test_valid_nick_with_digits() {
    assert!(is_valid_nick("user123"));
}

#[test]
fn test_valid_nick_with_hyphen() {
    assert!(is_valid_nick("my-nick"));
}

#[test]
fn test_valid_nick_with_special_start() {
    assert!(is_valid_nick("[Guest]"));
    assert!(is_valid_nick("_underscore"));
    assert!(is_valid_nick("{curly}"));
    assert!(is_valid_nick("|pipe|"));
    assert!(is_valid_nick("^caret"));
    assert!(is_valid_nick("\\backslash"));
}

#[test]
fn test_invalid_nick_empty() {
    assert!(!is_valid_nick(""));
}

#[test]
fn test_invalid_nick_starts_with_digit() {
    assert!(!is_valid_nick("1user"));
}

#[test]
fn test_invalid_nick_starts_with_hyphen() {
    assert!(!is_valid_nick("-user"));
}

#[test]
fn test_invalid_nick_contains_space() {
    assert!(!is_valid_nick("my nick"));
}

#[test]
fn test_invalid_nick_contains_at() {
    assert!(!is_valid_nick("user@host"));
}

#[test]
fn test_invalid_nick_too_long() {
    // 31 characters — one over the 30-char limit
    assert!(!is_valid_nick(&"a".repeat(31)));
}

#[test]
fn test_valid_nick_max_length() {
    // 30 characters — exactly at the limit
    assert!(is_valid_nick(&"a".repeat(30)));
}

// ---------------------------------------------------------------------------
// UserManager construction
// ---------------------------------------------------------------------------

#[test]
fn test_new_user_manager_stores_nick() {
    let mgr = UserManager::new("mercury").unwrap();
    assert_eq!(mgr.current_nick(), "mercury");
}

#[test]
fn test_new_user_manager_rejects_invalid_nick() {
    assert!(UserManager::new("1invalid").is_err());
    assert!(UserManager::new("").is_err());
    assert!(UserManager::new("has space").is_err());
}

#[test]
fn test_new_user_manager_no_pending_nick() {
    let mgr = UserManager::new("mercury").unwrap();
    assert!(mgr.pending_nick().is_none());
}

// ---------------------------------------------------------------------------
// Nick change
// ---------------------------------------------------------------------------

#[test]
fn test_request_nick_change_builds_nick_message() {
    let mut mgr = UserManager::new("alice").unwrap();
    let msg = mgr.request_nick_change("bob").unwrap();
    assert_eq!(
        msg,
        OutboundMessage::Nick {
            new_nick: "bob".to_string()
        }
    );
}

#[test]
fn test_request_nick_change_sets_pending() {
    let mut mgr = UserManager::new("alice").unwrap();
    mgr.request_nick_change("bob").unwrap();
    assert_eq!(mgr.pending_nick(), Some("bob"));
}

#[test]
fn test_request_nick_change_rejects_invalid() {
    let mut mgr = UserManager::new("alice").unwrap();
    assert!(mgr.request_nick_change("1bad").is_err());
    assert!(mgr.request_nick_change("").is_err());
    // Pending should not have been set.
    assert!(mgr.pending_nick().is_none());
}

#[test]
fn test_confirm_nick_change_updates_current_nick() {
    let mut mgr = UserManager::new("alice").unwrap();
    mgr.request_nick_change("bob").unwrap();
    mgr.confirm_nick_change("alice", "bob");
    assert_eq!(mgr.current_nick(), "bob");
    assert!(mgr.pending_nick().is_none());
}

#[test]
fn test_confirm_nick_change_case_insensitive_match() {
    let mut mgr = UserManager::new("Alice").unwrap();
    mgr.request_nick_change("Bob").unwrap();
    // Server may echo with different capitalisation.
    mgr.confirm_nick_change("ALICE", "Bob");
    assert_eq!(mgr.current_nick(), "Bob");
}

#[test]
fn test_confirm_nick_change_for_other_user_does_not_change_current() {
    let mut mgr = UserManager::new("alice").unwrap();
    mgr.confirm_nick_change("carol", "dave");
    assert_eq!(mgr.current_nick(), "alice");
}

// ---------------------------------------------------------------------------
// WHOIS caching
// ---------------------------------------------------------------------------

#[test]
fn test_build_whois_message() {
    let mgr = UserManager::new("me").unwrap();
    let msg = mgr.build_whois("alice").unwrap();
    assert_eq!(
        msg,
        OutboundMessage::Whois {
            nick: "alice".to_string()
        }
    );
}

#[test]
fn test_build_whois_rejects_empty_nick() {
    let mgr = UserManager::new("me").unwrap();
    assert!(mgr.build_whois("").is_err());
}

#[test]
fn test_handle_whois_user_caches_info() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_whois_user("alice", "ali", "example.com", "Alice Smith");
    let info = mgr.whois_info("alice").unwrap();
    assert_eq!(info.nick, "alice");
    assert_eq!(info.username, "ali");
    assert_eq!(info.host, "example.com");
    assert_eq!(info.realname, "Alice Smith");
}

#[test]
fn test_handle_whois_server_caches_info() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_whois_server("alice", "irc.example.com", "Example IRC");
    let info = mgr.whois_info("alice").unwrap();
    assert_eq!(info.server, "irc.example.com");
    assert_eq!(info.server_info, "Example IRC");
}

#[test]
fn test_handle_whois_channels_caches_info() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_whois_channels("alice", "#general #dev");
    let info = mgr.whois_info("alice").unwrap();
    assert_eq!(info.channels, vec!["#general", "#dev"]);
}

#[test]
fn test_handle_whois_account_sets_identified() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_whois_account("alice", "alice_account");
    let info = mgr.whois_info("alice").unwrap();
    assert!(info.is_identified);
    assert_eq!(info.account, Some("alice_account".to_string()));
}

#[test]
fn test_whois_lookup_is_case_insensitive() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_whois_user("Alice", "ali", "example.com", "Alice Smith");
    // Query with different capitalisation.
    assert!(mgr.whois_info("ALICE").is_some());
    assert!(mgr.whois_info("alice").is_some());
}

#[test]
fn test_whois_unknown_nick_returns_none() {
    let mgr = UserManager::new("me").unwrap();
    assert!(mgr.whois_info("nobody").is_none());
}

#[test]
fn test_nick_change_invalidates_whois_cache() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_whois_user("alice", "ali", "host", "Alice");
    // alice changes nick to carol — cache for "alice" should be cleared.
    mgr.confirm_nick_change("alice", "carol");
    assert!(mgr.whois_info("alice").is_none());
}

// ---------------------------------------------------------------------------
// WHO
// ---------------------------------------------------------------------------

#[test]
fn test_build_who_message() {
    let mgr = UserManager::new("me").unwrap();
    let msg = mgr.build_who("#general").unwrap();
    assert_eq!(
        msg,
        OutboundMessage::Who {
            mask: "#general".to_string()
        }
    );
}

#[test]
fn test_build_who_rejects_empty_mask() {
    let mgr = UserManager::new("me").unwrap();
    assert!(mgr.build_who("").is_err());
}

#[test]
fn test_handle_who_reply_stores_entry() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_who_reply(
        "alice",
        "ali",
        "host.example",
        "irc.example.com",
        "H",
        "Alice Smith",
    );
    let results = mgr.who_results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].nick, "alice");
    assert_eq!(results[0].username, "ali");
    assert!(!results[0].away);
}

#[test]
fn test_handle_who_reply_away_flag() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_who_reply("bob", "b", "host", "irc.example.com", "G", "Bob");
    assert!(mgr.who_results()[0].away);
}

#[test]
fn test_clear_who_results() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_who_reply("alice", "ali", "host", "irc", "H", "Alice");
    mgr.clear_who_results();
    assert!(mgr.who_results().is_empty());
}

#[test]
fn test_who_reply_replaces_stale_entry_for_same_nick() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.handle_who_reply("alice", "old", "old-host", "irc", "H", "Old");
    mgr.handle_who_reply("alice", "new", "new-host", "irc", "H", "New");
    let results = mgr.who_results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].username, "new");
}

// ---------------------------------------------------------------------------
// NickServ / services
// ---------------------------------------------------------------------------

#[test]
fn test_build_identify() {
    let mgr = UserManager::new("me").unwrap();
    let msg = mgr.build_identify("s3cr3t");
    assert_eq!(
        msg,
        OutboundMessage::NickServ {
            text: "IDENTIFY s3cr3t".to_string()
        }
    );
}

#[test]
fn test_build_register() {
    let mgr = UserManager::new("me").unwrap();
    let msg = mgr.build_register("s3cr3t", "me@example.com");
    assert_eq!(
        msg,
        OutboundMessage::NickServ {
            text: "REGISTER s3cr3t me@example.com".to_string()
        }
    );
}

#[test]
fn test_build_nickserv_arbitrary() {
    let mgr = UserManager::new("me").unwrap();
    let msg = mgr.build_nickserv("GHOST alice hunter2");
    assert_eq!(
        msg,
        OutboundMessage::NickServ {
            text: "GHOST alice hunter2".to_string()
        }
    );
}

// ---------------------------------------------------------------------------
// Regression: nick visible in status bar (App::nick reflects connect-time nick)
// ---------------------------------------------------------------------------

/// Regression test: App::nick() must return the nick supplied at connect time
/// so that the status bar can display it immediately, before any server-side
/// nick change is confirmed.
#[test]
fn test_app_nick_reflects_connect_time_nick() {
    let app = App::new_disconnected("localhost", 6667, "testuser");
    assert_eq!(app.nick(), "testuser");
}

/// App::nick() must update after a confirmed nick change so the status bar
/// always shows the current nick.
#[test]
fn test_app_nick_updates_after_confirmed_change() {
    let mut app = App::new_disconnected("localhost", 6667, "original");
    app.user_mgr.request_nick_change("renamed").unwrap();
    app.user_mgr.confirm_nick_change("original", "renamed");
    assert_eq!(app.nick(), "renamed");
}

// ---------------------------------------------------------------------------
// NickServStatus transitions
// ---------------------------------------------------------------------------

#[test]
fn test_nickserv_status_default_is_unregistered() {
    let mgr = UserManager::new("me").unwrap();
    assert_eq!(mgr.nickserv_status(), NickServStatus::Unregistered);
}

#[test]
fn test_set_nickserv_status_unauthenticated() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.set_nickserv_status(NickServStatus::Unauthenticated);
    assert_eq!(mgr.nickserv_status(), NickServStatus::Unauthenticated);
}

#[test]
fn test_set_nickserv_status_authenticated() {
    let mut mgr = UserManager::new("me").unwrap();
    mgr.set_nickserv_status(NickServStatus::Authenticated);
    assert_eq!(mgr.nickserv_status(), NickServStatus::Authenticated);
}

/// Progression: Unregistered → Unauthenticated → Authenticated
#[test]
fn test_nickserv_status_full_progression() {
    let mut mgr = UserManager::new("me").unwrap();
    assert_eq!(mgr.nickserv_status(), NickServStatus::Unregistered);

    mgr.set_nickserv_status(NickServStatus::Unauthenticated);
    assert_eq!(mgr.nickserv_status(), NickServStatus::Unauthenticated);

    mgr.set_nickserv_status(NickServStatus::Authenticated);
    assert_eq!(mgr.nickserv_status(), NickServStatus::Authenticated);
}

/// Own nick change must reset NickServ status to Unregistered.
#[test]
fn test_nickserv_status_resets_on_own_nick_change() {
    let mut mgr = UserManager::new("alice").unwrap();
    mgr.set_nickserv_status(NickServStatus::Authenticated);
    assert_eq!(mgr.nickserv_status(), NickServStatus::Authenticated);

    // Simulate own nick change confirmed by server.
    mgr.request_nick_change("alice2").unwrap();
    mgr.confirm_nick_change("alice", "alice2");

    assert_eq!(mgr.nickserv_status(), NickServStatus::Unregistered);
}

/// Another user's nick change must NOT reset our NickServ status.
#[test]
fn test_nickserv_status_not_reset_on_other_nick_change() {
    let mut mgr = UserManager::new("alice").unwrap();
    mgr.set_nickserv_status(NickServStatus::Authenticated);

    // Someone else changes nick — should not affect our auth status.
    mgr.confirm_nick_change("carol", "dave");

    assert_eq!(mgr.nickserv_status(), NickServStatus::Authenticated);
}
