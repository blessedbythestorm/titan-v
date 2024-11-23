use std::env;
use std::process::Command;

fn main() {
    // Only trigger a build for titan-viewer if this is not already a workspace build.
    if env::var("CARGO_WORKSPACE_BUILD").is_ok() {
        println!("Skipping build of titan-viewer to avoid loop.");
        return;
    }

    // Monitor changes in titan-viewer to rebuild titan-lsp if necessary
    println!("cargo:rerun-if-changed=../titan-viewer/src/");

    // Trigger titan-viewer build (if needed, but generally avoid to prevent loop)
}
