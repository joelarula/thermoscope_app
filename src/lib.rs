pub mod device;
pub mod uvc_adapter;

use crate::device::ThermalDevice;
use crate::uvc_adapter::UvcAdapter;
use eframe::egui;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;

pub struct ThermalEngine {
    frame_tx: Sender<egui::ColorImage>,
}

impl ThermalEngine {
    pub fn new(frame_tx: Sender<egui::ColorImage>) -> Self {
        Self { frame_tx }
    }

    pub fn start(&self, vid: u16, pid: u16) {
        let tx = self.frame_tx.clone();

        thread::spawn(move || match ThermalDevice::standalone_unlock(vid, pid) {
            Ok(_) => {
                println!("✅ Hardware unlock successful. Waiting 1s for OS to refresh driver...");
                thread::sleep(Duration::from_millis(1000));

                let mut adapter = match UvcAdapter::new("libuvc.dll") {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("❌ Failed to load libuvc.dll: {}", e);
                        return;
                    }
                };

                match adapter.open_device(vid as i32, pid as i32) {
                    Ok(_) => {
                        println!("✅ libuvc: Device opened.");
                        let (raw_tx, raw_rx) = channel::<Vec<u8>>();
                        match adapter.start_streaming(raw_tx) {
                            Ok(_) => {
                                println!("🎬 libuvc: Streaming active!");
                                let width = 256;
                                let height = 192;

                                let mut local_count = 0;
                                while let Ok(frame_data) = raw_rx.recv() {
                                    if frame_data.len() >= width * height * 2 {
                                        local_count += 1;
                                        if local_count % 30 == 0 {
                                            println!(
                                                "🔥 Background: Processed frame {}...",
                                                local_count
                                            );
                                        }

                                        let mut min = u16::MAX;
                                        let mut max = u16::MIN;

                                        let raw_values: Vec<u16> = frame_data
                                            .chunks_exact(2)
                                            .take(width * height)
                                            .map(|chunk| {
                                                let val = u16::from_le_bytes([chunk[0], chunk[1]]);
                                                if val < min {
                                                    min = val;
                                                }
                                                if val > max {
                                                    max = val;
                                                }
                                                val
                                            })
                                            .collect();

                                        let mut rgb_image = egui::ColorImage::new(
                                            [width, height],
                                            egui::Color32::BLACK,
                                        );
                                        let range = (max - min) as f32;
                                        if range > 0.0 {
                                            let inv_range = 1.0 / range;
                                            for (i, pixel) in
                                                rgb_image.pixels.iter_mut().enumerate()
                                            {
                                                let t = (raw_values[i] - min) as f32 * inv_range;
                                                *pixel = if t < 0.25 {
                                                    egui::Color32::from_rgb(
                                                        0,
                                                        0,
                                                        (t * 1020.0) as u8,
                                                    )
                                                } else if t < 0.5 {
                                                    egui::Color32::from_rgb(
                                                        ((t - 0.25) * 1020.0) as u8,
                                                        0,
                                                        (255.0 - (t - 0.25) * 1020.0) as u8,
                                                    )
                                                } else if t < 0.75 {
                                                    egui::Color32::from_rgb(
                                                        255,
                                                        ((t - 0.5) * 1020.0) as u8,
                                                        0,
                                                    )
                                                } else {
                                                    egui::Color32::from_rgb(
                                                        255,
                                                        255,
                                                        ((t - 0.75) * 1020.0) as u8,
                                                    )
                                                };
                                            }
                                        }
                                        if tx.send(rgb_image).is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => eprintln!("❌ libuvc: Failed to start streaming: {}", e),
                        }
                    }
                    Err(e) => eprintln!("❌ libuvc: Failed to open device: {}", e),
                }
            }
            Err(e) => eprintln!("❌ Hardware unlock failed: {}", e),
        });
    }
}
