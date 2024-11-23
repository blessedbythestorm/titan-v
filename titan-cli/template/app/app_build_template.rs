use std::fs::{self, File};
use std::path::Path;
use std::process::Command;

fn main() {
    // Use a trigger file to ensure the LSP build happens every time
    let trigger_file = Path::new("tools/titan-lsp/.lsp_build_trigger");

    // Always create or touch the trigger file to ensure rerun-if-changed gets triggered
    File::create(trigger_file).expect("Failed to create trigger file for LSP build");

    // Ensure the build.rs script always reruns if the trigger file is touched
    println!("cargo:rerun-if-changed=tools/titan-lsp/.lsp_build_trigger");

    // Now, check if we are in a LSP build process to prevent an infinite loop
    if std::env::var("CARGO_BUILDING_LSP").is_ok() {
        println!("Detected LSP build, skipping post-build LSP compile to avoid loop.");
        return;
    }

    // Path to the LSP project
    let lsp_path = "tools/titan-lsp";

    // Build the LSP after the app in release mode
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .env("CARGO_BUILDING_LSP", "1") // Set this to prevent loops
        .current_dir(lsp_path)
        .status()
        .expect("Failed to build LSP");

    if !status.success() {
        panic!("LSP build failed.");
    }

    // Optionally clean up the trigger file or leave it as a permanent file
    println!("LSP build completed successfully.");
}
