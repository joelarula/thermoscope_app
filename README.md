# Thermoscope Pro: Pure Rust Thermal Imaging

A high-performance, real-time thermal imaging suite for HTI/InfiRay thermal cameras (T2L, T3, XTherm), built with the safety and speed of Rust.

## 🚀 Key Features

-   **High-Performance Streaming**: 30+ FPS real-time feed powered by a dedicated background processing engine.
-   **Zero-Lag UI**: Thread-decoupled rendering ensures the UI remains butter-smooth even during intensive frame decoding.
-   **Automated DLL Bundling**: The build system automatically manages `libuvc.dll`, ensuring it's always where it needs to be.
-   **Smart Driver Fallback**: Built-in logic to gracefully handle different driver configurations and hardware states.
-   **Minimalist Interface**: Clean, full-window thermal visualization with intelligent auto-scaling.

## 🏗️ Architecture

Thermoscope Pro is built on a modular "Engine" architecture:
-   **Core Library (`src/lib.rs`)**: Encapsulates all hardware logic, USB unlocking, and UVC stream management.
-   **Thermal Engine**: Manages high-priority worker threads for Y16 decoding and color mapping.
-   **Smart Loading**: Uses `libloading` with a smart search strategy to find `libuvc.dll` relative to the executable.

## 🛠️ Setup & Prerequisites

### 1. USB Driver Configuration (Windows)
The application requires direct USB access via the **WinUSB** driver. Use [Zadig](https://zadig.akeo.ie/) to configure your device:
1.  Plug in your thermal camera.
2.  Open Zadig and select **Options > List All Devices**.
3.  Choose your camera (e.g., "HTI Thermal Camera" or "Vendor 0bda Product 5830").
4.  Select **WinUSB** in the target driver box and click **Replace Driver**.

### 2. Runtime Dependency: `libuvc.dll`
This project includes a bundled `libuvc.dll` at the root. The automated build system (`build.rs`) will **automatically copy this DLL** to your target output folder whenever you build or run the project. You do not need to move it manually.

## 🏃 Quick Start

```powershell
# Clone the repository
git clone https://github.com/joelarula/thermoscope_app.git
cd thermoscope_app

# Run the application
cargo run --release
```

> [!TIP]
> **Troubleshooting Build Errors**
> If you encounter "Access Denied" or "OS Error 32" (caused by antivirus/indexing locking files), use a temporary build directory:
> `$env:CARGO_TARGET_DIR="C:\Users\joela\AppData\Local\Temp\cargo_target"; cargo run --release`

## 📖 Technical Details
-   **Frame Protocol**: 256x192 YUYV/Y16 raw capture.
-   **Processing**: Background-thread decoding using the `ThermalEngine` trait.
-   **UI**: Built with `eframe` and `egui` for native-speed GPU rendering.

## License
MIT / Apache 2.0
