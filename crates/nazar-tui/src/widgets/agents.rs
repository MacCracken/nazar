use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Row, Table};

use nazar_core::AgentSummary;

pub fn render(frame: &mut Frame, area: Rect, agents: &AgentSummary) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Agents ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(
                    "Total:{} Run:{} Idle:{} Err:{} ",
                    agents.total, agents.running, agents.idle, agents.error,
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if agents.total == 0 {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let p = ratatui::widgets::Paragraph::new("  No agents detected")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    // Merge CPU and memory maps
    let mut agent_ids: Vec<&String> = agents
        .cpu_usage
        .keys()
        .chain(agents.memory_usage.keys())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    agent_ids.sort();

    let widths = [
        ratatui::layout::Constraint::Min(14),    // Agent
        ratatui::layout::Constraint::Length(8),  // CPU%
        ratatui::layout::Constraint::Length(12), // Memory
    ];

    let header = Row::new(vec!["Agent", "CPU%", "Memory"]).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Cyan),
    );

    let rows: Vec<Row> = agent_ids
        .iter()
        .map(|id| {
            let cpu = agents
                .cpu_usage
                .get(*id)
                .map(|v| format!("{:.1}", v))
                .unwrap_or_else(|| "-".into());
            let mem = agents
                .memory_usage
                .get(*id)
                .map(|v| format!("{:.1} MB", *v as f64 / 1e6))
                .unwrap_or_else(|| "-".into());
            Row::new(vec![(*id).clone(), cpu, mem])
        })
        .collect();

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}
