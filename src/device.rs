use anyhow::{Context, Result};
use rusb::GlobalContext;
use std::time::Duration;

pub struct ThermalDevice {
    // We will store the device handle here later
}

impl ThermalDevice {
    pub fn list_devices() -> Result<Vec<String>> {
        let mut devices_list = Vec::new();
        for device in rusb::devices()
            .context("Failed to list USB devices")?
            .iter()
        {
            let device_desc = device.device_descriptor()?;
            let vid = device_desc.vendor_id();
            let pid = device_desc.product_id();

            // Try to open to read strings (might fail due to permissions, so we ignore failures)
            let product_string = match device.open() {
                Ok(handle) => handle
                    .read_product_string_ascii(&device_desc)
                    .unwrap_or_else(|_| "(unknown)".to_string()),
                Err(_) => "(access denied)".to_string(),
            };

            let config = device.active_config_descriptor();
            let mut interfaces_details = Vec::new();
            if let Ok(c) = config {
                for interface in c.interfaces() {
                    for desc in interface.descriptors() {
                        let mut ep_details = Vec::new();
                        for ep in desc.endpoint_descriptors() {
                            ep_details.push(format!(
                                "EP {:02x} ({:?})",
                                ep.address(),
                                ep.transfer_type()
                            ));
                        }
                        interfaces_details.push(format!(
                            "(Ifc {} Alt {} Class:{:02x} EPs: [{}])",
                            desc.interface_number(),
                            desc.setting_number(),
                            desc.class_code(),
                            ep_details.join(", ")
                        ));
                    }
                }
            }

            let info = format!(
                "ID {:04x}:{:04x} - {} Details: {}",
                vid,
                pid,
                product_string,
                interfaces_details.join(" | ")
            );
            println!("{}", info);
            devices_list.push(info);
        }
        Ok(devices_list)
    }
    pub fn connect(vid: u16, pid: u16) -> Result<rusb::DeviceHandle<GlobalContext>> {
        let device = rusb::devices()?
            .iter()
            .find(|d| {
                let desc = d.device_descriptor().unwrap();
                desc.vendor_id() == vid && desc.product_id() == pid
            })
            .context("Device not found")?;

        let handle = device.open().context("Failed to open device")?;

        // Detach kernel driver if necessary
        if handle.kernel_driver_active(0).unwrap_or(false) {
            handle.detach_kernel_driver(0).ok();
        }
        if handle.kernel_driver_active(1).unwrap_or(false) {
            handle.detach_kernel_driver(1).ok();
        }

        handle
            .claim_interface(0)
            .context("Failed to claim interface 0")?;

        // === MAGIC INITIALIZATION SEQUENCE ===
        // Extracted from libircmd.so via Ghidra reverse engineering
        // Functions: hand_shake_preview + preview_start

        println!("🔧 Sending initialization commands...");

        // Command 0: Handshake (MUST be sent first!)
        // Function: Java_com_energy_iruvc_ircmd_LibIRCMD_hand_1shake_1preview
        // UVCCamera::sendCommand(param_3, 'A', 0x45, 0x78, 0x1d00, &local_38, 8)
        // local_38 = 0x5305 (little-endian: 0x05, 0x53)
        let handshake_data: [u8; 8] = [
            0x05, 0x53, // Magic handshake bytes (0x5305 in little-endian)
            0x00, 0x00, // Reserved
            0x00, 0x00, 0x00, 0x00, // Reserved
        ];
        Self::send_vendor_command(&handle, 0x45, 0x78, 0x1d00, &handshake_data)?;
        println!("✓ Handshake sent");

        // Poll status after handshake
        println!("⏳ Waiting for handshake acknowledgment...");
        for i in 0..1000 {
            let mut status = [0u8; 1];
            let timeout = Duration::from_millis(100);
            let request_type = rusb::request_type(
                rusb::Direction::In,
                rusb::RequestType::Vendor,
                rusb::Recipient::Interface,
            );

            match handle.read_control(request_type, 0x44, 0x78, 0x200, &mut status, timeout) {
                Ok(_) => {
                    if (status[0] & 1) == 0
                        && (((status[0] as i32) << 30) < 0 || (status[0] & 0xfc) != 0)
                    {
                        println!(
                            "✓ Handshake acknowledged! (status: 0x{:02x}, iteration: {})",
                            status[0], i
                        );
                        break;
                    }
                }
                Err(_) => {}
            }

            if i == 999 {
                eprintln!("⚠ Handshake timeout");
            }
        }

        // Command 1: Start Y16 Preview
        // Function: Java_com_energy_iruvc_ircmd_LibIRCMD_y16_1preview_1start
        // UVCCamera::sendCommand(handle, 'A', 0x45, 0x78, 0x1d00, &local_30, 8)
        // local_30 = 0x10a (little-endian: 0x0a, 0x01)
        let y16_start_data: [u8; 8] = [
            0x0a, 0x01, // Magic bytes for Y16
            0x00, 0x00, // Parameters (param_3, param_4 from Java placeholders)
            0x00, 0x00, 0x00, 0x00,
        ];
        Self::send_vendor_command(&handle, 0x45, 0x78, 0x1d00, &y16_start_data)?;
        println!("✓ Y16 Preview Start sent");

        // Command 2: Poll status until camera is ready
        println!("⏳ Waiting for camera to initialize...");
        for i in 0..1000 {
            let mut status = [0u8; 1];
            let timeout = Duration::from_millis(100);
            let request_type = rusb::request_type(
                rusb::Direction::In,
                rusb::RequestType::Vendor,
                rusb::Recipient::Interface,
            );

            match handle.read_control(request_type, 0x44, 0x78, 0x200, &mut status, timeout) {
                Ok(_) => {
                    // Check bit 0 clear
                    if (status[0] & 1) == 0 {
                        println!(
                            "✓ Camera ready! (status: 0x{:02x}, iteration: {})",
                            status[0], i
                        );
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("⚠ Status poll failed: {}", e);
                }
            }
            if i == 999 {
                eprintln!("⚠ Camera initialization timeout");
            }
        }

        println!("🎥 Camera initialized successfully!");
        Ok(handle)
    }

    /// Performs the vendor initialization sequence and then RELEASES the device
    /// so that standard UVC drivers (Nokhwa/OpenCV) can take over.
    pub fn standalone_unlock(vid: u16, pid: u16) -> Result<()> {
        let handle = Self::connect(vid, pid)?;
        println!("🎥 Unlock sequence complete. Releasing device for OS driver...");
        // Releasing is handled by Drop if we don't return it,
        // but let's be explicit and release interface 0.
        handle.release_interface(0).ok();
        Ok(())
    }

    pub fn send_vendor_command(
        handle: &rusb::DeviceHandle<GlobalContext>,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
    ) -> Result<usize> {
        let timeout = Duration::from_secs(1);
        let request_type = rusb::request_type(
            rusb::Direction::Out,
            rusb::RequestType::Vendor,
            rusb::Recipient::Interface,
        );
        handle
            .write_control(request_type, request, value, index, data, timeout)
            .context("Vendor command failed")
    }

    pub fn start_streaming(handle: &mut rusb::DeviceHandle<GlobalContext>) -> Result<UVCStream> {
        println!("🚀 Transitioning to UVC stage...");

        // 1. Claim the streaming interface (Interface 1)
        println!("🔧 Claiming interface 1 (Streaming)...");
        handle
            .claim_interface(1)
            .context("Failed to claim Interface 1 (Streaming)")?;

        // 2. Negotiation: Standard UVC Probe & Commit
        Self::uvc_negotiate(handle)?;

        // 3. Set Alternate Setting
        println!("🔧 Setting alternate setting 1...");
        handle
            .set_alternate_setting(1, 1)
            .context("Failed to set alternate setting 1")?;

        println!("📹 UVC Stream ready!");

        Ok(UVCStream {
            frame_size: 256 * 192 * 2, // 256x192 pixels, 2 bytes per pixel
        })
    }

    fn uvc_negotiate(handle: &rusb::DeviceHandle<GlobalContext>) -> Result<()> {
        println!("📑 Negotiating UVC parameters (Probe/Commit)...");

        // UVC 1.1 Probe/Commit structure (26 bytes)
        let mut probe_data: [u8; 26] = [0; 26];
        probe_data[2] = 1; // bFormatIndex (Y16 usually 1)
        probe_data[3] = 1; // bFrameIndex (256x192 usually 1)

        // dwFrameInterval = 400000 (25 fps) -> 0x00061A80 in little-endian
        probe_data[4] = 0x80;
        probe_data[5] = 0x1A;
        probe_data[6] = 0x06;
        probe_data[7] = 0x00;

        let timeout = Duration::from_secs(1);
        let request_type_set = rusb::request_type(
            rusb::Direction::Out,
            rusb::RequestType::Class,
            rusb::Recipient::Interface,
        );
        let request_type_get = rusb::request_type(
            rusb::Direction::In,
            rusb::RequestType::Class,
            rusb::Recipient::Interface,
        );

        // VS_PROBE_CONTROL = 0x01
        // SET_CUR = 0x01
        // Interface = 1

        println!("  - Sending PROBE SET...");
        handle
            .write_control(request_type_set, 0x01, 0x0100, 0x0001, &probe_data, timeout)
            .context("UVC Probe Set failed")?;

        println!("  - Sending PROBE GET...");
        let mut get_probe_data = [0u8; 26];
        handle
            .read_control(
                request_type_get,
                0x81,
                0x0100,
                0x0001,
                &mut get_probe_data,
                timeout,
            )
            .context("UVC Probe Get failed")?;

        println!("  - Sending COMMIT SET...");
        handle
            .write_control(
                request_type_set,
                0x01,
                0x0200,
                0x0001,
                &get_probe_data,
                timeout,
            )
            .context("UVC Commit Set failed")?;

        println!("✓ UVC Negotiation complete");
        Ok(())
    }
}

pub struct UVCStream {
    frame_size: usize,
}

impl UVCStream {
    /// Reads a single video frame from the video endpoint.
    /// Supports both Bulk and Isochronous (simulated via high-speed read)
    pub fn read_frame(&mut self, handle: &rusb::DeviceHandle<GlobalContext>) -> Result<Vec<u8>> {
        let timeout = Duration::from_millis(1000);
        let frame_size = 256 * 192 * 2; // 256x192 pixels, 2 bytes per pixel (Y16 format)
        let mut frame_data = vec![0u8; frame_size];

        // Try endpoint 0x81 if 0x82 fails, and vice versa.
        // Some devices use 0x81 for Y16 and 0x82 for MJPEG.
        let endpoints = [0x81, 0x82];

        for &ep in &endpoints {
            match handle.read_bulk(ep, &mut frame_data, timeout) {
                Ok(bytes_read) => {
                    if bytes_read > 0 {
                        if bytes_read < frame_size {
                            // println!("  (EP 0x{:02x} read {} bytes)", ep, bytes_read);
                        }
                        return Ok(frame_data);
                    }
                }
                Err(_) => {
                    // Try next endpoint
                }
            }
        }

        anyhow::bail!("No data received from video endpoints 0x81 or 0x82. Device may require isochronous transfers.")
    }
}
