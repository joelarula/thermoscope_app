use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=libuvc.dll");
    println!("cargo:rerun-if-changed=build.rs");

    let dll_name = "libuvc.dll";
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = PathBuf::from(&manifest_dir);
    let dll_src = manifest_path.join(dll_name);

    if !dll_src.exists() {
        let project_root = manifest_path.parent().unwrap();
        let alt_src = project_root.join(dll_name);
        if alt_src.exists() {
            copy_dll(&alt_src, dll_name);
        } else {
            println!(
                "cargo:warning=libuvc.dll not found in {} or {}",
                manifest_dir,
                project_root.display()
            );
        }
    } else {
        copy_dll(&dll_src, dll_name);
    }
}

fn copy_dll(src: &std::path::Path, name: &str) {
    if let Ok(out_dir) = env::var("OUT_DIR") {
        let out_path = PathBuf::from(out_dir);
        let mut target_dir = out_path;
        target_dir.pop();
        target_dir.pop();
        target_dir.pop();
        target_dir.pop();

        let dest = target_dir.join(name);
        println!(
            "cargo:warning=Copying {} to {}",
            src.display(),
            dest.display()
        );
        if let Err(e) = fs::copy(src, dest) {
            println!("cargo:warning=Failed to copy DLL: {}", e);
        }
    }
}
