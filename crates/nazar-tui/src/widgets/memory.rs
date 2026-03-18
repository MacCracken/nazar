use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Sparkline};

use nazar_core::MemoryMetrics;

fn usage_color(pct: f64) -> Color {
    if pct > 80.0 {
        Color::Red
    } else if pct > 50.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

pub fn render(frame: &mut Frame, area: Rect, mem: &MemoryMetrics, history: &[f64]) {
    let used_gb = mem.used_bytes as f64 / 1e9;
    let total_gb = mem.total_bytes as f64 / 1e9;
    let pct = mem.used_percent();

    let mut title_spans = vec![
        Span::styled(" Memory ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:.1} GB / {:.1} GB ", used_gb, total_gb),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    if mem.swap_total_bytes > 0 {
        title_spans.push(Span::styled(
            format!(
                "Swap: {:.1}/{:.1} GB ({:.0}%) ",
                mem.swap_used_bytes as f64 / 1e9,
                mem.swap_total_bytes as f64 / 1e9,
                mem.swap_used_percent(),
            ),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(inner);

    let color = usage_color(pct);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio((pct / 100.0).clamp(0.0, 1.0))
        .label(format!("{:.1}%", pct));
    frame.render_widget(gauge, chunks[0]);

    if !history.is_empty() {
        let spark_data: Vec<u64> = history.iter().map(|v| *v as u64).collect();
        let sparkline = Sparkline::default()
            .data(&spark_data)
            .max(100)
            .style(Style::default().fg(color))
            .bar_set(symbols::bar::NINE_LEVELS);
        frame.render_widget(sparkline, chunks[1]);
    }
}
