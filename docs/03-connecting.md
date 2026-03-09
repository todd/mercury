# Connecting to a Server

## `/connect`

```
/connect <server> [port] [nick] [--plain] [--no-verify]
```

Connects to an IRC server. You can run `/connect` again at any time — it
resets the current session and opens a fresh connection.

### Arguments

| Argument | Required | Description |
|---|---|---|
| `<server>` | Yes | Hostname or IP address of the IRC server |
| `[port]` | No | Port number. Defaults to `6697` (TLS) or `6667` (plain) |
| `[nick]` | No | Desired nickname. Defaults to `mercury` |

The positional arguments are order-sensitive and must appear before any flags.

### Flags

**`--plain`**

Connect over unencrypted TCP instead of TLS. Changes the default port from
6697 to 6667. The status bar will show `[plain]` in yellow as a persistent
reminder that the connection is not encrypted. Only use this when the server
does not support TLS.

```
/connect irc.example.com --plain
/connect irc.example.com 6667 mynick --plain
```

**`--no-verify`**

Disable TLS certificate verification. The server's certificate will not be
checked against trusted certificate authorities, and hostname verification is
also skipped.

This flag is not recommended for day-to-day use. It leaves you open to
man-in-the-middle attacks. When it is active, a prominent warning is displayed
in the server buffer:

```
⚠ TLS certificate verification disabled
```

It exists primarily for connecting to self-signed or development servers where
you already have another means of verifying trust. If you find yourself using
it routinely, consider installing a valid certificate on the server instead.

### Non-configurable fields

The following fields are set by Mercury and are not currently user-configurable:

| Field | Value |
|---|---|
| `username` | Same as your nick |
| `realname` | `Mercury IRC Client` |

### Examples

```
/connect irc.libera.chat
```
Connect to Libera.Chat over TLS on port 6697 with the default nick `mercury`.

```
/connect irc.libera.chat 6697 alice
```
Same, but with the nick `alice`.

```
/connect irc.example.com 6667 alice --plain
```
Connect over plaintext on port 6667.

```
/connect localhost 6668 testbot --plain --no-verify
```
Connect to a local development server.

### Connection feedback

While connecting you will see in the server buffer:

```
Connecting to irc.libera.chat:6697 [tls]…
```

On success:

```
Connected to irc.libera.chat:6697 [tls]
```

On failure:

```
Connection failed: <error detail>
```

The status bar connection indicator transitions through `◌ connecting…` →
`● connected` (or back to `○ disconnected` on failure).

### Alternate nicks

If your chosen nick is already taken during the connection handshake, Mercury
automatically tries fallback nicks in this order: `nick_`, `nick__`, `nick_1`,
`nick_2`. You can change to your preferred nick once connected with `/nick` —
see [06-nick-management.md](06-nick-management.md).

---

## `/disconnect`

```
/disconnect
```

Gracefully disconnect from the current server. Sends an IRC QUIT with the
message `Mercury IRC Client` and closes the connection. Safe to run when
already disconnected.

---

## `/quit`

```
/quit
```

Disconnect from the server and exit Mercury entirely. Your terminal is
restored to its normal state.
