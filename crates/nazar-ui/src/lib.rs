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
}

impl eframe::App for NazarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let refresh_ms;
        let api_url;
        let has_snapshot;
        let sample_count;

        {
            let s = self.state.read().unwrap();
            refresh_ms = s.config.ui_refresh_ms;
            api_url = s.config.api_url.clone();
            has_snapshot = s.latest.is_some();
            sample_count = s.cpu_history.points.len();
        }

        // ---- Header ----
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
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
        });

        // ---- Bottom: alerts ----
        egui::TopBottomPanel::bottom("alerts_panel")
            .resizable(true)
            .min_height(80.0)
            .max_height(250.0)
            .show(ctx, |ui| {
                ui.heading("Alerts");
                let s = self.state.read().unwrap();
                if s.alerts.is_empty() {
                    ui.label("No active alerts");
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for alert in s.alerts.iter().rev().take(20) {
                            let color = match alert.severity {
                                AlertSeverity::Critical => egui::Color32::RED,
                                AlertSeverity::Warning => egui::Color32::YELLOW,
                                AlertSeverity::Info => egui::Color32::LIGHT_GRAY,
                            };
                            ui.horizontal(|ui| {
                                ui.colored_label(color, format!("[{}]", alert.severity));
                                ui.label(&alert.component);
                                ui.label(&alert.message);
                            });
                        }
                    });
                }
            });

        // ---- Central panel ----
        egui::CentralPanel::default().show(ctx, |ui| {
            if !has_snapshot {
                ui.centered_and_justified(|ui| {
                    ui.label("Waiting for first metrics snapshot...");
                });
                return;
            }

            let s = self.state.read().unwrap();
            let snap = s.latest.as_ref().unwrap();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // ---- CPU ----
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

                    // Sparkline
                    let cpu_data = s.cpu_history.last_n(120);
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

                    // Per-core bars
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

                ui.add_space(8.0);

                // ---- Memory ----
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

                    let mem_data = s.mem_history.last_n(120);
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

                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    // ---- Disk ----
                    cols[0].group(|ui| {
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

                    // ---- Network ----
                    cols[1].group(|ui| {
                        ui.heading("Network");
                        let net = &snap.network;
                        ui.label(format!(
                            "Total RX: {:.1} MB  |  TX: {:.1} MB  |  Connections: {}",
                            net.total_rx_bytes as f64 / 1e6,
                            net.total_tx_bytes as f64 / 1e6,
                            net.active_connections,
                        ));
                        for iface in &net.interfaces {
                            if iface.name == "lo" {
                                continue;
                            }
                            ui.label(format!(
                                "  {} — RX: {:.1} MB  TX: {:.1} MB{}",
                                iface.name,
                                iface.rx_bytes as f64 / 1e6,
                                iface.tx_bytes as f64 / 1e6,
                                if iface.rx_errors > 0 || iface.tx_errors > 0 {
                                    format!("  (errors: rx={} tx={})", iface.rx_errors, iface.tx_errors)
                                } else {
                                    String::new()
                                },
                            ));
                        }
                    });
                });

                ui.add_space(8.0);

                // ---- Services ----
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

                // ---- Predictions ----
                if !s.predictions.is_empty() {
                    ui.add_space(8.0);
                    ui.group(|ui| {
                        ui.heading("Predictions");
                        for pred in &s.predictions {
                            let poll_secs = s.config.poll_interval_secs;
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
        let s = app.state.read().unwrap();
        assert_eq!(s.config.api_url, "http://127.0.0.1:8090");
    }
}
