/// Ratatui rendering: layout, widgets, and draw calls.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, BufferLine, MemberEntry};
use super::layout::word_wrap_line_count;
use crate::irc::client::ClientState;
use crate::irc::user::NickServStatus;

// ---------------------------------------------------------------------------
// Colour palette
// ---------------------------------------------------------------------------

const COLOR_BG: Color = Color::Reset;
const COLOR_BORDER: Color = Color::DarkGray;
const COLOR_ACTIVE_BORDER: Color = Color::Cyan;
const COLOR_SYSTEM_MSG: Color = Color::Yellow;
const COLOR_NICK: Color = Color::Green;
const COLOR_STATUS_CONNECTED: Color = Color::Green;
const COLOR_STATUS_DISCONNECTED: Color = Color::Red;
const COLOR_STATUS_TRANSIENT: Color = Color::Yellow;
const COLOR_CHANNEL_ACTIVE: Color = Color::Cyan;
const COLOR_CHANNEL_INACTIVE: Color = Color::White;
const COLOR_SECTION_HEADER: Color = Color::DarkGray;

/// Width of the channel list and user list panels (columns).
const PANEL_WIDTH: u16 = 22;

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Root draw function — called on every tick / event.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Vertical split: status bar / main area / input bar
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(0),    // main area
            Constraint::Length(3), // input bar
        ])
        .split(area);

    let status_area = vertical[0];
    let main_area = vertical[1];
    let input_area = vertical[2];

    // Horizontal split depends on whether the active buffer is a channel.
    // Channel view: channel-list | message | user-list
    // Otherwise:   channel-list | message
    if app.active_is_channel() {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(PANEL_WIDTH), // channel list
                Constraint::Min(0),              // message pane
                Constraint::Length(PANEL_WIDTH), // user list
            ])
            .split(main_area);

        draw_channel_list(frame, app, horizontal[0]);
        draw_message_pane(frame, app, horizontal[1]);
        draw_user_list(frame, app, horizontal[2]);
    } else {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(PANEL_WIDTH), // channel list
                Constraint::Min(0),              // message pane
            ])
            .split(main_area);

        draw_channel_list(frame, app, horizontal[0]);
        draw_message_pane(frame, app, horizontal[1]);
    }

    draw_status_bar(frame, app, status_area);
    draw_input_bar(frame, app, input_area);
}

// ---------------------------------------------------------------------------
// Status bar (top)
// ---------------------------------------------------------------------------

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let state = app.connection_state();
    let (state_str, state_color) = match state {
        ClientState::Connected => ("● connected", COLOR_STATUS_CONNECTED),
        ClientState::Disconnected => ("○ disconnected", COLOR_STATUS_DISCONNECTED),
        ClientState::Connecting => ("◌ connecting…", COLOR_STATUS_TRANSIENT),
        ClientState::Disconnecting => ("◌ disconnecting…", COLOR_STATUS_TRANSIENT),
    };

    // Build the nick + auth-status segment.
    // Format: " server — alice (authenticated) " or just " alice " when not connected to a server.
    let (auth_str, auth_color) = match app.nickserv_status() {
        NickServStatus::Authenticated => (" (authenticated)", COLOR_STATUS_CONNECTED),
        NickServStatus::Unauthenticated => (" (unauthenticated)", Color::Yellow),
        NickServStatus::Unregistered => (" (unregistered)", Color::DarkGray),
    };

    let server_part = if let Some(srv) = app.client.current_server() {
        format!(" {} — {}", srv, app.nick())
    } else {
        format!(" {}", app.nick())
    };

    // TLS indicator: only shown while connected or connecting.
    // [tls] in green, [plain] in yellow to draw attention to the insecure case.
    let tls_span = match state {
        ClientState::Connected | ClientState::Connecting | ClientState::Disconnecting => {
            if app.client.is_tls() {
                Some(Span::styled(" [tls]", Style::default().fg(COLOR_STATUS_CONNECTED)))
            } else {
                Some(Span::styled(" [plain]", Style::default().fg(Color::Yellow)))
            }
        }
        ClientState::Disconnected => None,
    };

    let status_msg = app.status_message.as_deref().unwrap_or("");

    let mut spans = vec![
        Span::styled(
            " mercury ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("│"),
        Span::styled(server_part, Style::default().fg(Color::White)),
        Span::styled(auth_str, Style::default().fg(auth_color)),
        Span::styled(" ", Style::default()),
        Span::raw("│"),
        Span::styled(format!(" {} ", state_str), Style::default().fg(state_color)),
    ];

    if let Some(s) = tls_span {
        spans.push(s);
        spans.push(Span::raw(" "));
    }

    spans.push(Span::raw("│"));
    spans.push(Span::styled(
        format!(" {} ", status_msg),
        Style::default().fg(Color::DarkGray),
    ));

    let paragraph = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black));
    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Channel list (left panel)
// ---------------------------------------------------------------------------

fn draw_channel_list(frame: &mut Frame, app: &App, area: Rect) {
    let channels = app.sorted_joined_channels();
    let active = app.active_channel.as_deref();
    let private_chats = &app.private_chats;

    let mut items: Vec<ListItem> = Vec::new();

    // --- Server entry (no section header, always top) ---
    let server_style = if active.is_none() {
        Style::default()
            .fg(COLOR_CHANNEL_ACTIVE)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_CHANNEL_INACTIVE)
    };
    items.push(ListItem::new(Span::styled(" server", server_style)));

    // --- Channels section ---
    if !channels.is_empty() {
        items.push(ListItem::new(Span::styled(
            "Channels",
            Style::default().fg(COLOR_SECTION_HEADER),
        )));
        for ch in &channels {
            let style = if Some(ch.as_str()) == active {
                Style::default()
                    .fg(COLOR_CHANNEL_ACTIVE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_CHANNEL_INACTIVE)
            };
            items.push(ListItem::new(Span::styled(format!(" {}", ch), style)));
        }
    }

    // --- Messages (PM) section ---
    if !private_chats.is_empty() {
        items.push(ListItem::new(Span::styled(
            "Messages",
            Style::default().fg(COLOR_SECTION_HEADER),
        )));
        for pm in private_chats {
            let style = if Some(pm.as_str()) == active {
                Style::default()
                    .fg(COLOR_CHANNEL_ACTIVE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_CHANNEL_INACTIVE)
            };
            items.push(ListItem::new(Span::styled(format!(" {}", pm), style)));
        }
    }

    let border_style = Style::default().fg(COLOR_BORDER);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            " channels ",
            Style::default().fg(Color::DarkGray),
        ));

    let list = List::new(items)
        .block(block)
        .style(Style::default().bg(COLOR_BG));
    frame.render_widget(list, area);
}

// ---------------------------------------------------------------------------
// User list (right panel — only shown when a channel is active)
// ---------------------------------------------------------------------------

fn draw_user_list(frame: &mut Frame, app: &App, area: Rect) {
    let members = app.active_channel_members();
    let own_nick = app.nick().to_lowercase();

    let items: Vec<ListItem> = members
        .iter()
        .map(|m| render_member(m, &own_nick))
        .collect();

    let count = members.len();
    let border_style = Style::default().fg(COLOR_BORDER);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            format!(" users ({}) ", count),
            Style::default().fg(Color::DarkGray),
        ));

    let list = List::new(items)
        .block(block)
        .style(Style::default().bg(COLOR_BG));
    frame.render_widget(list, area);
}

fn render_member(m: &MemberEntry, own_nick_lower: &str) -> ListItem<'static> {
    let prefix = if m.is_op {
        "@"
    } else if m.is_voiced {
        "+"
    } else {
        " "
    };

    let is_self = m.nick.to_lowercase() == own_nick_lower;
    let style = if is_self {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_CHANNEL_INACTIVE)
    };

    ListItem::new(Span::styled(format!("{}{}", prefix, m.nick), style))
}

// ---------------------------------------------------------------------------
// Message pane (centre)
// ---------------------------------------------------------------------------

/// Count the number of terminal rows a single rendered `Line` occupies when
/// word-wrapped inside a pane of `inner_width` columns.
///
/// Delegates to [`word_wrap_line_count`] which mirrors ratatui's
/// `Wrap { trim: false }` algorithm exactly, so the scroll offset calculation
/// stays in sync with what ratatui actually renders.
fn rendered_row_count(line: &Line, inner_width: usize) -> usize {
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    word_wrap_line_count(&text, inner_width)
}

fn draw_message_pane(frame: &mut Frame, app: &mut App, area: Rect) {
    // Record the visible height so key handlers can issue page-sized scrolls.
    app.message_pane_height = area.height;

    // Collect everything we need from shared borrows before the mutable call.
    let title = app
        .active_channel
        .as_deref()
        .unwrap_or("server")
        .to_string();
    let rendered: Vec<Line> = app
        .active_lines()
        .iter()
        .map(|line| match line {
            BufferLine::System(s) => Line::from(Span::styled(
                format!("  {}", s),
                Style::default().fg(COLOR_SYSTEM_MSG),
            )),
            BufferLine::Chat(m) => Line::from(vec![
                Span::styled(
                    format!(" <{}>", m.nick),
                    Style::default().fg(COLOR_NICK).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", m.text)),
            ]),
        })
        .collect();

    // Inner width of the pane (subtract the two border columns).
    let inner_width = (area.width as usize).saturating_sub(2);
    // Number of visible rows inside the pane (subtract top and bottom borders).
    let visible_rows = (area.height as usize).saturating_sub(2);

    // Total rendered rows, accounting for wrapped lines.
    let total_rows: usize = rendered
        .iter()
        .map(|l| rendered_row_count(l, inner_width))
        .sum();

    // Maximum possible scroll offset (can't scroll past the top).
    let max_offset = total_rows.saturating_sub(visible_rows);
    // User's requested offset (rows above the bottom); clamped to the ceiling.
    let scroll_offset = app.active_scroll_offset().min(max_offset);
    // Final ratatui scroll row: how many rows from the *top* to skip.
    let scroll_row = max_offset.saturating_sub(scroll_offset) as u16;

    // Show a [scrolled] indicator in the title when not at the live bottom.
    let is_scrolled = scroll_offset > 0;
    let title_text = if is_scrolled {
        format!(" {} [scrolled] ", title)
    } else {
        format!(" {} ", title)
    };

    let border_style = Style::default().fg(COLOR_ACTIVE_BORDER);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            title_text,
            Style::default()
                .fg(COLOR_ACTIVE_BORDER)
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(rendered)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_row, 0));

    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Input bar (bottom)
// ---------------------------------------------------------------------------

fn draw_input_bar(frame: &mut Frame, app: &App, area: Rect) {
    let prompt = if app.input.starts_with('/') {
        Span::styled("  ", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("  ", Style::default().fg(Color::Cyan))
    };

    let input_span = Span::styled(app.input.as_str(), Style::default().fg(Color::White));

    let cursor = Span::styled(
        "█",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::SLOW_BLINK),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_BORDER))
        .title(Span::styled(
            " input ",
            Style::default().fg(Color::DarkGray),
        ));

    let paragraph = Paragraph::new(Line::from(vec![prompt, input_span, cursor])).block(block);
    frame.render_widget(paragraph, area);
}
