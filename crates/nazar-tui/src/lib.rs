//! Nazar TUI — Terminal UI for system monitoring (ratatui)

pub mod app;
pub mod widgets;

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};

use nazar_core::SharedState;

use app::{FrameData, Tab, TuiApp};

/// Launch the TUI event loop. Blocks until the user quits.
pub fn run_tui(state: SharedState) -> io::Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = TuiApp::new(state);

    let result = run_loop(&mut terminal, &mut app);

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiApp,
) -> io::Result<()> {
    loop {
        let size = terminal.size()?;
        let frame_data = FrameData::from_state(&app.state, size.width as usize);

        terminal.draw(|frame| {
            render_frame(frame, app, &frame_data);
        })?;

        // Poll for events with timeout matching refresh rate
        if event::poll(Duration::from_millis(app.refresh_ms))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('?') => app.show_help = !app.show_help,
                KeyCode::Tab => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        app.prev_tab();
                    } else {
                        app.next_tab();
                    }
                }
                KeyCode::BackTab => app.prev_tab(),
                KeyCode::Char('1') => {
                    app.active_tab = Tab::Processes;
                    app.scroll_offset = 0;
                }
                KeyCode::Char('2') => {
                    app.active_tab = Tab::Alerts;
                    app.scroll_offset = 0;
                }
                KeyCode::Char('3') => {
                    app.active_tab = Tab::Predictions;
                    app.scroll_offset = 0;
                }
                KeyCode::Char('4') => {
                    app.active_tab = Tab::Agents;
                    app.scroll_offset = 0;
                }
                KeyCode::Char('5') => {
                    app.active_tab = Tab::Services;
                    app.scroll_offset = 0;
                }
                KeyCode::Char('6') => {
                    app.active_tab = Tab::Insights;
                    app.scroll_offset = 0;
                }
                KeyCode::Up => app.scroll_up(),
                KeyCode::Down => app.scroll_down(),
                KeyCode::Char('s') => app.cycle_sort(),
                KeyCode::Char('r') => app.toggle_sort_order(),
                _ => {}
            }
        }
    }
}

fn render_frame(frame: &mut ratatui::Frame, app: &TuiApp, data: &FrameData) {
    let area = frame.area();

    // Header (1 row) + main content
    let [header_area, main_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);

    widgets::header::render(frame, header_area, app, data);

    let Some(snap) = &data.snap else {
        let waiting = ratatui::widgets::Paragraph::new("  Waiting for first metrics snapshot...")
            .style(ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray));
        frame.render_widget(waiting, main_area);
        return;
    };

    // Adaptive layout based on terminal height and AGNOS mode
    if main_area.height < 10 {
        // Minimal: just CPU + Memory side by side
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(main_area);
        widgets::cpu::render(frame, left, &snap.cpu, &data.cpu_history);
        widgets::memory::render(frame, right, &snap.memory, &data.mem_history);
    } else if data.is_agnos {
        render_agnos_layout(frame, main_area, app, data, snap);
    } else {
        render_standard_layout(frame, main_area, app, data, snap);
    }

    // Help overlay
    if app.show_help {
        widgets::help::render(frame, area);
    }
}

fn render_standard_layout(
    frame: &mut ratatui::Frame,
    area: Rect,
    app: &TuiApp,
    data: &FrameData,
    snap: &nazar_core::SystemSnapshot,
) {
    // Top: CPU | Memory
    // Mid: Disk | Network
    // Mid2: GPU | Temps  (conditional)
    // Bottom: active tab panel
    let has_gpu = !snap.gpu.is_empty();
    let has_temps = !snap.temperatures.is_empty();
    let mid2_height = if has_gpu || has_temps { 6 } else { 0 };

    let constraints = vec![
        Constraint::Length(8),           // CPU + Memory
        Constraint::Length(8),           // Disk + Network
        Constraint::Length(mid2_height), // GPU + Temps
        Constraint::Min(6),              // Tab panel
    ];

    let chunks = Layout::vertical(constraints).split(area);

    // Row 1: CPU | Memory
    let [cpu_area, mem_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(chunks[0]);
    widgets::cpu::render(frame, cpu_area, &snap.cpu, &data.cpu_history);
    widgets::memory::render(frame, mem_area, &snap.memory, &data.mem_history);

    // Row 2: Disk | Network
    let [disk_area, net_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(chunks[1]);
    widgets::disk::render(frame, disk_area, &snap.disk);
    widgets::network::render(
        frame,
        net_area,
        &snap.network,
        &data.net_rx_history,
        &data.net_tx_history,
        &data.iface_history,
    );

    // Row 3: GPU | Temps (conditional)
    if mid2_height > 0 {
        let [gpu_area, temp_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(chunks[2]);
        if has_gpu {
            widgets::gpu::render(frame, gpu_area, &snap.gpu);
        }
        if has_temps {
            widgets::temperatures::render(frame, temp_area, &snap.temperatures);
        }
    }

    // Row 4: Active tab
    let tab_area = if mid2_height > 0 {
        chunks[3]
    } else {
        chunks[2]
    };
    render_tab_panel(frame, tab_area, app, data, snap);
}

fn render_agnos_layout(
    frame: &mut ratatui::Frame,
    area: Rect,
    app: &TuiApp,
    data: &FrameData,
    snap: &nazar_core::SystemSnapshot,
) {
    // Top: CPU | Memory
    // Mid: Disk+Network | Services+Agents
    // Bottom: active tab panel
    let constraints = vec![
        Constraint::Length(8),  // CPU + Memory
        Constraint::Length(10), // Disk+Net | Services+Agents
        Constraint::Min(6),     // Tab panel
    ];

    let chunks = Layout::vertical(constraints).split(area);

    // Row 1: CPU | Memory
    let [cpu_area, mem_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(chunks[0]);
    widgets::cpu::render(frame, cpu_area, &snap.cpu, &data.cpu_history);
    widgets::memory::render(frame, mem_area, &snap.memory, &data.mem_history);

    // Row 2: Left = Disk+Network stacked, Right = Services+Agents stacked
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(chunks[1]);

    let [disk_area, net_area] =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(left);
    widgets::disk::render(frame, disk_area, &snap.disk);
    widgets::network::render(
        frame,
        net_area,
        &snap.network,
        &data.net_rx_history,
        &data.net_tx_history,
        &data.iface_history,
    );

    let [svc_area, agent_area] =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(right);
    widgets::services::render(frame, svc_area, &snap.services);
    widgets::agents::render(frame, agent_area, &snap.agents);

    // Row 3: Active tab
    render_tab_panel(frame, chunks[2], app, data, snap);
}

fn render_tab_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    app: &TuiApp,
    data: &FrameData,
    snap: &nazar_core::SystemSnapshot,
) {
    match app.active_tab {
        Tab::Processes => widgets::processes::render(frame, area, app, &snap.top_processes),
        Tab::Alerts => widgets::alerts::render(frame, area, &data.alerts, app.scroll_offset),
        Tab::Predictions => {
            widgets::predictions::render(frame, area, &data.predictions, data.poll_secs)
        }
        Tab::Agents => widgets::agents::render(frame, area, &snap.agents),
        Tab::Services => widgets::services::render(frame, area, &snap.services),
        Tab::Insights => {
            widgets::insights::render(frame, area, &data.triage, &data.recommendations)
        }
    }
}
