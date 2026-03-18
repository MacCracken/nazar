//! Nazar UI — egui-based system monitor dashboard

use nazar_core::*;

/// Launch the Nazar GUI application with shared state.
pub fn run_app(state: SharedState) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Nazar — System Monitor")
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Nazar",
        options,
        Box::new(move |_cc| Ok(Box::new(NazarApp::new(state)))),
    );
}

struct NazarApp {
    state: SharedState,
}

impl NazarApp {
    fn new(state: SharedState) -> Self {
        Self { state }
    }

    fn draw_header(
        &self,
        ui: &mut egui::Ui,
        api_url: &str,
        has_snapshot: bool,
        sample_count: usize,
    ) {
        ui.horizontal(|ui| {
            ui.heading("Nazar — System Monitor");
            ui.separator();
            ui.label(format!("daimon: {api_url}"));
            if has_snapshot {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{sample_count} samples collected"));
                });
            }
        });
    }

    fn draw_alerts_panel(&self, ui: &mut egui::Ui) {
        ui.heading("Alerts");
        let alerts = {
            let s = read_state(&self.state);
            s.alerts.clone()
        };
        if alerts.is_empty() {
            ui.label("No active alerts");
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for alert in alerts.iter().rev().take(20) {
                    let color = match alert.severity {
                        AlertSeverity::Critical => egui::Color32::RED,
                        AlertSeverity::Warning => egui::Color32::YELLOW,
                        AlertSeverity::Info => egui::Color32::LIGHT_GRAY,
                    };
                    let age = chrono::Utc::now() - alert.timestamp;
                    let age_str = if age.num_hours() > 0 {
                        format!("{}h ago", age.num_hours())
                    } else if age.num_minutes() > 0 {
                        format!("{}m ago", age.num_minutes())
                    } else {
                        format!("{}s ago", age.num_seconds())
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("[{}]", alert.severity));
                        ui.label(&alert.component);
                        ui.label(&alert.message);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.weak(&age_str);
                        });
                    });
                }
            });
        }
    }

    fn draw_cpu_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot, cpu_data: Vec<f64>) {
        ui.group(|ui| {
            ui.heading("CPU");
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Usage: {:.1}%  |  Load: {:.2} / {:.2} / {:.2}  |  Procs: {}  Threads: {}",
                    snap.cpu.total_percent,
                    snap.cpu.load_average[0],
                    snap.cpu.load_average[1],
                    snap.cpu.load_average[2],
                    snap.cpu.processes,
                    snap.cpu.threads,
                ));
            });
            ui.add(
                egui::ProgressBar::new((snap.cpu.total_percent / 100.0) as f32)
                    .text(format!("{:.1}%", snap.cpu.total_percent)),
            );

            if cpu_data.len() > 1 {
                let line_points: egui_plot::PlotPoints = cpu_data
                    .iter()
                    .enumerate()
                    .map(|(i, v)| [i as f64, *v])
                    .collect();
                let line = egui_plot::Line::new("CPU %", line_points);
                egui_plot::Plot::new("cpu_plot")
                    .height(80.0)
                    .include_y(0.0)
                    .include_y(100.0)
                    .show_axes(false)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .allow_scroll(false)
                    .show(ui, |plot_ui| {
                        plot_ui.line(line);
                    });
            }

            if !snap.cpu.cores.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for (i, pct) in snap.cpu.cores.iter().enumerate() {
                        ui.add(
                            egui::ProgressBar::new((*pct / 100.0) as f32)
                                .text(format!("C{i}: {pct:.0}%"))
                                .desired_width(80.0),
                        );
                    }
                });
            }
        });
    }

    fn draw_memory_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot, mem_data: Vec<f64>) {
        ui.group(|ui| {
            ui.heading("Memory");
            let mem = &snap.memory;
            ui.label(format!(
                "Used: {:.1} GB / {:.1} GB ({:.1}%)",
                mem.used_bytes as f64 / 1e9,
                mem.total_bytes as f64 / 1e9,
                mem.used_percent()
            ));
            ui.add(egui::ProgressBar::new((mem.used_percent() / 100.0) as f32).text("RAM"));
            if mem.swap_total_bytes > 0 {
                ui.label(format!(
                    "Swap: {:.1} GB / {:.1} GB ({:.1}%)",
                    mem.swap_used_bytes as f64 / 1e9,
                    mem.swap_total_bytes as f64 / 1e9,
                    mem.swap_used_percent()
                ));
            }

            if mem_data.len() > 1 {
                let line_points: egui_plot::PlotPoints = mem_data
                    .iter()
                    .enumerate()
                    .map(|(i, v)| [i as f64, *v])
                    .collect();
                let line = egui_plot::Line::new("Mem %", line_points);
                egui_plot::Plot::new("mem_plot")
                    .height(80.0)
                    .include_y(0.0)
                    .include_y(100.0)
                    .show_axes(false)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .allow_scroll(false)
                    .show(ui, |plot_ui| {
                        plot_ui.line(line);
                    });
            }
        });
    }

    fn draw_disk_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot) {
        ui.group(|ui| {
            ui.heading("Disk");
            if snap.disk.is_empty() {
                ui.label("No disk data");
            } else {
                for d in &snap.disk {
                    let io_str = if d.read_bytes > 0 || d.write_bytes > 0 {
                        format!(
                            "  R: {:.0} KB  W: {:.0} KB",
                            d.read_bytes as f64 / 1024.0,
                            d.write_bytes as f64 / 1024.0
                        )
                    } else {
                        String::new()
                    };
                    ui.label(format!(
                        "{} ({}) — {:.1} GB / {:.1} GB{}",
                        d.mount_point,
                        d.device,
                        d.used_bytes as f64 / 1e9,
                        d.total_bytes as f64 / 1e9,
                        io_str,
                    ));
                    ui.add(
                        egui::ProgressBar::new((d.used_percent() / 100.0) as f32)
                            .text(format!("{:.1}%", d.used_percent())),
                    );
                }
            }
        });
    }

    fn draw_network_panel(
        &self,
        ui: &mut egui::Ui,
        snap: &SystemSnapshot,
        iface_history: &std::collections::HashMap<String, (Vec<f64>, Vec<f64>)>,
    ) {
        ui.group(|ui| {
            ui.heading("Network");
            let net = &snap.network;
            ui.label(format!(
                "RX: {:.1} KB/s  |  TX: {:.1} KB/s  |  Connections: {}",
                net.total_rx_bytes as f64 / 1024.0,
                net.total_tx_bytes as f64 / 1024.0,
                net.active_connections,
            ));
            for iface in &net.interfaces {
                if iface.name == "lo" {
                    continue;
                }
                ui.label(format!(
                    "  {} — RX: {:.1} KB  TX: {:.1} KB{}",
                    iface.name,
                    iface.rx_bytes as f64 / 1024.0,
                    iface.tx_bytes as f64 / 1024.0,
                    if iface.rx_errors > 0 || iface.tx_errors > 0 {
                        format!("  (errors: rx={} tx={})", iface.rx_errors, iface.tx_errors)
                    } else {
                        String::new()
                    },
                ));
                // Per-interface sparkline
                if let Some((rx_data, tx_data)) = iface_history.get(&iface.name)
                    && rx_data.len() > 1
                {
                    let rx_points: egui_plot::PlotPoints = rx_data
                        .iter()
                        .enumerate()
                        .map(|(i, v)| [i as f64, *v / 1024.0])
                        .collect();
                    let tx_points: egui_plot::PlotPoints = tx_data
                        .iter()
                        .enumerate()
                        .map(|(i, v)| [i as f64, *v / 1024.0])
                        .collect();
                    egui_plot::Plot::new(format!("net_{}", iface.name))
                        .height(50.0)
                        .show_axes(false)
                        .allow_drag(false)
                        .allow_zoom(false)
                        .allow_scroll(false)
                        .show(ui, |plot_ui| {
                            plot_ui.line(egui_plot::Line::new("RX KB/s", rx_points));
                            plot_ui.line(egui_plot::Line::new("TX KB/s", tx_points));
                        });
                }
            }
        });
    }

    fn draw_gpu_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot) {
        if snap.gpu.is_empty() {
            return;
        }
        ui.group(|ui| {
            ui.heading("GPU");
            for g in &snap.gpu {
                ui.label(format!("{} ({}) — {}", g.name, g.driver, g.id));
                ui.horizontal(|ui| {
                    ui.label(format!("Usage: {:.0}%", g.utilization_percent));
                    ui.separator();
                    ui.label(format!(
                        "VRAM: {:.0} MB / {:.0} MB ({:.1}%)",
                        g.vram_used_bytes as f64 / 1e6,
                        g.vram_total_bytes as f64 / 1e6,
                        g.vram_used_percent()
                    ));
                    if let Some(temp) = g.temp_celsius {
                        ui.separator();
                        ui.label(format!("{:.0}°C", temp));
                    }
                    if let Some(power) = g.power_watts {
                        ui.separator();
                        ui.label(format!("{:.1}W", power));
                    }
                    if let Some(clock) = g.clock_mhz {
                        ui.separator();
                        ui.label(format!("{} MHz", clock));
                    }
                });
                ui.add(
                    egui::ProgressBar::new((g.utilization_percent / 100.0) as f32)
                        .text(format!("GPU {:.0}%", g.utilization_percent)),
                );
                ui.add(
                    egui::ProgressBar::new((g.vram_used_percent() / 100.0) as f32)
                        .text(format!("VRAM {:.1}%", g.vram_used_percent())),
                );
            }
        });
    }

    fn draw_temperatures_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot) {
        if snap.temperatures.is_empty() {
            return;
        }
        ui.group(|ui| {
            ui.heading("Temperatures");
            for t in &snap.temperatures {
                let crit_str = if let Some(crit) = t.critical_celsius {
                    format!(" / {:.0}°C crit", crit)
                } else {
                    String::new()
                };
                let color = if t.critical_celsius.is_some_and(|c| t.temp_celsius > c * 0.9) {
                    egui::Color32::RED
                } else if t.temp_celsius > 70.0 {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::LIGHT_GRAY
                };
                ui.colored_label(
                    color,
                    format!("{}: {:.1}°C{}", t.label, t.temp_celsius, crit_str),
                );
            }
        });
    }

    fn draw_services_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot) {
        ui.group(|ui| {
            ui.heading("Services");
            if snap.services.is_empty() {
                ui.label("No service data (daimon not connected)");
            } else {
                for svc in &snap.services {
                    let (icon, color) = match svc.state {
                        ServiceState::Running => ("●", egui::Color32::GREEN),
                        ServiceState::Failed => ("●", egui::Color32::RED),
                        ServiceState::Stopped => ("●", egui::Color32::GRAY),
                        _ => ("●", egui::Color32::YELLOW),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, icon);
                        ui.label(format!("{} ({})", svc.name, svc.state));
                        if let Some(port) = svc.port {
                            ui.label(format!("port {port}"));
                        }
                        if let Some(up) = svc.uptime_secs {
                            let h = up / 3600;
                            let m = (up % 3600) / 60;
                            ui.label(format!("up {h}h {m}m"));
                        }
                    });
                }
            }
        });
    }

    fn draw_agents_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot) {
        let agents = &snap.agents;
        if agents.total == 0 {
            return;
        }
        ui.group(|ui| {
            ui.heading("Agents");
            ui.label(format!(
                "Total: {}  Running: {}  Idle: {}  Error: {}",
                agents.total, agents.running, agents.idle, agents.error
            ));

            if !agents.cpu_usage.is_empty() || !agents.memory_usage.is_empty() {
                egui::Grid::new("agent_grid")
                    .striped(true)
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        ui.strong("Agent");
                        ui.strong("CPU %");
                        ui.strong("Memory");
                        ui.end_row();

                        // Merge CPU and memory maps
                        let mut agent_ids: Vec<&String> = agents
                            .cpu_usage
                            .keys()
                            .chain(agents.memory_usage.keys())
                            .collect::<std::collections::HashSet<_>>()
                            .into_iter()
                            .collect();
                        agent_ids.sort();

                        for id in agent_ids {
                            ui.label(id);
                            if let Some(cpu) = agents.cpu_usage.get(id) {
                                ui.label(format!("{:.1}", cpu));
                            } else {
                                ui.label("-");
                            }
                            if let Some(mem) = agents.memory_usage.get(id) {
                                ui.label(format!("{:.1} MB", *mem as f64 / 1e6));
                            } else {
                                ui.label("-");
                            }
                            ui.end_row();
                        }
                    });
            }
        });
    }

    fn draw_ai_insights_panel(
        &self,
        ui: &mut egui::Ui,
        triage: &Option<String>,
        recommendations: &Option<String>,
    ) {
        if triage.is_none() && recommendations.is_none() {
            return;
        }
        ui.group(|ui| {
            ui.heading("AI Insights");
            if let Some(t) = triage {
                ui.label("Alert Triage:");
                ui.indent("triage", |ui| {
                    ui.label(t);
                });
            }
            if let Some(r) = recommendations {
                ui.label("Process Recommendations:");
                ui.indent("recs", |ui| {
                    ui.label(r);
                });
            }
        });
    }

    fn draw_processes_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot) {
        ui.group(|ui| {
            ui.heading("Top Processes");
            if snap.top_processes.is_empty() {
                ui.label("No process data");
                return;
            }
            egui::Grid::new("proc_grid")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.strong("PID");
                    ui.strong("Name");
                    ui.strong("CPU %");
                    ui.strong("Memory");
                    ui.strong("Mem %");
                    ui.strong("State");
                    ui.strong("Threads");
                    ui.end_row();

                    for p in &snap.top_processes {
                        ui.label(p.pid.to_string());
                        ui.label(&p.name);
                        ui.label(format!("{:.1}", p.cpu_percent));
                        ui.label(format!("{:.1} MB", p.memory_bytes as f64 / 1e6));
                        ui.label(format!("{:.1}", p.memory_percent));
                        ui.label(p.state.to_string());
                        ui.label(p.threads.to_string());
                        ui.end_row();
                    }
                });
        });
    }

    fn draw_predictions_panel(
        &self,
        ui: &mut egui::Ui,
        predictions: &[PredictionResult],
        poll_secs: u64,
    ) {
        ui.group(|ui| {
            ui.heading("Predictions");
            for pred in predictions {
                let mins = (pred.intervals_until * poll_secs) / 60;
                ui.label(format!(
                    "{}: {:.1}% now → {:.1}% in ~{} min (trend: {:?})",
                    pred.metric, pred.current_value, pred.predicted_value, mins, pred.trend,
                ));
            }
        });
    }
}

impl eframe::App for NazarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let refresh_ms;
        let api_url;
        let has_snapshot;
        let sample_count;

        {
            let s = read_state(&self.state);
            refresh_ms = s.config.ui_refresh_ms;
            api_url = s.config.api_url.clone();
            has_snapshot = s.latest.is_some();
            sample_count = s.cpu_history.points.len();
        }

        // ---- Header ----
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            self.draw_header(ui, &api_url, has_snapshot, sample_count);
        });

        // ---- Bottom: alerts ----
        egui::TopBottomPanel::bottom("alerts_panel")
            .resizable(true)
            .min_height(80.0)
            .max_height(250.0)
            .show(ctx, |ui| {
                self.draw_alerts_panel(ui);
            });

        // ---- Central panel ----
        egui::CentralPanel::default().show(ctx, |ui| {
            if !has_snapshot {
                ui.centered_and_justified(|ui| {
                    ui.label("Waiting for first metrics snapshot...");
                });
                return;
            }

            // Clone all needed data out of the lock to avoid holding it during rendering
            let (
                snap,
                cpu_data,
                mem_data,
                iface_history,
                predictions,
                poll_secs,
                triage,
                recommendations,
            ) = {
                let s = read_state(&self.state);
                let snap = s.latest.clone().unwrap();
                let cpu_data = s.cpu_history.last_n(120);
                let mem_data = s.mem_history.last_n(120);
                let iface_history: std::collections::HashMap<String, (Vec<f64>, Vec<f64>)> = s
                    .net_iface_history
                    .iter()
                    .map(|(k, (rx, tx))| (k.clone(), (rx.last_n(60), tx.last_n(60))))
                    .collect();
                let predictions = s.predictions.clone();
                let poll_secs = s.config.poll_interval_secs;
                let triage = s.last_triage.clone();
                let recommendations = s.last_recommendations.clone();
                (
                    snap,
                    cpu_data,
                    mem_data,
                    iface_history,
                    predictions,
                    poll_secs,
                    triage,
                    recommendations,
                )
            };

            egui::ScrollArea::vertical().show(ui, |ui| {
                self.draw_cpu_panel(ui, &snap, cpu_data);
                ui.add_space(8.0);
                self.draw_memory_panel(ui, &snap, mem_data);
                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    self.draw_disk_panel(&mut cols[0], &snap);
                    self.draw_network_panel(&mut cols[1], &snap, &iface_history);
                });

                ui.add_space(8.0);
                self.draw_gpu_panel(ui, &snap);
                self.draw_temperatures_panel(ui, &snap);

                ui.add_space(8.0);
                self.draw_processes_panel(ui, &snap);

                ui.add_space(8.0);
                self.draw_agents_panel(ui, &snap);

                ui.add_space(8.0);
                self.draw_services_panel(ui, &snap);

                if !predictions.is_empty() {
                    ui.add_space(8.0);
                    self.draw_predictions_panel(ui, &predictions, poll_secs);
                }

                self.draw_ai_insights_panel(ui, &triage, &recommendations);
            });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(refresh_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nazar_app_creates() {
        let state = new_shared_state(NazarConfig::default());
        let app = NazarApp::new(state);
        let s = read_state(&app.state);
        assert_eq!(s.config.api_url, "http://127.0.0.1:8090");
    }
}
