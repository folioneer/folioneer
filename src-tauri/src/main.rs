// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // SPF-020 — the OS-scheduled invocation runs the daily download invisibly
    // and exits without ever creating a window.
    if std::env::args().any(|argument| argument == "--scheduled-fetch") {
        std::process::exit(folioneer_lib::run_scheduled_fetch_headless());
    }
    folioneer_lib::run()
}
