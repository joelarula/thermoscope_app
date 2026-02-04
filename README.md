# Thermoscope Rust App

A Rust-based desktop application to stream thermal video from HTI/InfiRay thermal cameras (like XTherm).

## Prerequisites
1.  **Rust**: Install from [rustup.rs](https://rustup.rs/).
2.  **USB Driver (Windows)**:
    *   The `rusb` library requires a generic USB driver to access the device.
    *   Download **Zadig** (https://zadig.akeo.ie/).
    *   Plug in your Thermal Camera.
    *   Open Zadig -> Options -> List All Devices.
    *   Select your camera (it might appear as "Vendor..." or "HTI...").
    *   Replace the driver with **WinUSB**.
    *   *Warning*: This will make the device inaccessible to the original Windows drivers if any existed. (You can rollback via Device Manager).

## Running
1.  Open terminal in `thermoscope_app`.
2.  Run the app:
    ```bash
    cargo run
    ```

## Troubleshooting
### "OS Error 32" / File Locking Issues
If you see errors like `The process cannot access the file because it is being used by another process`, it is likely due to Antivirus or cloud sync locking the build artifacts.
**Fix:** Move the build directory to a temporary folder:
```powershell
$env:CARGO_TARGET_DIR="C:\Users\joela\AppData\Local\Temp\cargo_target"
cargo run
```
