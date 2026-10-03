//! `folioneer-cli`: the command line as a console program (CLI-040). On Windows the main
//! program is built without a console, so PowerShell would show none of its output and not
//! wait for its exit code; this program is a console one and runs the same commands.

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(folioneer_lib::run_command_line("folioneer-cli", &arguments));
}
