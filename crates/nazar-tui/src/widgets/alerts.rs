use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use nazar_core::{Alert, AlertSeverity};

pub fn render(frame: &mut Frame, area: Rect, alerts: &[Alert], scroll: u16) {
    let block = Block::default()
        .title(Span::styled(
            format!(" Alerts ({}) ", alerts.len()),
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if alerts.is_empty() {
        let p = Paragraph::new("  No active alerts").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let lines: Vec<Line> = alerts
        .iter()
        .rev()
        .map(|alert| {
            let (sev_color, sev_label) = match alert.severity {
                AlertSeverity::Critical => (Color::Red, "CRIT"),
                AlertSeverity::Warning => (Color::Yellow, "WARN"),
                AlertSeverity::Info => (Color::DarkGray, "INFO"),
            };

            let age = chrono::Utc::now() - alert.timestamp;
            let age_str = if age.num_hours() > 0 {
                format!("{}h ago", age.num_hours())
            } else if age.num_minutes() > 0 {
                format!("{}m ago", age.num_minutes())
            } else {
                format!("{}s ago", age.num_seconds())
            };

            Line::from(vec![
                Span::styled(
                    format!(" [{sev_label}] "),
                    Style::default().fg(sev_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}: ", alert.component),
                    Style::default().fg(Color::White),
                ),
                Span::styled(alert.message.clone(), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("  {age_str}"), Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let p = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(p, inner);
}
