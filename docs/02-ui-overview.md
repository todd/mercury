# UI Overview

Mercury's interface is divided into four fixed areas.

```
┌─────────────────────────────────────────────────────────────────┐
│ mercury │ irc.libera.chat — alice (authenticated) │ ● connected [tls] │
├──────────────────┬──────────────────────────────────┬───────────┤
│ server           │  #general                        │ users (4) │
│ Channels         │                                  │  @op      │
│  #general        │  <alice> hello everyone          │  +voiced  │
│  #dev            │    bob joined #general           │  alice    │
│ Messages         │  <bob> hey!                      │  bob      │
│  carol           │                                  │           │
├──────────────────┴──────────────────────────────────┴───────────┤
│  /join #general█                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Status bar (top)

The single row at the top of the screen shows the current state of your
connection at a glance.

**App name** — `mercury` in bold, always present.

**Server and nick** — shown as `hostname — nick` once connected, for example
`irc.libera.chat — alice`. Not shown when disconnected.

**NickServ auth status** — shown in parentheses after your nick:

| Badge | Meaning |
|---|---|
| `(authenticated)` | NickServ has accepted your credentials |
| `(unauthenticated)` | The server recognises your nick is registered, but you have not identified |
| `(unregistered)` | No NickServ registration information has been received |

The badge resets to `(unregistered)` whenever your nick changes. See
[06-nick-management.md](06-nick-management.md) for details.

**Connection indicator** — a symbol and label on the right:

| Indicator | Meaning |
|---|---|
| `● connected` (green) | Active connection |
| `○ disconnected` (red) | No connection |
| `◌ connecting…` (yellow) | Handshake in progress |
| `◌ disconnecting…` (yellow) | Graceful disconnect in progress |

**TLS indicator** — shown next to the connection state while connected or
connecting:

| Badge | Meaning |
|---|---|
| `[tls]` (green) | Connection is encrypted |
| `[plain]` (yellow) | Connection is unencrypted |

**Status message area** — transient messages (errors, confirmations) appear in
dark gray on the right side of the status bar. Press Escape to dismiss.

## Navigation list (left panel)

The left panel is 22 columns wide and lists every buffer you can switch to.

- **`server`** — always at the top; shows connection messages, MOTD, and
  server numerics.
- **Channels** section — each channel you have joined, sorted alphabetically.
- **Messages** section — each open private-message conversation, sorted
  alphabetically.

The active buffer is shown in bold cyan. Use `Alt+Up` and `Alt+Down` to move
between entries, or click an entry if your terminal supports mouse input.

## Message pane (centre)

The centre pane displays the buffer for the currently selected entry.

**Chat messages** appear as `<nick> text`, with the nick in bold green.

**System messages** (join/part notices, server responses, errors) appear
indented with two spaces in yellow.

The pane title at the top shows the buffer name. When you have scrolled up and
are no longer viewing the latest messages, `[scrolled]` is appended to the
title as a reminder.

Each buffer has its own independent scroll position, which is preserved when
you switch away and come back.

## User list (right panel)

The right panel (22 columns wide) is only shown when a **channel** is the
active buffer. When you are viewing the server buffer or a private-message
conversation, the message pane expands to fill the full width.

The panel title shows `users (N)` where N is the current member count.

Members are listed in order:

1. Operators (`@` prefix), alphabetical
2. Voiced members (`+` prefix), alphabetical
3. Regular members (no prefix), alphabetical

Your own nick is highlighted in bold cyan.

## Input bar (bottom)

The input bar is three rows tall and sits at the bottom of the screen.

A prompt indicator on the left changes based on what you are typing:

- Yellow `  ` — you have typed a `/`, indicating a command
- Cyan `  ` — plain text, will be sent as a chat message

Your text is shown in white with a blinking cyan block cursor at the end.

Press Enter to submit. Press Escape to clear the input and dismiss any status
message.
