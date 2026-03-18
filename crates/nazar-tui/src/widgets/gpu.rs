use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use nazar_core::GpuMetrics;

pub fn render(frame: &mut Frame, area: Rect, gpus: &[GpuMetrics]) {
    let block = Block::default()
        .title(Span::styled(
            " GPU ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if gpus.is_empty() {
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 1 {
        return;
    }

    // 2 rows per GPU: info line + gauge
    let constraints: Vec<Constraint> = gpus
        .iter()
        .flat_map(|_| [Constraint::Length(1), Constraint::Length(1)])
        .chain(std::iter::once(Constraint::Min(0)))
        .collect();
    let chunks = Layout::vertical(constraints).split(inner);

    for (i, g) in gpus.iter().enumerate() {
        let info_idx = i * 2;
        let gauge_idx = i * 2 + 1;
        if gauge_idx >= chunks.len() {
            break;
        }

        let mut info = format!(
            "{} ({}) Util:{:.0}% VRAM:{:.0}M/{:.0}M",
            g.name,
            g.driver,
            g.utilization_percent,
            g.vram_used_bytes as f64 / 1e6,
            g.vram_total_bytes as f64 / 1e6,
        );
        if let Some(temp) = g.temp_celsius {
            info.push_str(&format!(" {:.0}°C", temp));
        }
        if let Some(power) = g.power_watts {
            info.push_str(&format!(" {:.1}W", power));
        }
        if let Some(clock) = g.clock_mhz {
            info.push_str(&format!(" {}MHz", clock));
        }

        let p = Paragraph::new(info).style(Style::default().fg(Color::White));
        frame.render_widget(p, chunks[info_idx]);

        let color = if g.utilization_percent > 80.0 {
            Color::Red
        } else if g.utilization_percent > 50.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
            .ratio((g.utilization_percent / 100.0).clamp(0.0, 1.0))
            .label(format!(
                "GPU {:.0}%  VRAM {:.1}%",
                g.utilization_percent,
                g.vram_used_percent()
            ));
        frame.render_widget(gauge, chunks[gauge_idx]);
    }
}
