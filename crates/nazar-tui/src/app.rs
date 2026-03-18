use std::collections::HashMap;

use nazar_core::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Processes,
    Alerts,
    Predictions,
    Agents,
    Services,
    Insights,
}

impl Tab {
    pub const ALL: &[Tab] = &[
        Tab::Processes,
        Tab::Alerts,
        Tab::Predictions,
        Tab::Agents,
        Tab::Services,
        Tab::Insights,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Processes => "Processes",
            Tab::Alerts => "Alerts",
            Tab::Predictions => "Predictions",
            Tab::Agents => "Agents",
            Tab::Services => "Services",
            Tab::Insights => "AI Insights",
        }
    }

    pub fn key(self) -> char {
        match self {
            Tab::Processes => '1',
            Tab::Alerts => '2',
            Tab::Predictions => '3',
            Tab::Agents => '4',
            Tab::Services => '5',
            Tab::Insights => '6',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSort {
    Cpu,
    Memory,
    Pid,
    Name,
}

impl ProcessSort {
    pub fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Pid,
            Self::Pid => Self::Name,
            Self::Name => Self::Cpu,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU%",
            Self::Memory => "MEM",
            Self::Pid => "PID",
            Self::Name => "NAME",
        }
    }
}

/// All data cloned out of SharedState for a single render frame.
pub struct FrameData {
    pub snap: Option<SystemSnapshot>,
    pub alerts: Vec<Alert>,
    pub predictions: Vec<PredictionResult>,
    pub cpu_history: Vec<f64>,
    pub mem_history: Vec<f64>,
    pub net_rx_history: Vec<f64>,
    pub net_tx_history: Vec<f64>,
    pub iface_history: HashMap<String, (Vec<f64>, Vec<f64>)>,
    pub api_url: String,
    pub sample_count: usize,
    pub poll_secs: u64,
    pub triage: Option<String>,
    pub recommendations: Option<String>,
    pub is_agnos: bool,
}

impl FrameData {
    pub fn from_state(state: &SharedState, width: usize) -> Self {
        let s = read_state(state);
        let snap = s.latest.clone();
        let is_agnos = snap.as_ref().is_some_and(|sn| {
            sn.agents.total > 0
                || sn
                    .services
                    .iter()
                    .any(|svc| svc.name == "daimon" && svc.state == ServiceState::Running)
        });
        Self {
            snap,
            alerts: s.alerts.clone(),
            predictions: s.predictions.clone(),
            cpu_history: s.cpu_history.last_n(width),
            mem_history: s.mem_history.last_n(width),
            net_rx_history: s.net_rx_history.last_n(width),
            net_tx_history: s.net_tx_history.last_n(width),
            iface_history: s
                .net_iface_history
                .iter()
                .map(|(k, (rx, tx))| (k.clone(), (rx.last_n(width), tx.last_n(width))))
                .collect(),
            api_url: s.config.api_url.clone(),
            sample_count: s.cpu_history.points.len(),
            poll_secs: s.config.poll_interval_secs,
            triage: s.last_triage.clone(),
            recommendations: s.last_recommendations.clone(),
            is_agnos,
        }
    }
}

pub struct TuiApp {
    pub state: SharedState,
    pub active_tab: Tab,
    pub scroll_offset: u16,
    pub process_sort: ProcessSort,
    pub sort_reverse: bool,
    pub show_help: bool,
    pub refresh_ms: u64,
}

impl TuiApp {
    pub fn new(state: SharedState) -> Self {
        let refresh_ms = {
            let s = read_state(&state);
            s.config.ui_refresh_ms
        };
        Self {
            state,
            active_tab: Tab::Processes,
            scroll_offset: 0,
            process_sort: ProcessSort::Cpu,
            sort_reverse: true,
            show_help: false,
            refresh_ms,
        }
    }

    pub fn next_tab(&mut self) {
        let idx = Tab::ALL
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        self.active_tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
        self.scroll_offset = 0;
    }

    pub fn prev_tab(&mut self) {
        let idx = Tab::ALL
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        self.active_tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()];
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn cycle_sort(&mut self) {
        self.process_sort = self.process_sort.next();
    }

    pub fn toggle_sort_order(&mut self) {
        self.sort_reverse = !self.sort_reverse;
    }

    pub fn sorted_processes(&self, procs: &[ProcessInfo]) -> Vec<ProcessInfo> {
        let mut sorted = procs.to_vec();
        sorted.sort_by(|a, b| {
            let cmp = match self.process_sort {
                ProcessSort::Cpu => a.cpu_percent.partial_cmp(&b.cpu_percent).unwrap(),
                ProcessSort::Memory => a.memory_bytes.cmp(&b.memory_bytes),
                ProcessSort::Pid => a.pid.cmp(&b.pid),
                ProcessSort::Name => a.name.cmp(&b.name),
            };
            if self.sort_reverse {
                cmp.reverse()
            } else {
                cmp
            }
        });
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> SharedState {
        new_shared_state(NazarConfig::default())
    }

    fn sample_processes() -> Vec<ProcessInfo> {
        vec![
            ProcessInfo {
                pid: 100,
                name: "alpha".into(),
                state: 'R',
                cpu_percent: 50.0,
                memory_bytes: 200_000_000,
                memory_percent: 10.0,
                threads: 4,
            },
            ProcessInfo {
                pid: 200,
                name: "beta".into(),
                state: 'S',
                cpu_percent: 80.0,
                memory_bytes: 100_000_000,
                memory_percent: 5.0,
                threads: 2,
            },
            ProcessInfo {
                pid: 50,
                name: "gamma".into(),
                state: 'R',
                cpu_percent: 10.0,
                memory_bytes: 500_000_000,
                memory_percent: 25.0,
                threads: 8,
            },
        ]
    }

    // ---- Tab tests ----

    #[test]
    fn tab_all_has_six_entries() {
        assert_eq!(Tab::ALL.len(), 6);
    }

    #[test]
    fn tab_labels_are_nonempty() {
        for tab in Tab::ALL {
            assert!(!tab.label().is_empty());
        }
    }

    #[test]
    fn tab_keys_are_unique() {
        let keys: Vec<char> = Tab::ALL.iter().map(|t| t.key()).collect();
        let unique: std::collections::HashSet<char> = keys.iter().copied().collect();
        assert_eq!(keys.len(), unique.len());
    }

    #[test]
    fn tab_keys_are_1_through_6() {
        let keys: Vec<char> = Tab::ALL.iter().map(|t| t.key()).collect();
        assert_eq!(keys, vec!['1', '2', '3', '4', '5', '6']);
    }

    // ---- ProcessSort tests ----

    #[test]
    fn process_sort_cycles_through_all() {
        let start = ProcessSort::Cpu;
        let s1 = start.next();
        assert_eq!(s1, ProcessSort::Memory);
        let s2 = s1.next();
        assert_eq!(s2, ProcessSort::Pid);
        let s3 = s2.next();
        assert_eq!(s3, ProcessSort::Name);
        let s4 = s3.next();
        assert_eq!(s4, ProcessSort::Cpu); // wraps around
    }

    #[test]
    fn process_sort_labels_nonempty() {
        for sort in [
            ProcessSort::Cpu,
            ProcessSort::Memory,
            ProcessSort::Pid,
            ProcessSort::Name,
        ] {
            assert!(!sort.label().is_empty());
        }
    }

    // ---- TuiApp tests ----

    #[test]
    fn app_new_defaults() {
        let state = make_state();
        let app = TuiApp::new(state);
        assert_eq!(app.active_tab, Tab::Processes);
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.process_sort, ProcessSort::Cpu);
        assert!(app.sort_reverse);
        assert!(!app.show_help);
        assert_eq!(app.refresh_ms, 1000); // default ui_refresh_ms
    }

    #[test]
    fn app_next_tab_cycles() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        assert_eq!(app.active_tab, Tab::Processes);

        app.next_tab();
        assert_eq!(app.active_tab, Tab::Alerts);

        app.next_tab();
        assert_eq!(app.active_tab, Tab::Predictions);

        app.next_tab();
        assert_eq!(app.active_tab, Tab::Agents);

        app.next_tab();
        assert_eq!(app.active_tab, Tab::Services);

        app.next_tab();
        assert_eq!(app.active_tab, Tab::Insights);

        app.next_tab(); // wraps
        assert_eq!(app.active_tab, Tab::Processes);
    }

    #[test]
    fn app_prev_tab_cycles() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        assert_eq!(app.active_tab, Tab::Processes);

        app.prev_tab(); // wraps backwards
        assert_eq!(app.active_tab, Tab::Insights);

        app.prev_tab();
        assert_eq!(app.active_tab, Tab::Services);
    }

    #[test]
    fn app_tab_change_resets_scroll() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        app.scroll_offset = 10;
        app.next_tab();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn app_scroll_up_down() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        assert_eq!(app.scroll_offset, 0);

        app.scroll_down();
        assert_eq!(app.scroll_offset, 1);
        app.scroll_down();
        assert_eq!(app.scroll_offset, 2);

        app.scroll_up();
        assert_eq!(app.scroll_offset, 1);
        app.scroll_up();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn app_scroll_up_saturates_at_zero() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        app.scroll_up();
        assert_eq!(app.scroll_offset, 0);
        app.scroll_up();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn app_cycle_sort() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        assert_eq!(app.process_sort, ProcessSort::Cpu);

        app.cycle_sort();
        assert_eq!(app.process_sort, ProcessSort::Memory);

        app.cycle_sort();
        assert_eq!(app.process_sort, ProcessSort::Pid);
    }

    #[test]
    fn app_toggle_sort_order() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        assert!(app.sort_reverse);

        app.toggle_sort_order();
        assert!(!app.sort_reverse);

        app.toggle_sort_order();
        assert!(app.sort_reverse);
    }

    // ---- Process sorting tests ----

    #[test]
    fn sorted_by_cpu_desc() {
        let state = make_state();
        let app = TuiApp::new(state);
        // default: CPU desc
        let sorted = app.sorted_processes(&sample_processes());
        assert_eq!(sorted[0].name, "beta"); // 80%
        assert_eq!(sorted[1].name, "alpha"); // 50%
        assert_eq!(sorted[2].name, "gamma"); // 10%
    }

    #[test]
    fn sorted_by_cpu_asc() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        app.sort_reverse = false;
        let sorted = app.sorted_processes(&sample_processes());
        assert_eq!(sorted[0].name, "gamma"); // 10%
        assert_eq!(sorted[2].name, "beta"); // 80%
    }

    #[test]
    fn sorted_by_memory_desc() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        app.process_sort = ProcessSort::Memory;
        let sorted = app.sorted_processes(&sample_processes());
        assert_eq!(sorted[0].name, "gamma"); // 500MB
        assert_eq!(sorted[1].name, "alpha"); // 200MB
        assert_eq!(sorted[2].name, "beta"); // 100MB
    }

    #[test]
    fn sorted_by_pid_asc() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        app.process_sort = ProcessSort::Pid;
        app.sort_reverse = false;
        let sorted = app.sorted_processes(&sample_processes());
        assert_eq!(sorted[0].pid, 50);
        assert_eq!(sorted[1].pid, 100);
        assert_eq!(sorted[2].pid, 200);
    }

    #[test]
    fn sorted_by_name_asc() {
        let state = make_state();
        let mut app = TuiApp::new(state);
        app.process_sort = ProcessSort::Name;
        app.sort_reverse = false;
        let sorted = app.sorted_processes(&sample_processes());
        assert_eq!(sorted[0].name, "alpha");
        assert_eq!(sorted[1].name, "beta");
        assert_eq!(sorted[2].name, "gamma");
    }

    #[test]
    fn sorted_empty_processes() {
        let state = make_state();
        let app = TuiApp::new(state);
        let sorted = app.sorted_processes(&[]);
        assert!(sorted.is_empty());
    }

    // ---- FrameData tests ----

    #[test]
    fn frame_data_empty_state() {
        let state = make_state();
        let data = FrameData::from_state(&state, 80);
        assert!(data.snap.is_none());
        assert!(data.alerts.is_empty());
        assert!(data.predictions.is_empty());
        assert!(data.cpu_history.is_empty());
        assert!(data.mem_history.is_empty());
        assert!(!data.is_agnos);
        assert_eq!(data.sample_count, 0);
        assert_eq!(data.poll_secs, 5);
        assert_eq!(data.api_url, "http://127.0.0.1:8090");
    }

    #[test]
    fn frame_data_detects_agnos_by_agents() {
        let state = make_state();
        {
            let mut s = write_state(&state);
            s.latest = Some(SystemSnapshot {
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
                agents: AgentSummary {
                    total: 3,
                    running: 2,
                    idle: 1,
                    error: 0,
                    cpu_usage: HashMap::new(),
                    memory_usage: HashMap::new(),
                },
                services: vec![],
                top_processes: vec![],
            });
        }
        let data = FrameData::from_state(&state, 80);
        assert!(data.is_agnos);
    }

    #[test]
    fn frame_data_detects_agnos_by_daimon_service() {
        let state = make_state();
        {
            let mut s = write_state(&state);
            s.latest = Some(SystemSnapshot {
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
                services: vec![ServiceStatus {
                    name: "daimon".into(),
                    state: ServiceState::Running,
                    pid: Some(1234),
                    uptime_secs: Some(3600),
                    port: Some(8090),
                }],
                top_processes: vec![],
            });
        }
        let data = FrameData::from_state(&state, 80);
        assert!(data.is_agnos);
    }

    #[test]
    fn frame_data_not_agnos_without_agents_or_daimon() {
        let state = make_state();
        {
            let mut s = write_state(&state);
            s.latest = Some(SystemSnapshot {
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
            });
        }
        let data = FrameData::from_state(&state, 80);
        assert!(!data.is_agnos);
    }

    #[test]
    fn frame_data_not_agnos_daimon_stopped() {
        let state = make_state();
        {
            let mut s = write_state(&state);
            s.latest = Some(SystemSnapshot {
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
                services: vec![ServiceStatus {
                    name: "daimon".into(),
                    state: ServiceState::Stopped,
                    pid: None,
                    uptime_secs: None,
                    port: Some(8090),
                }],
                top_processes: vec![],
            });
        }
        let data = FrameData::from_state(&state, 80);
        assert!(!data.is_agnos);
    }

    #[test]
    fn frame_data_captures_history() {
        let state = make_state();
        {
            let mut s = write_state(&state);
            for i in 0..10 {
                s.cpu_history.push(i as f64 * 10.0);
                s.mem_history.push(i as f64 * 5.0);
            }
        }
        let data = FrameData::from_state(&state, 5);
        assert_eq!(data.cpu_history.len(), 5);
        assert_eq!(data.mem_history.len(), 5);
        assert_eq!(data.sample_count, 10);
    }

    #[test]
    fn frame_data_captures_alerts() {
        let state = make_state();
        {
            let mut s = write_state(&state);
            s.push_alerts(vec![
                Alert {
                    severity: AlertSeverity::Warning,
                    component: "cpu".into(),
                    message: "high usage".into(),
                    timestamp: chrono::Utc::now(),
                },
                Alert {
                    severity: AlertSeverity::Critical,
                    component: "disk".into(),
                    message: "full".into(),
                    timestamp: chrono::Utc::now(),
                },
            ]);
        }
        let data = FrameData::from_state(&state, 80);
        assert_eq!(data.alerts.len(), 2);
    }

    #[test]
    fn frame_data_captures_triage_and_recommendations() {
        let state = make_state();
        {
            let mut s = write_state(&state);
            s.last_triage = Some("CPU spike from process X".into());
            s.last_recommendations = Some("Kill process X".into());
        }
        let data = FrameData::from_state(&state, 80);
        assert_eq!(data.triage.as_deref(), Some("CPU spike from process X"));
        assert_eq!(data.recommendations.as_deref(), Some("Kill process X"));
    }
}
