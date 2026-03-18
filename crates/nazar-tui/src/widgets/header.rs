use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{FrameData, Tab, TuiApp};

pub fn render(frame: &mut Frame, area: Rect, app: &TuiApp, data: &FrameData) {
    let agnos_tag = if data.is_agnos {
        Span::styled(" AGNOS ", Style::default().fg(Color::Black).bg(Color::Cyan))
    } else {
        Span::raw("")
    };

    let tabs: Vec<Span> = Tab::ALL
        .iter()
        .map(|&tab| {
            let style = if tab == app.active_tab {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Span::styled(format!(" {}:{} ", tab.key(), tab.label()), style)
        })
        .collect();

    let mut spans = vec![
        Span::styled(" NAZAR ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        agnos_tag,
        Span::raw(" "),
        Span::styled(
            format!("daimon: {} ", data.api_url),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("| "),
        Span::styled(
            format!("{} samples ", data.sample_count),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("| "),
    ];
    spans.extend(tabs);
    spans.push(Span::raw(" | "));
    spans.push(Span::styled("q:quit ?:help", Style::default().fg(Color::DarkGray)));

    let header = Paragraph::new(Line::from(spans));
    frame.render_widget(header, area);
}
