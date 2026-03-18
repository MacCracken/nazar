use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Row, Table};

use nazar_core::DiskMetrics;

fn usage_color(pct: f64) -> Color {
    if pct > 90.0 {
        Color::Red
    } else if pct > 70.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

pub fn render(frame: &mut Frame, area: Rect, disks: &[DiskMetrics]) {
    let block = Block::default()
        .title(Span::styled(
            " Disk ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if disks.is_empty() {
        frame.render_widget(block.clone(), area);
        return;
    }

    let widths = [
        ratatui::layout::Constraint::Length(12), // mount
        ratatui::layout::Constraint::Length(10), // device
        ratatui::layout::Constraint::Length(12), // used/total
        ratatui::layout::Constraint::Length(6),  // pct
        ratatui::layout::Constraint::Min(10),    // bar
    ];

    let header = Row::new(vec!["Mount", "Device", "Used/Total", "%", ""]).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Cyan),
    );

    let rows: Vec<Row> = disks
        .iter()
        .map(|d| {
            let pct = d.used_percent();
            let color = usage_color(pct);
            let used = format!(
                "{:.1}G/{:.1}G",
                d.used_bytes as f64 / 1e9,
                d.total_bytes as f64 / 1e9
            );
            let pct_str = format!("{:.1}%", pct);

            // Simple text-based bar
            let bar_width = 10usize;
            let filled = ((pct / 100.0) * bar_width as f64) as usize;
            let bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(bar_width - filled));

            Row::new(vec![
                d.mount_point.clone(),
                d.device.clone(),
                used,
                pct_str,
                bar,
            ])
            .style(Style::default().fg(color))
        })
        .collect();

    let table = Table::new(rows, widths).header(header).block(block);

    frame.render_widget(table, area);
}
