use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use nazar_core::{PredictionResult, Trend};

pub fn render(frame: &mut Frame, area: Rect, predictions: &[PredictionResult], poll_secs: u64) {
    let block = Block::default()
        .title(Span::styled(
            " Predictions ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if predictions.is_empty() {
        let p = Paragraph::new("  No predictions yet")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let lines: Vec<Line> = predictions
        .iter()
        .map(|pred| {
            let mins = (pred.intervals_until * poll_secs) / 60;
            let trend_icon = match pred.trend {
                Trend::Rising => "^",
                Trend::Stable => "~",
                Trend::Falling => "v",
            };
            let trend_color = match pred.trend {
                Trend::Rising => Color::Red,
                Trend::Stable => Color::Yellow,
                Trend::Falling => Color::Green,
            };

            let mut spans = vec![
                Span::styled(format!("  {}: ", pred.metric), Style::default().fg(Color::White)),
                Span::styled(
                    format!("{:.1}%", pred.current_value),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" -> "),
                Span::styled(
                    format!("{:.1}%", pred.predicted_value),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!(" in ~{mins}min "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("[{trend_icon}]"),
                    Style::default().fg(trend_color),
                ),
            ];

            if let (Some(lo), Some(hi)) = (pred.confidence_low, pred.confidence_high) {
                let lo_min = (lo * poll_secs) / 60;
                let hi_min = (hi * poll_secs) / 60;
                spans.push(Span::styled(
                    format!(" (95%: {lo_min}-{hi_min}min)"),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        })
        .collect();

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}
