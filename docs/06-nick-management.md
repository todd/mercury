# Nick Management

## Changing your nick

```
/nick <new_nick>
```

Request a nick change. Mercury validates the nick locally before sending it
to the server. Valid nicks must:

- Be 1–30 characters long
- Start with a letter or one of: `[`, `]`, `\`, `^`, `_`, `{`, `|`, `}`
- Contain only letters, digits, hyphens, or the characters listed above

If the nick fails local validation:

```
Error: 'foo bar' is not a valid nickname.
```

If the nick passes local validation but is already in use on the server:

```
Error: nickname 'alice' is already in use.
```

When the server confirms the change, you will see in the server buffer:

```
You are now known as alice
```

And the status bar will update to reflect the new nick.

**Note:** a nick change always resets your NickServ authentication status to
`(unregistered)`. You will need to identify again with `/ns IDENTIFY` after
changing nicks.

## NickServ

```
/ns <command>
```

Alias: `/nickserv`

Send a command to NickServ, the network service that manages nick
registration and authentication. Everything after `/ns` is forwarded verbatim
as a message to NickServ.

Running `/ns` with no arguments prints a brief reminder:

```
NickServ commands:
  /ns IDENTIFY <password>
  /ns REGISTER <password> <email>
  /ns GHOST <nick> <password>
  /ns INFO <nick>
```

### Identifying

```
/ns IDENTIFY <password>
```

Authenticate with NickServ for your current nick. On success the status bar
will change to `(authenticated)`. Mercury detects the confirmation
automatically by parsing NickServ's NOTICE response — you do not need to do
anything else.

### Registering a nick

```
/ns REGISTER <password> <email>
```

Register your current nick with NickServ. You only need to do this once.
After registration, use `/ns IDENTIFY` at the start of each session.

### Reclaiming a ghosted nick

```
/ns GHOST <nick> <password>
```

If your nick is already in use by a stale or disconnected session, GHOST
forces that session off the network so you can reclaim the nick.

### Looking up a nick

```
/ns INFO <nick>
```

Display NickServ's registration information for a nick in the server buffer.

## NickServ authentication status

The status bar always reflects your current authentication state:

| Status | Meaning |
|---|---|
| `(authenticated)` | NickServ has accepted your password for this nick |
| `(unauthenticated)` | The server knows this nick is registered, but you have not yet identified |
| `(unregistered)` | No NickServ registration information has been received for this nick |

Mercury tracks status changes automatically by parsing NickServ NOTICE
messages. You do not need to run any command to refresh it.

Any nick change — whether via `/nick` or because a fallback nick was used
during connection — resets the status to `(unregistered)`.

## Alternate nicks on connect

If your chosen nick is already in use when you connect, Mercury automatically
tries the following fallback nicks in order: `nick_`, `nick__`, `nick_1`,
`nick_2`. If you end up on a fallback nick, use `/nick` to switch to your
preferred one once connected, then `/ns IDENTIFY` to authenticate.
