# Messaging

## Sending a chat message

Type any text that does not begin with `/` into the input bar and press Enter.
The message is sent to the currently active channel and appears in the buffer
as:

```
<yournick> your message here
```

If you are not currently in a channel (e.g. the server buffer is active), pressing
Enter will show an error:

```
not in a channel — use /join #channel
```

## Private messages

```
/msg <nick> <text>
```

Send a private message to another user. A dedicated PM buffer for that
conversation opens in the navigation list under the **Messages** section. Your
message is echoed in the buffer with `me` as the sender label.

When the other user replies, their message appears in the same buffer. If they
initiate a conversation with you first, Mercury opens a buffer for them
automatically.

PM buffers behave the same as channel buffers — they have their own scroll
position, preserved when you switch away. Use `Alt+Up` / `Alt+Down` to navigate
to them.

## The server buffer

The `server` entry at the top of the navigation list is a special buffer that
collects messages not associated with a specific channel or conversation:

- The server's **message of the day** (MOTD) on connect
- Connection and disconnection events
- Responses to `/whois` and `/who` queries
- IRC numeric replies that do not have a more specific destination
- Nick change confirmations for your own nick

## Looking up users

### `/whois <nick>`

```
/whois <nick>
```

Request identity information about a user from the server. Results appear in
the server buffer as they arrive:

```
[whois] alice (ali@example.com) — Alice Smith
[whois] alice via irc.example.com (Example IRC Server)
[whois] alice channels: #general #dev
```

Mercury caches WHOIS results in memory, keyed by nick. The cache entry is
automatically invalidated if that user changes their nick while you are
connected.

### `/who [mask]`

```
/who [mask]
```

Send a WHO query. The `mask` is optional and defaults to `*` (all visible
users). Results appear in the server buffer:

```
[who] alice  ali@host.example  Alice Smith
[who] bob (away)  bob@other.host  Bob
```

Away status is detected automatically from the server's WHO reply and shown in
parentheses. Each new `/who` query clears the previous results before
displaying the new ones.
