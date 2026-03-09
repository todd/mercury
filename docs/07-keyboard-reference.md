# Keyboard Reference

## Input

| Key | Action |
|---|---|
| Any printable character | Append to input |
| `Backspace` | Delete last character |
| `Enter` | Submit input (send message or run command) |
| `Escape` | Clear input and dismiss any status message |

## Navigation

| Key | Action |
|---|---|
| `Alt+Up` | Switch to previous buffer |
| `Alt+Down` | Switch to next buffer |

Buffer order: `server` → channels (alphabetical) → private messages
(alphabetical) → wraps back to `server`.

## Scrolling

| Key | Action |
|---|---|
| `Up arrow` | Scroll message pane up one line |
| `Down arrow` | Scroll message pane down one line |
| `Page Up` | Scroll message pane up one page |
| `Page Down` | Scroll message pane down one page |

Each buffer has its own independent scroll position. When a buffer is not
scrolled to the bottom, its title shows `[scrolled]`.

## Quitting

| Key | Action |
|---|---|
| `Ctrl-C` | Force quit |
| `Ctrl-Q` | Force quit |

Both keys disconnect from the server immediately and restore the terminal.
For a graceful exit, use the `/quit` command instead.
