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

    fn draw_header(&self, ui: &mut egui::Ui, api_url: &str, has_snapshot: bool, sample_count: usize) {
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
                    "Usage: {:.1}%  |  Load: {:.2} / {:.2} / {:.2}  |  Running: {}",
                    snap.cpu.total_percent,
                    snap.cpu.load_average[0],
                    snap.cpu.load_average[1],
                    snap.cpu.load_average[2],
                    snap.cpu.processes,
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
            ui.add(
                egui::ProgressBar::new((mem.used_percent() / 100.0) as f32)
                    .text("RAM"),
            );
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
                    ui.label(format!(
                        "{} ({}) — {:.1} GB / {:.1} GB",
                        d.mount_point,
                        d.device,
                        d.used_bytes as f64 / 1e9,
                        d.total_bytes as f64 / 1e9,
                    ));
                    ui.add(
                        egui::ProgressBar::new((d.used_percent() / 100.0) as f32)
                            .text(format!("{:.1}%", d.used_percent())),
                    );
                }
            }
        });
    }

    fn draw_network_panel(&self, ui: &mut egui::Ui, snap: &SystemSnapshot) {
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
                        ui.label(format!(
                            "{} ({})",
                            svc.name, svc.state
                        ));
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

    fn draw_predictions_panel(&self, ui: &mut egui::Ui, predictions: &[PredictionResult], poll_secs: u64) {
        ui.group(|ui| {
            ui.heading("Predictions");
            for pred in predictions {
                let mins = (pred.intervals_until * poll_secs) / 60;
                ui.label(format!(
                    "{}: {:.1}% now → {:.1}% in ~{} min (trend: {:?})",
                    pred.metric,
                    pred.current_value,
                    pred.predicted_value,
                    mins,
                    pred.trend,
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
            let (snap, cpu_data, mem_data, predictions, poll_secs) = {
                let s = read_state(&self.state);
                let snap = s.latest.clone().unwrap();
                let cpu_data = s.cpu_history.last_n(120);
                let mem_data = s.mem_history.last_n(120);
                let predictions = s.predictions.clone();
                let poll_secs = s.config.poll_interval_secs;
                (snap, cpu_data, mem_data, predictions, poll_secs)
            };

            egui::ScrollArea::vertical().show(ui, |ui| {
                self.draw_cpu_panel(ui, &snap, cpu_data);
                ui.add_space(8.0);
                self.draw_memory_panel(ui, &snap, mem_data);
                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    self.draw_disk_panel(&mut cols[0], &snap);
                    self.draw_network_panel(&mut cols[1], &snap);
                });

                ui.add_space(8.0);
                self.draw_services_panel(ui, &snap);

                if !predictions.is_empty() {
                    ui.add_space(8.0);
                    self.draw_predictions_panel(ui, &predictions, poll_secs);
                }
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
