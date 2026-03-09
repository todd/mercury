# Getting Started with Mercury

Mercury is a terminal IRC client. It runs entirely inside your terminal — no
graphical window, no browser, no config file to edit before first use.
Everything is driven interactively from within the app itself.

## Building and running

```
cargo build --release
./target/release/mercury
```

Or, during development:

```
cargo run
```

Mercury takes no command-line arguments. On launch it switches your terminal
into full-screen mode and you're ready to go.

If you want to see low-level debug output, set `RUST_LOG=debug` before
running. Logs are written to stderr and will not interfere with the UI:

```
RUST_LOG=debug ./target/release/mercury
```

## Your first session

Here is the shortest path from launch to chatting.

### 1. Connect to a server

Type the following into the input bar at the bottom of the screen and press
Enter:

```
/connect irc.libera.chat
```

Mercury connects over TLS on port 6697 by default. You will see the server's
message of the day appear in the centre pane, and the status bar at the top
will change to show `● connected [tls]`.

Your nickname defaults to `mercury`. To choose your own from the start:

```
/connect irc.libera.chat 6697 yournick
```

### 2. Join a channel

```
/join #mercury
```

The channel appears in the left-hand list and its message pane opens in the
centre. Members are listed on the right.

### 3. Send a message

Type anything that does not start with `/` and press Enter — it is sent as a
chat message to the current channel.

### 4. Quit

```
/quit
```

Mercury sends a QUIT to the server and restores your terminal.

## What's next

- [02-ui-overview.md](02-ui-overview.md) — understand every element on screen
- [03-connecting.md](03-connecting.md) — all `/connect` options, TLS, and plaintext
- [04-channels.md](04-channels.md) — joining, creating, and leaving channels
- [05-messaging.md](05-messaging.md) — chat, private messages, and looking up users
- [06-nick-management.md](06-nick-management.md) — changing your nick and authenticating with NickServ
- [07-keyboard-reference.md](07-keyboard-reference.md) — every keyboard shortcut at a glance
