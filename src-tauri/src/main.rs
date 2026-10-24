// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Root helper for 🔑 deletes (spawned via osascript): never starts the UI.
    if args.get(1).map(String::as_str) == Some("--helper-delete") {
        std::process::exit(args.get(2).map_or(2, |l| dv_lib::helper_main(l)));
    }
    dv_lib::run()
}
