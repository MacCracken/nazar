use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use nazar_core::ThermalInfo;

fn temp_color(t: &ThermalInfo) -> Color {
    if t.critical_celsius.is_some_and(|c| t.temp_celsius > c * 0.9) {
        Color::Red
    } else if t.temp_celsius > 70.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

pub fn render(frame: &mut Frame, area: Rect, temps: &[ThermalInfo]) {
    let block = Block::default()
        .title(Span::styled(
            " Temperatures ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if temps.is_empty() {
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = temps
        .iter()
        .map(|t| {
            let color = temp_color(t);
            let crit = if let Some(c) = t.critical_celsius {
                format!(" / {:.0}°C crit", c)
            } else {
                String::new()
            };
            Line::from(vec![Span::styled(
                format!("  {}: {:.1}°C{}", t.label, t.temp_celsius, crit),
                Style::default().fg(color),
            )])
        })
        .collect();

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}
