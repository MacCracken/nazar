use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    triage: &Option<String>,
    recommendations: &Option<String>,
) {
    let block = Block::default()
        .title(Span::styled(
            " AI Insights ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if triage.is_none() && recommendations.is_none() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let p = Paragraph::new("  Waiting for AI analysis...")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let has_both = triage.is_some() && recommendations.is_some();
    let chunks = if has_both {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(inner)
    } else {
        Layout::vertical([Constraint::Min(1)]).split(inner)
    };

    let mut chunk_idx = 0;

    if let Some(t) = triage {
        let p = Paragraph::new(format!("  Triage: {t}"))
            .style(Style::default().fg(Color::Yellow))
            .wrap(Wrap { trim: true });
        frame.render_widget(p, chunks[chunk_idx]);
        chunk_idx += 1;
    }

    if let Some(r) = recommendations {
        let target = if chunk_idx < chunks.len() {
            chunk_idx
        } else {
            0
        };
        let p = Paragraph::new(format!("  Recommendations: {r}"))
            .style(Style::default().fg(Color::Cyan))
            .wrap(Wrap { trim: true });
        frame.render_widget(p, chunks[target]);
    }
}
