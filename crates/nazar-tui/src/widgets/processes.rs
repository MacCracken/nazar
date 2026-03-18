use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Row, Table};

use nazar_core::ProcessInfo;

use crate::app::{ProcessSort, TuiApp};

pub fn render(frame: &mut Frame, area: Rect, app: &TuiApp, procs: &[ProcessInfo]) {
    let sorted = app.sorted_processes(procs);

    let block = Block::default()
        .title(Span::styled(
            format!(
                " Processes [sort: {} {}] s:cycle r:reverse ",
                app.process_sort.label(),
                if app.sort_reverse { "desc" } else { "asc" }
            ),
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let widths = [
        ratatui::layout::Constraint::Length(7),  // PID
        ratatui::layout::Constraint::Min(14),    // Name
        ratatui::layout::Constraint::Length(7),  // CPU%
        ratatui::layout::Constraint::Length(10), // Memory
        ratatui::layout::Constraint::Length(6),  // Mem%
        ratatui::layout::Constraint::Length(3),  // St
        ratatui::layout::Constraint::Length(5),  // Thr
    ];

    let header_style = Style::default()
        .add_modifier(Modifier::BOLD)
        .fg(Color::Cyan);

    let header_cells = ["PID", "Name", "CPU%", "Memory", "Mem%", "St", "Thr"];
    let header_row: Vec<Span> = header_cells
        .iter()
        .enumerate()
        .map(|(i, &cell)| {
            let sort_match = matches!(
                (i, app.process_sort),
                (0, ProcessSort::Pid)
                    | (1, ProcessSort::Name)
                    | (2, ProcessSort::Cpu)
                    | (3, ProcessSort::Memory)
            );
            if sort_match {
                let arrow = if app.sort_reverse { "v" } else { "^" };
                Span::styled(format!("{cell}{arrow}"), header_style.fg(Color::White))
            } else {
                Span::styled(cell.to_string(), header_style)
            }
        })
        .collect();
    let header = Row::new(header_row);

    let skip = app.scroll_offset as usize;
    let rows: Vec<Row> = sorted
        .iter()
        .skip(skip)
        .map(|p| {
            let cpu_color = if p.cpu_percent > 80.0 {
                Color::Red
            } else if p.cpu_percent > 40.0 {
                Color::Yellow
            } else {
                Color::White
            };
            Row::new(vec![
                p.pid.to_string(),
                p.name.clone(),
                format!("{:.1}", p.cpu_percent),
                format!("{:.1} MB", p.memory_bytes as f64 / 1e6),
                format!("{:.1}", p.memory_percent),
                p.state.to_string(),
                p.threads.to_string(),
            ])
            .style(Style::default().fg(cpu_color))
        })
        .collect();

    let table = Table::new(rows, widths).header(header).block(block);

    frame.render_widget(table, area);
}
