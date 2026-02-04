use eframe::egui;
use std::sync::mpsc::{channel, Receiver};
use thermoscope_app::ThermalEngine;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("Thermoscope Pro"),
        ..Default::default()
    };
    eframe::run_native(
        "Thermoscope",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    )
}

struct MyApp {
    frame_rx: Receiver<egui::ColorImage>,
    texture: Option<egui::TextureHandle>,
    frame_count: u64,
    status: String,
}

impl Default for MyApp {
    fn default() -> Self {
        let (tx, rx) = channel();
        let app = Self {
            frame_rx: rx,
            texture: None,
            frame_count: 0,
            status: "Initializing...".to_string(),
        };

        // Start the thermal engine
        let engine = ThermalEngine::new(tx);
        engine.start(0x0bda, 0x5830);

        app
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain frames and only keep the last one
        let mut latest_image = None;
        while let Ok(image) = self.frame_rx.try_recv() {
            latest_image = Some(image);
            self.frame_count += 1;
        }

        if let Some(image) = latest_image {
            self.texture = Some(ctx.load_texture("thermal_feed", image, Default::default()));
            self.status = "✔ ACTIVE".to_string();
        }

        // Keep the UI thread polling even if no frame arrived this exact update
        ctx.request_repaint();

        // MINIMALISTIC UI
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                if let Some(texture) = &self.texture {
                    // Full window video scaling
                    ui.add(egui::Image::new(texture).fit_to_exact_size(ui.available_size()));
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new(&self.status)
                                .color(egui::Color32::LIGHT_GRAY)
                                .size(20.0),
                        );
                    });
                }

                // Overlay status
                let painter = ui.painter();
                let rect = ui.max_rect();
                painter.text(
                    rect.left_top() + egui::vec2(10.0, 10.0),
                    egui::Align2::LEFT_TOP,
                    format!("FPS: ~{}", self.frame_count / 30), // Extremely rough estimate
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_white_alpha(100),
                );
            });
    }
}
