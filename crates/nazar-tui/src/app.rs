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
