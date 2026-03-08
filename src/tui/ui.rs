/// Ratatui rendering: layout, widgets, and draw calls.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, BufferLine, MemberEntry};
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
pub fn draw(frame: &mut Frame, app: &App) {
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

    let status_msg = app.status_message.as_deref().unwrap_or("");

    let spans = vec![
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
        Span::raw("│"),
        Span::styled(
            format!(" {} ", status_msg),
            Style::default().fg(Color::DarkGray),
        ),
    ];

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

fn draw_message_pane(frame: &mut Frame, app: &App, area: Rect) {
    let lines = app.active_lines();
    let title = app.active_channel.as_deref().unwrap_or("server");

    let rendered: Vec<Line> = lines
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

    let border_style = Style::default().fg(COLOR_ACTIVE_BORDER);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(COLOR_ACTIVE_BORDER)
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(rendered)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((
            lines.len().saturating_sub(area.height as usize - 2) as u16,
            0,
        ));

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
