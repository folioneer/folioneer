// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // CLI-010 — a command (`folioneer holding …`) runs without a window and exits; it is
    // decided first, so no argument of a command can start anything else.
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().is_some_and(|first| first == "holding") {
        std::process::exit(folioneer_lib::run_command_line(&arguments));
    }
    // SPF-020 — the OS-scheduled invocation runs the daily download invisibly
    // and exits without ever creating a window.
    if arguments
        .iter()
        .any(|argument| argument == "--scheduled-fetch")
    {
        std::process::exit(folioneer_lib::run_scheduled_fetch_headless());
    }
    folioneer_lib::run()
}
