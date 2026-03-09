# Channels

## Joining a channel

```
/join <#channel>
```

Sends a JOIN request to the server. The channel appears in the left-hand
navigation list and its buffer becomes the active view. While the join is in
progress you will see:

```
Joining #channel…
```

Once the server confirms the join, the channel's member list is populated from
the server's `NAMES` reply.

## Creating a channel

```
/create <#channel>
```

On IRC, creating a channel and joining one are the same operation — the first
person to JOIN a channel that does not yet exist brings it into being. `/create`
sends an identical JOIN request to the server and behaves exactly the same way
as `/join`. It is provided as a distinct command because the intent is
conceptually different, but there is no protocol difference between the two.

## Leaving a channel

```
/part [#channel] [reason]
```

Alias: `/leave`

Leave a channel. If you omit `#channel`, Mercury parts the channel you are
currently viewing. An optional reason string is sent as the PART message and
will be visible to other members.

```
/part
/part #general
/part #general heading out for the night
```

If you part the channel you are currently viewing, the active buffer switches
to the server buffer.

## Channel name rules

Channel names must:

- Start with `#` or `&`
- Be between 2 and 200 characters long
- Not contain: NUL, BEL, CR, LF, space, or comma

Mercury validates the name locally before sending it to the server and will
show an error if these rules are not met.

## The member list

The right-hand panel shows the members of the active channel. The title
displays the current count: `users (N)`.

Members are sorted into three groups, each alphabetical within the group:

1. **`@` Operators** — channel operators
2. **`+` Voiced** — members with voice privilege
3. **Regular members** — no prefix

Your own nick is highlighted in bold cyan.

The member list is rebuilt from scratch when you join a channel, and kept up
to date as members join, leave, or change nick while you are present.

## In-channel events

Mercury displays system messages in the channel buffer as events occur:

| Event | Message shown |
|---|---|
| You join | `  You joined #channel` |
| Someone else joins | `  nick joined #channel` |
| You part | (shown in server buffer: `You left #channel`) |
| Someone else parts | `  nick left #channel` |
| Someone else parts with a reason | `  nick left #channel: reason` |
| A nick changes | `  oldnick is now known as newnick` |

## Scrolling through history

Each channel buffer has its own independent scroll position. Use the arrow
keys or Page Up / Page Down to scroll. When you are not at the bottom of the
buffer, the pane title changes to `#channel [scrolled]` as a reminder.

Switching to a different buffer and back preserves your scroll position. See
[07-keyboard-reference.md](07-keyboard-reference.md) for all scroll keys.
