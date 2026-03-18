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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use nazar_core::*;

    fn buf_to_string(buf: &Buffer) -> String {
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf.cell((x, y)).unwrap().symbol());
            }
        }
        s
    }

    fn make_state() -> SharedState {
        new_shared_state(NazarConfig::default())
    }

    fn make_snapshot() -> SystemSnapshot {
        SystemSnapshot {
            timestamp: chrono::Utc::now(),
            cpu: CpuMetrics {
                cores: vec![25.0, 50.0, 75.0, 100.0],
                total_percent: 62.5,
                load_average: [1.5, 1.2, 0.9],
                processes: 150,
                threads: 500,
            },
            memory: MemoryMetrics {
                total_bytes: 16_000_000_000,
                used_bytes: 8_000_000_000,
                available_bytes: 8_000_000_000,
                swap_total_bytes: 4_000_000_000,
                swap_used_bytes: 1_000_000_000,
                agent_usage: HashMap::new(),
            },
            disk: vec![DiskMetrics {
                mount_point: "/".into(),
                device: "/dev/sda1".into(),
                filesystem: "ext4".into(),
                total_bytes: 500_000_000_000,
                used_bytes: 250_000_000_000,
                available_bytes: 250_000_000_000,
                read_bytes: 1024,
                write_bytes: 2048,
            }],
            network: NetworkMetrics {
                interfaces: vec![InterfaceMetrics {
                    name: "eth0".into(),
                    rx_bytes: 5000,
                    tx_bytes: 3000,
                    rx_packets: 100,
                    tx_packets: 80,
                    rx_errors: 0,
                    tx_errors: 0,
                    is_up: true,
                }],
                total_rx_bytes: 5000,
                total_tx_bytes: 3000,
                active_connections: 42,
            },
            temperatures: vec![ThermalInfo {
                label: "CPU".into(),
                temp_celsius: 55.0,
                critical_celsius: Some(100.0),
            }],
            gpu: vec![],
            agents: AgentSummary::default(),
            services: vec![],
            top_processes: vec![
                ProcessInfo {
                    pid: 1234,
                    name: "python".into(),
                    state: 'R',
                    cpu_percent: 45.0,
                    memory_bytes: 2_100_000_000,
                    memory_percent: 13.1,
                    threads: 8,
                },
                ProcessInfo {
                    pid: 5678,
                    name: "node".into(),
                    state: 'S',
                    cpu_percent: 12.0,
                    memory_bytes: 800_000_000,
                    memory_percent: 5.0,
                    threads: 4,
                },
            ],
        }
    }

    fn make_frame_data(snap: Option<SystemSnapshot>, is_agnos: bool) -> FrameData {
        FrameData {
            snap,
            alerts: vec![],
            predictions: vec![],
            cpu_history: vec![10.0, 20.0, 30.0, 40.0, 50.0],
            mem_history: vec![40.0, 45.0, 50.0, 55.0, 60.0],
            net_rx_history: vec![100.0, 200.0, 300.0],
            net_tx_history: vec![50.0, 100.0, 150.0],
            iface_history: HashMap::new(),
            api_url: "http://127.0.0.1:8090".into(),
            sample_count: 42,
            poll_secs: 5,
            triage: None,
            recommendations: None,
            is_agnos,
        }
    }

    // ---- Render tests: ensure no panics and basic content ----

    #[test]
    fn render_waiting_state() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);
        let data = make_frame_data(None, false);

        terminal
            .draw(|frame| {
                render_frame(frame, &app, &data);
            })
            .unwrap();

        let content = buf_to_string(terminal.backend().buffer());
        assert!(content.contains("Waiting"));
    }

    #[test]
    fn render_standard_layout_no_panic() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);
        let data = make_frame_data(Some(make_snapshot()), false);

        terminal
            .draw(|frame| {
                render_frame(frame, &app, &data);
            })
            .unwrap();

        let content = buf_to_string(terminal.backend().buffer());
        assert!(content.contains("NAZAR"));
        assert!(content.contains("CPU"));
        assert!(content.contains("Memory"));
    }

    #[test]
    fn render_agnos_layout_no_panic() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);

        let mut snap = make_snapshot();
        snap.agents = AgentSummary {
            total: 3,
            running: 2,
            idle: 1,
            error: 0,
            cpu_usage: HashMap::from([("planner".into(), 12.0), ("vision".into(), 34.0)]),
            memory_usage: HashMap::from([
                ("planner".into(), 450_000_000),
                ("vision".into(), 1_200_000_000),
            ]),
        };
        snap.services = vec![ServiceStatus {
            name: "daimon".into(),
            state: ServiceState::Running,
            pid: Some(100),
            uptime_secs: Some(7200),
            port: Some(8090),
        }];

        let data = make_frame_data(Some(snap), true);

        terminal
            .draw(|frame| {
                render_frame(frame, &app, &data);
            })
            .unwrap();

        let content = buf_to_string(terminal.backend().buffer());
        assert!(content.contains("NAZAR"));
        assert!(content.contains("AGNOS"));
    }

    #[test]
    fn render_minimal_layout_small_terminal() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);
        let data = make_frame_data(Some(make_snapshot()), false);

        // Should not panic even on tiny terminal
        terminal
            .draw(|frame| {
                render_frame(frame, &app, &data);
            })
            .unwrap();
    }

    #[test]
    fn render_help_overlay_no_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let mut app = TuiApp::new(state);
        app.show_help = true;
        let data = make_frame_data(Some(make_snapshot()), false);

        terminal
            .draw(|frame| {
                render_frame(frame, &app, &data);
            })
            .unwrap();

        let content = buf_to_string(terminal.backend().buffer());
        assert!(content.contains("Help"));
    }

    #[test]
    fn render_all_tabs_no_panic() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let mut app = TuiApp::new(state);

        let mut data = make_frame_data(Some(make_snapshot()), false);
        data.alerts = vec![Alert {
            severity: AlertSeverity::Warning,
            component: "cpu".into(),
            message: "high".into(),
            timestamp: chrono::Utc::now(),
        }];
        data.predictions = vec![PredictionResult {
            metric: "memory".into(),
            current_value: 50.0,
            predicted_value: 95.0,
            intervals_until: 540,
            trend: Trend::Rising,
            confidence_low: Some(400),
            confidence_high: Some(700),
        }];
        data.triage = Some("CPU spike from python".into());
        data.recommendations = Some("Consider restarting".into());

        for &tab in Tab::ALL {
            app.active_tab = tab;
            terminal
                .draw(|frame| {
                    render_frame(frame, &app, &data);
                })
                .unwrap();
        }
    }

    #[test]
    fn render_with_gpu_data() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);

        let mut snap = make_snapshot();
        snap.gpu = vec![GpuMetrics {
            id: "card0".into(),
            driver: "amdgpu".into(),
            name: "RX 7900".into(),
            utilization_percent: 75.0,
            vram_total_bytes: 16_000_000_000,
            vram_used_bytes: 8_000_000_000,
            temp_celsius: Some(65.0),
            power_watts: Some(200.0),
            clock_mhz: Some(2400),
        }];

        let data = make_frame_data(Some(snap), false);

        terminal
            .draw(|frame| {
                render_frame(frame, &app, &data);
            })
            .unwrap();

        let content = buf_to_string(terminal.backend().buffer());
        assert!(content.contains("GPU"));
    }

    #[test]
    fn render_empty_data_panels() {
        // Snapshot with all empty collections
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);

        let snap = SystemSnapshot {
            timestamp: chrono::Utc::now(),
            cpu: CpuMetrics {
                cores: vec![],
                total_percent: 0.0,
                load_average: [0.0; 3],
                processes: 0,
                threads: 0,
            },
            memory: MemoryMetrics {
                total_bytes: 0,
                used_bytes: 0,
                available_bytes: 0,
                swap_total_bytes: 0,
                swap_used_bytes: 0,
                agent_usage: HashMap::new(),
            },
            disk: vec![],
            network: NetworkMetrics {
                interfaces: vec![],
                total_rx_bytes: 0,
                total_tx_bytes: 0,
                active_connections: 0,
            },
            temperatures: vec![],
            gpu: vec![],
            agents: AgentSummary::default(),
            services: vec![],
            top_processes: vec![],
        };

        let data = make_frame_data(Some(snap), false);

        terminal
            .draw(|frame| {
                render_frame(frame, &app, &data);
            })
            .unwrap();
    }

    #[test]
    fn render_very_small_terminal() {
        // 20x5 — extremely small, should not panic
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);
        let data = make_frame_data(Some(make_snapshot()), false);

        terminal
            .draw(|frame| {
                render_frame(frame, &app, &data);
            })
            .unwrap();
    }

    #[test]
    fn header_shows_sample_count() {
        let backend = TestBackend::new(120, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);
        let data = make_frame_data(None, false);

        terminal
            .draw(|frame| {
                let area = frame.area();
                widgets::header::render(frame, area, &app, &data);
            })
            .unwrap();

        let content = buf_to_string(terminal.backend().buffer());
        assert!(content.contains("42 samples"));
    }

    // ---- Widget-level render tests ----

    #[test]
    fn widget_cpu_renders() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let cpu = CpuMetrics {
            cores: vec![20.0, 40.0, 60.0, 80.0],
            total_percent: 50.0,
            load_average: [1.0, 0.8, 0.7],
            processes: 100,
            threads: 300,
        };
        let history = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        terminal
            .draw(|frame| {
                widgets::cpu::render(frame, frame.area(), &cpu, &history);
            })
            .unwrap();
    }

    #[test]
    fn widget_memory_renders() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let mem = MemoryMetrics {
            total_bytes: 16_000_000_000,
            used_bytes: 12_000_000_000,
            available_bytes: 4_000_000_000,
            swap_total_bytes: 8_000_000_000,
            swap_used_bytes: 2_000_000_000,
            agent_usage: HashMap::new(),
        };

        terminal
            .draw(|frame| {
                widgets::memory::render(frame, frame.area(), &mem, &[50.0, 60.0, 70.0, 75.0]);
            })
            .unwrap();
    }

    #[test]
    fn widget_disk_renders_empty() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::disk::render(frame, frame.area(), &[]);
            })
            .unwrap();
    }

    #[test]
    fn widget_gpu_renders_empty() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::gpu::render(frame, frame.area(), &[]);
            })
            .unwrap();
    }

    #[test]
    fn widget_temperatures_renders_empty() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::temperatures::render(frame, frame.area(), &[]);
            })
            .unwrap();
    }

    #[test]
    fn widget_services_renders_empty() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::services::render(frame, frame.area(), &[]);
            })
            .unwrap();
    }

    #[test]
    fn widget_agents_renders_empty() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::agents::render(frame, frame.area(), &AgentSummary::default());
            })
            .unwrap();
    }

    #[test]
    fn widget_alerts_renders_empty() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::alerts::render(frame, frame.area(), &[], 0);
            })
            .unwrap();
    }

    #[test]
    fn widget_predictions_renders_empty() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::predictions::render(frame, frame.area(), &[], 5);
            })
            .unwrap();
    }

    #[test]
    fn widget_insights_renders_none() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::insights::render(frame, frame.area(), &None, &None);
            })
            .unwrap();
    }

    #[test]
    fn widget_insights_renders_both() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::insights::render(
                    frame,
                    frame.area(),
                    &Some("triage text".into()),
                    &Some("rec text".into()),
                );
            })
            .unwrap();
    }

    #[test]
    fn widget_help_renders() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                widgets::help::render(frame, frame.area());
            })
            .unwrap();
    }

    #[test]
    fn widget_services_renders_with_data() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        let services = vec![
            ServiceStatus {
                name: "daimon".into(),
                state: ServiceState::Running,
                pid: Some(1234),
                uptime_secs: Some(7200),
                port: Some(8090),
            },
            ServiceStatus {
                name: "hoosh".into(),
                state: ServiceState::Failed,
                pid: None,
                uptime_secs: None,
                port: Some(8088),
            },
        ];

        terminal
            .draw(|frame| {
                widgets::services::render(frame, frame.area(), &services);
            })
            .unwrap();
    }

    #[test]
    fn widget_alerts_renders_with_scroll() {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        let alerts: Vec<Alert> = (0..20)
            .map(|i| Alert {
                severity: if i % 3 == 0 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                },
                component: format!("comp-{i}"),
                message: format!("msg {i}"),
                timestamp: chrono::Utc::now(),
            })
            .collect();

        terminal
            .draw(|frame| {
                widgets::alerts::render(frame, frame.area(), &alerts, 5);
            })
            .unwrap();
    }

    #[test]
    fn widget_processes_renders_with_data() {
        let backend = TestBackend::new(80, 15);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state();
        let app = TuiApp::new(state);

        let procs = vec![
            ProcessInfo {
                pid: 1,
                name: "init".into(),
                state: 'S',
                cpu_percent: 0.1,
                memory_bytes: 10_000_000,
                memory_percent: 0.1,
                threads: 1,
            },
            ProcessInfo {
                pid: 1000,
                name: "firefox".into(),
                state: 'R',
                cpu_percent: 55.0,
                memory_bytes: 3_000_000_000,
                memory_percent: 18.7,
                threads: 42,
            },
        ];

        terminal
            .draw(|frame| {
                widgets::processes::render(frame, frame.area(), &app, &procs);
            })
            .unwrap();
    }
}
