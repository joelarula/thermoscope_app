use libloading::{Library, Symbol};
use std::ffi::{c_char, c_void};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

type UvcInitFn = unsafe extern "C" fn(ctx: *mut *mut c_void, usb_ctx: *mut c_void) -> i32;
type UvcFindDeviceFn = unsafe extern "C" fn(
    ctx: *mut c_void,
    dev: *mut *mut c_void,
    vid: i32,
    pid: i32,
    sn: *const i8,
) -> i32;
type UvcOpenFn = unsafe extern "C" fn(dev: *mut c_void, devh: *mut *mut c_void) -> i32;
type UvcCloseFn = unsafe extern "C" fn(devh: *mut c_void);
type UvcExitFn = unsafe extern "C" fn(ctx: *mut c_void);
type UvcGetStreamCtrlFormatSizeFn = unsafe extern "C" fn(
    devh: *mut c_void,
    ctrl: *mut UvcStreamCtrl,
    format: u32,
    width: i32,
    height: i32,
    fps: i32,
) -> i32;
type UvcStartStreamingFn = unsafe extern "C" fn(
    devh: *mut c_void,
    ctrl: *const UvcStreamCtrl,
    cb: UvcFrameCallbackFn,
    user_ptr: *mut c_void,
    flags: u8,
) -> i32;
type UvcStopStreamingFn = unsafe extern "C" fn(devh: *mut c_void);
type UvcFrameCallbackFn = unsafe extern "C" fn(frame: *mut UvcFrame, user_ptr: *mut c_void);

#[repr(C)]
#[derive(Debug, Default)]
pub struct UvcStreamCtrl {
    pub hint: u16,
    pub format_index: u8,
    pub frame_index: u8,
    pub dw_frame_interval: u32,
    pub w_key_frame_rate: u16,
    pub w_p_frame_rate: u16,
    pub w_comp_quality: u16,
    pub w_comp_window_size: u16,
    pub w_delay: u16,
    pub dw_max_video_frame_size: u32,
    pub dw_max_payload_transfer_size: u32,
    pub dw_clock_frequency: u32,
    pub frame_format: u8,
    pub b_interface_number: u8,
}

#[repr(C)]
pub struct UvcFrame {
    pub data: *mut u8,
    pub data_bytes: usize,
    pub width: u32,
    pub height: u32,
    pub frame_format: u32,
    pub step: usize,
    pub sequence: u32,
    // There are more fields, but we only need these for now
}

pub struct UvcAdapter {
    lib: Library,
    ctx: *mut c_void,
    devh: *mut c_void,
}

impl UvcAdapter {
    pub fn new(dll_path: &str) -> anyhow::Result<Self> {
        let lib = unsafe { Library::new(dll_path)? };
        let mut ctx: *mut c_void = std::ptr::null_mut();

        unsafe {
            let uvc_init: Symbol<UvcInitFn> = lib.get(b"uvc_init")?;
            let res = uvc_init(&mut ctx, std::ptr::null_mut());
            if res < 0 {
                return Err(anyhow::anyhow!("uvc_init failed: {}", res));
            }
        }

        Ok(Self {
            lib,
            ctx,
            devh: std::ptr::null_mut(),
        })
    }

    pub fn open_device(&mut self, vid: i32, pid: i32) -> anyhow::Result<()> {
        unsafe {
            let uvc_find_device: Symbol<UvcFindDeviceFn> = self.lib.get(b"uvc_find_device")?;
            let uvc_open: Symbol<UvcOpenFn> = self.lib.get(b"uvc_open")?;

            let mut dev: *mut c_void = std::ptr::null_mut();
            let res = uvc_find_device(self.ctx, &mut dev, vid, pid, std::ptr::null());
            if res < 0 {
                return Err(anyhow::anyhow!("Thermal camera not found: {}", res));
            }

            let res = uvc_open(dev, &mut self.devh);
            if res < 0 {
                return Err(anyhow::anyhow!("uvc_open failed: {}", res));
            }
        }
        Ok(())
    }

    pub fn start_streaming(&self, tx: Sender<Vec<u8>>) -> anyhow::Result<()> {
        unsafe {
            let uvc_get_stream_ctrl: Symbol<UvcGetStreamCtrlFormatSizeFn> =
                self.lib.get(b"uvc_get_stream_ctrl_format_size")?;
            let uvc_start_streaming: Symbol<UvcStartStreamingFn> =
                self.lib.get(b"uvc_start_streaming")?;

            let mut ctrl = UvcStreamCtrl::default();
            // Y16 is often format 4 in libuvc for these cameras, or we use UVC_FRAME_FORMAT_Y16
            // Based on generic libuvc: enum uvc_frame_format { ... UVC_FRAME_FORMAT_Y16 = 4 }
            let res = uvc_get_stream_ctrl(self.devh, &mut ctrl, 4, 256, 192, 25);
            if res < 0 {
                // Fallback to format 1 (YUY2) if Y16 fails
                uvc_get_stream_ctrl(self.devh, &mut ctrl, 1, 256, 192, 25);
            }

            let box_tx = Box::new(tx);
            let user_ptr = Box::into_raw(box_tx) as *mut c_void;

            let res = uvc_start_streaming(self.devh, &ctrl, frame_callback, user_ptr, 0);
            if res < 0 {
                return Err(anyhow::anyhow!("uvc_start_streaming failed: {}", res));
            }
        }
        Ok(())
    }
}

unsafe extern "C" fn frame_callback(frame: *mut UvcFrame, user_ptr: *mut c_void) {
    if frame.is_null() || user_ptr.is_null() {
        return;
    }

    let frame = unsafe { &*frame };
    let tx = unsafe { &*(user_ptr as *const Sender<Vec<u8>>) };

    if frame.data.is_null() {
        return;
    }

    let data = unsafe { std::slice::from_raw_parts(frame.data, frame.data_bytes) };
    let _ = tx.send(data.to_vec());
}

impl Drop for UvcAdapter {
    fn drop(&mut self) {
        unsafe {
            if !self.devh.is_null() {
                if let Ok(uvc_stop) = self.lib.get::<UvcStopStreamingFn>(b"uvc_stop_streaming") {
                    uvc_stop(self.devh);
                }
                if let Ok(uvc_close) = self.lib.get::<UvcCloseFn>(b"uvc_close") {
                    uvc_close(self.devh);
                }
            }
            if !self.ctx.is_null() {
                if let Ok(uvc_exit) = self.lib.get::<UvcExitFn>(b"uvc_exit") {
                    uvc_exit(self.ctx);
                }
            }
        }
    }
}
