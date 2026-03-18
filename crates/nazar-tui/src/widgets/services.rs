use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use nazar_core::{ServiceState, ServiceStatus};

pub fn render(frame: &mut Frame, area: Rect, services: &[ServiceStatus]) {
    let block = Block::default()
        .title(Span::styled(
            " Services ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if services.is_empty() {
        let p = Paragraph::new("  No service data (daimon not connected)")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let lines: Vec<Line> = services
        .iter()
        .map(|svc| {
            let (icon_color, icon) = match svc.state {
                ServiceState::Running => (Color::Green, "●"),
                ServiceState::Failed => (Color::Red, "●"),
                ServiceState::Stopped => (Color::DarkGray, "●"),
                ServiceState::Starting => (Color::Yellow, "●"),
                ServiceState::Unknown => (Color::DarkGray, "?"),
            };

            let mut spans = vec![
                Span::raw("  "),
                Span::styled(format!("{icon} "), Style::default().fg(icon_color)),
                Span::styled(
                    format!("{:<12} ", svc.name),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("{:<8}", svc.state),
                    Style::default().fg(icon_color),
                ),
            ];

            if let Some(port) = svc.port {
                spans.push(Span::styled(
                    format!(" :{port}"),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            if let Some(up) = svc.uptime_secs {
                let h = up / 3600;
                let m = (up % 3600) / 60;
                spans.push(Span::styled(
                    format!("  up {h}h{m}m"),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        })
        .collect();

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}
