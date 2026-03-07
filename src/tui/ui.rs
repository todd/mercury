/// Ratatui rendering: layout, widgets, and draw calls.
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use super::app::{App, BufferLine};
use crate::irc::client::ClientState;

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

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Root draw function — called on every tick / event.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // ┌─────────────────────────────┐
    // │  status bar (1 line)        │
    // ├───────────┬─────────────────┤
    // │ channels  │  message pane   │
    // │  (20 col) │                 │
    // ├───────────┴─────────────────┤
    // │  input bar (3 lines)        │
    // └─────────────────────────────┘

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // status bar
            Constraint::Min(0),     // main area
            Constraint::Length(3),  // input bar
        ])
        .split(area);

    let status_area = vertical[0];
    let main_area = vertical[1];
    let input_area = vertical[2];

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22), // channel list
            Constraint::Min(0),     // message pane
        ])
        .split(main_area);

    let channel_list_area = horizontal[0];
    let message_area = horizontal[1];

    draw_status_bar(frame, app, status_area);
    draw_channel_list(frame, app, channel_list_area);
    draw_message_pane(frame, app, message_area);
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

    let server_info = if let Some(srv) = app.client.current_server() {
        format!(" {} ", srv)
    } else {
        " mercury ".to_string()
    };

    let status_msg = app
        .status_message
        .as_deref()
        .unwrap_or("");

    let spans = vec![
        Span::styled(" mercury ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("│"),
        Span::styled(server_info, Style::default().fg(Color::White)),
        Span::raw("│"),
        Span::styled(format!(" {} ", state_str), Style::default().fg(state_color)),
        Span::raw("│"),
        Span::styled(format!(" {} ", status_msg), Style::default().fg(Color::DarkGray)),
    ];

    let paragraph = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::Black));
    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Channel list (left panel)
// ---------------------------------------------------------------------------

fn draw_channel_list(frame: &mut Frame, app: &App, area: Rect) {
    let channels = app.sorted_joined_channels();
    let active = app.active_channel.as_deref();

    let mut items: Vec<ListItem> = Vec::new();

    // Server buffer entry
    let server_style = if active.is_none() {
        Style::default().fg(COLOR_CHANNEL_ACTIVE).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_CHANNEL_INACTIVE)
    };
    items.push(ListItem::new(Span::styled(" server", server_style)));

    for ch in &channels {
        let style = if Some(ch.as_str()) == active {
            Style::default().fg(COLOR_CHANNEL_ACTIVE).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_CHANNEL_INACTIVE)
        };
        items.push(ListItem::new(Span::styled(format!(" {}", ch), style)));
    }

    let border_style = Style::default().fg(COLOR_BORDER);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(" channels ", Style::default().fg(Color::DarkGray)));

    let list = List::new(items).block(block).style(Style::default().bg(COLOR_BG));
    frame.render_widget(list, area);
}

// ---------------------------------------------------------------------------
// Message pane (right panel)
// ---------------------------------------------------------------------------

fn draw_message_pane(frame: &mut Frame, app: &App, area: Rect) {
    let lines = app.active_lines();
    let title = app
        .active_channel
        .as_deref()
        .unwrap_or("server");

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
            Style::default().fg(COLOR_ACTIVE_BORDER).add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(rendered)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((
            // Scroll to bottom: compute offset
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

    let input_span = Span::styled(
        app.input.as_str(),
        Style::default().fg(Color::White),
    );

    let cursor = Span::styled("█", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_BORDER))
        .title(Span::styled(" input ", Style::default().fg(Color::DarkGray)));

    let paragraph = Paragraph::new(Line::from(vec![prompt, input_span, cursor])).block(block);
    frame.render_widget(paragraph, area);
}
