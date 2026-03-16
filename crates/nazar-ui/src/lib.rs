//! Nazar UI — egui-based system monitor dashboard

use nazar_api::ApiClient;
use nazar_core::*;

/// Launch the Nazar GUI application.
pub fn run_app(api_url: &str) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Nazar — System Monitor")
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    let api_url = api_url.to_string();
    let _ = eframe::run_native(
        "Nazar",
        options,
        Box::new(move |_cc| Ok(Box::new(NazarApp::new(&api_url)))),
    );
}

struct NazarApp {
    config: NazarConfig,
    cpu_series: TimeSeries,
    mem_series: TimeSeries,
    last_memory: Option<MemoryMetrics>,
}

impl NazarApp {
    fn new(api_url: &str) -> Self {
        let config = NazarConfig {
            api_url: api_url.to_string(),
            ..NazarConfig::default()
        };

        Self {
            config,
            cpu_series: TimeSeries::new("CPU Usage", "%", 720),
            mem_series: TimeSeries::new("Memory Usage", "%", 720),
            last_memory: None,
        }
    }

    fn poll_local_metrics(&mut self) {
        let mem = ApiClient::read_memory_metrics();
        self.mem_series.push(mem.used_percent());
        self.last_memory = Some(mem);

        let cpu = ApiClient::read_cpu_metrics();
        self.cpu_series.push(cpu.load_average[0]);
    }
}

impl eframe::App for NazarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_local_metrics();

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Nazar — System Monitor");
                ui.separator();
                ui.label(format!("Connected: {}", self.config.api_url));
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |cols| {
                // Left column: CPU and Memory
                cols[0].group(|ui| {
                    ui.heading("CPU");
                    if let Some(load) = self.cpu_series.latest() {
                        ui.label(format!("Load Average (1m): {:.2}", load));
                    }
                    let cpu_bar = self.cpu_series.latest().unwrap_or(0.0) / 8.0; // rough scale
                    ui.add(egui::ProgressBar::new(cpu_bar.min(1.0) as f32).text("CPU"));
                });

                cols[0].add_space(10.0);

                cols[0].group(|ui| {
                    ui.heading("Memory");
                    if let Some(ref mem) = self.last_memory {
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
                    }
                });

                // Right column: Services and status
                cols[1].group(|ui| {
                    ui.heading("AGNOS Services");
                    ui.label("● daimon (port 8090)");
                    ui.label("● hoosh (port 8088)");
                    ui.label("● phylax (threat scanner)");
                });

                cols[1].add_space(10.0);

                cols[1].group(|ui| {
                    ui.heading("Quick Stats");
                    let stats = format!(
                        "History points: CPU={}, Mem={}",
                        self.cpu_series.points.len(),
                        self.mem_series.points.len()
                    );
                    ui.label(stats);
                });
            });
        });

        // Request repaint at refresh interval
        ctx.request_repaint_after(std::time::Duration::from_millis(self.config.ui_refresh_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nazar_app_creates() {
        let app = NazarApp::new("http://127.0.0.1:8090");
        assert_eq!(app.config.api_url, "http://127.0.0.1:8090");
        assert_eq!(app.cpu_series.points.len(), 0);
    }

    #[test]
    fn nazar_app_poll_local() {
        let mut app = NazarApp::new("http://127.0.0.1:8090");
        app.poll_local_metrics();
        assert_eq!(app.cpu_series.points.len(), 1);
        assert_eq!(app.mem_series.points.len(), 1);
        assert!(app.last_memory.is_some());
    }
}
