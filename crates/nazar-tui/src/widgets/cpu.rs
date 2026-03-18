use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Sparkline};

use nazar_core::CpuMetrics;

fn usage_color(pct: f64) -> Color {
    if pct > 80.0 {
        Color::Red
    } else if pct > 50.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

pub fn render(frame: &mut Frame, area: Rect, cpu: &CpuMetrics, history: &[f64]) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" CPU ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(
                    "Load: {:.2}/{:.2}/{:.2}  Procs: {}  Threads: {} ",
                    cpu.load_average[0],
                    cpu.load_average[1],
                    cpu.load_average[2],
                    cpu.processes,
                    cpu.threads,
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

    // Split: gauge row + sparkline + per-core bars
    let has_cores = !cpu.cores.is_empty();
    let core_rows = if has_cores {
        // 1 row for every 8 cores
        cpu.cores.len().div_ceil(8).min(3) as u16
    } else {
        0
    };

    let constraints: Vec<Constraint> = if inner.height > 3 + core_rows {
        vec![
            Constraint::Length(1),                // gauge
            Constraint::Min(2),                   // sparkline
            Constraint::Length(core_rows.max(1)), // cores
        ]
    } else {
        vec![Constraint::Length(1), Constraint::Min(1)]
    };

    let chunks = Layout::vertical(constraints).split(inner);

    // Overall gauge
    let color = usage_color(cpu.total_percent);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio((cpu.total_percent / 100.0).clamp(0.0, 1.0))
        .label(format!("{:.1}%", cpu.total_percent));
    frame.render_widget(gauge, chunks[0]);

    // Sparkline
    if chunks.len() > 1 && !history.is_empty() {
        let spark_data: Vec<u64> = history.iter().map(|v| *v as u64).collect();
        let sparkline = Sparkline::default()
            .data(&spark_data)
            .max(100)
            .style(Style::default().fg(color))
            .bar_set(symbols::bar::NINE_LEVELS);
        frame.render_widget(sparkline, chunks[1]);
    }

    // Per-core mini gauges
    if chunks.len() > 2 && has_cores {
        let core_area = chunks[2];
        let cores_per_row = 8usize;
        for (i, pct) in cpu.cores.iter().enumerate() {
            let row = (i / cores_per_row) as u16;
            let col = i % cores_per_row;
            let col_width = core_area.width / cores_per_row as u16;
            if row >= core_area.height {
                break;
            }
            let rect = Rect {
                x: core_area.x + col as u16 * col_width,
                y: core_area.y + row,
                width: col_width,
                height: 1,
            };
            let c = usage_color(*pct);
            let g = Gauge::default()
                .gauge_style(Style::default().fg(c).bg(Color::DarkGray))
                .ratio((pct / 100.0).clamp(0.0, 1.0))
                .label(format!("C{i}:{pct:.0}%"));
            frame.render_widget(g, rect);
        }
    }
}
