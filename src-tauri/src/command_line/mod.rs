//! The command line (CLI spec, #044): an interface beside the window — like the Tauri shell,
//! it calls the use cases; it records opening balances, purchases and sales from a terminal,
//! without a window, through the same rules as the window.

mod args;
pub mod headless;
mod orchestrator;
mod output;
