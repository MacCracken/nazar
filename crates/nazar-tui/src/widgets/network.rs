use std::collections::HashMap;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Sparkline};
use ratatui::Frame;

use nazar_core::NetworkMetrics;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    net: &NetworkMetrics,
    rx_history: &[f64],
    tx_history: &[f64],
    _iface_history: &HashMap<String, (Vec<f64>, Vec<f64>)>,
) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Network ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(
                    "RX: {:.1} KB/s  TX: {:.1} KB/s  Conn: {} ",
                    net.total_rx_bytes as f64 / 1024.0,
                    net.total_tx_bytes as f64 / 1024.0,
                    net.active_connections,
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    // Show combined RX/TX sparklines, then per-interface info
    let iface_count = net
        .interfaces
        .iter()
        .filter(|i| i.name != "lo")
        .count()
        .min(4) as u16;

    let mut constraints = vec![Constraint::Length(2)]; // combined RX/TX
    for _ in 0..iface_count {
        constraints.push(Constraint::Length(1)); // iface label
    }
    constraints.push(Constraint::Min(0)); // leftover

    let chunks = Layout::vertical(constraints).split(inner);

    // Combined RX sparkline
    if !rx_history.is_empty() {
        let rx_max = rx_history.iter().copied().fold(1.0_f64, f64::max);
        let rx_data: Vec<u64> = rx_history
            .iter()
            .map(|v| ((v / rx_max) * 100.0) as u64)
            .collect();
        let spark = Sparkline::default()
            .data(&rx_data)
            .max(100)
            .style(Style::default().fg(Color::Green))
            .bar_set(symbols::bar::NINE_LEVELS);
        let rx_area = Rect { height: 1, ..chunks[0] };
        frame.render_widget(spark, rx_area);
    }

    if !tx_history.is_empty() && chunks[0].height > 1 {
        let tx_max = tx_history.iter().copied().fold(1.0_f64, f64::max);
        let tx_data: Vec<u64> = tx_history
            .iter()
            .map(|v| ((v / tx_max) * 100.0) as u64)
            .collect();
        let spark = Sparkline::default()
            .data(&tx_data)
            .max(100)
            .style(Style::default().fg(Color::Blue))
            .bar_set(symbols::bar::NINE_LEVELS);
        let tx_area = Rect {
            y: chunks[0].y + 1,
            height: 1,
            ..chunks[0]
        };
        frame.render_widget(spark, tx_area);
    }

    // Per-interface labels
    for (idx, iface) in net
        .interfaces
        .iter()
        .filter(|i| i.name != "lo")
        .take(iface_count as usize)
        .enumerate()
    {
        let chunk_idx = idx + 1;
        if chunk_idx >= chunks.len() {
            break;
        }
        let status = if iface.is_up { "UP" } else { "DN" };
        let err_str = if iface.rx_errors > 0 || iface.tx_errors > 0 {
            format!(" err:rx={} tx={}", iface.rx_errors, iface.tx_errors)
        } else {
            String::new()
        };
        let line = Line::from(vec![
            Span::styled(
                format!("  {} ", iface.name),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("[{}] ", status),
                if iface.is_up {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
            Span::styled(
                format!(
                    "RX:{:.1}K TX:{:.1}K{}",
                    iface.rx_bytes as f64 / 1024.0,
                    iface.tx_bytes as f64 / 1024.0,
                    err_str,
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        let p = ratatui::widgets::Paragraph::new(line);
        frame.render_widget(p, chunks[chunk_idx]);
    }
}
