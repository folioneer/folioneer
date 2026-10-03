//! Headless entry of the command line (CLI-030, CLI-031): no window, no event, the same
//! data folder as the application — refused while the window owns it.

use std::path::Path;

use crate::core::{Database, BACKEND};
use crate::shared::infrastructure::app_directories;
use crate::shared::infrastructure::container::AppContainer;
use crate::shared::infrastructure::window_lock::WindowLock;

use super::args::{parse, Invocation};
use super::help::help;
use super::orchestrator::{CommandRunner, Refusal};
use super::output::{refused, render, Printed, RECORDED, WRONG_USAGE};

/// Reads the arguments after the program name, runs the command against the data folder,
/// and returns what to print with the exit code. `program` is the name the user typed it
/// under, for the help it prints (CLI-016).
pub async fn run(program: &str, args: &[String]) -> Printed {
    let (command, json) = match parse(args) {
        Ok(Invocation::Run { command, json }) => (command, json),
        Ok(Invocation::Help(topic)) => {
            return Printed {
                stdout: Some(help(program, topic)),
                stderr: None,
                exit_code: RECORDED,
            }
        }
        // CLI-024 — the mistake and where to read how, never the whole help.
        Err(error) => {
            return Printed {
                stdout: None,
                stderr: Some(format!(
                    "error: {}\nTry '{}'.",
                    error.message,
                    error.topic.invocation(program)
                )),
                exit_code: WRONG_USAGE,
            }
        }
    };
    let Some(data_dir) = app_directories::resolve_local_data_dir() else {
        return refused(&unavailable("no data folder for this user"), json);
    };
    run_in(&data_dir, command, json).await
}

/// Runs a parsed command against the portfolio in `data_dir`.
pub(crate) async fn run_in(data_dir: &Path, command: super::args::Command, json: bool) -> Printed {
    // CLI-032 — a command works on an existing portfolio; it never creates one.
    if !Database::exists_in(data_dir) {
        return refused(
            &Refusal {
                code: "NoPortfolio".to_string(),
                message: "no portfolio for this user on this computer".to_string(),
            },
            json,
        );
    }
    // CLI-030 — held until the command has written, so a window cannot start meanwhile and
    // two commands never write at once.
    let _lock = match WindowLock::acquire(data_dir) {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            return refused(
                &Refusal {
                    code: "WindowOpen".to_string(),
                    message: "close Folioneer first — it is open and owns the portfolio"
                        .to_string(),
                },
                json,
            )
        }
        Err(error) => {
            tracing::error!(target: BACKEND, err = %error, "command line: window lock unavailable");
            return refused(&unavailable("the data folder cannot be locked"), json);
        }
    };
    let database = match Database::new(data_dir.to_path_buf()).await {
        Ok(database) => database,
        Err(error) => {
            tracing::error!(target: BACKEND, err = %format!("{error:#}"), "command line: database initialization failed");
            return refused(&unavailable("the portfolio could not be opened"), json);
        }
    };
    // No event bus: nothing listens (CLI-031). Writes are recorded for sync (SYN-020).
    let container = AppContainer::for_headless_writes(database.pool);
    let runner = CommandRunner::new(container.account_service, container.asset_service);
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    render(&runner.run(command, &today).await, json)
}

fn unavailable(reason: &str) -> Refusal {
    Refusal {
        code: "DatabaseError".to_string(),
        message: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_line::args::{parse, Command, Invocation};
    use crate::command_line::output::{REFUSED, WRONG_USAGE};
    use crate::context::account::UpdateFrequency;
    use crate::context::asset::{AssetClass, CreateAssetDTO};
    use crate::shared::infrastructure::window_lock::WindowLock;
    use crate::use_cases::account_creation::AccountCreationUseCase;
    use std::path::PathBuf;
    use std::sync::Arc;

    const M: i64 = 1_000_000;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("folioneer-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn command(line: &str) -> (Command, bool) {
        let args: Vec<String> = line.split_whitespace().map(str::to_string).collect();
        match parse(&args).expect("valid command") {
            Invocation::Run { command, json } => (command, json),
            Invocation::Help(_) => panic!("help"),
        }
    }

    async fn run_line(dir: &Path, line: &str) -> Printed {
        let (command, json) = command(line);
        run_in(dir, command, json).await
    }

    async fn asset(container: &AppContainer, name: &str, reference: &str) {
        container
            .asset_service
            .create_asset(CreateAssetDTO {
                name: name.to_string(),
                reference: reference.to_string(),
                isin: None,
                class: AssetClass::ETF,
                currency: "EUR".to_string(),
                risk_level: 3,
                category_id: "default-uncategorized".to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .expect("asset");
    }

    /// A portfolio with the account "PEA" holding 5,000 EUR of cash, and two assets:
    /// "Amundi MSCI World" (CW8) and "Amundi Euro Stoxx" (C50).
    async fn portfolio(name: &str) -> PathBuf {
        let dir = fresh_dir(name);
        let database = Database::new(dir.clone()).await.expect("database");
        let container = AppContainer::for_headless_writes(database.pool);
        let accounts: Arc<dyn crate::context::account::AccountServiceContract> =
            Arc::clone(&container.account_service) as _;
        let assets: Arc<dyn crate::context::asset::AssetServiceContract> =
            Arc::clone(&container.asset_service) as _;
        let account = AccountCreationUseCase::new(accounts, assets)
            .create(
                "PEA".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .expect("account");
        container
            .account_service
            .record_deposit(&account.id, "2026-01-02".to_string(), 5_000 * M, None)
            .await
            .expect("deposit");
        asset(&container, "Amundi MSCI World", "CW8").await;
        asset(&container, "Amundi Euro Stoxx", "C50").await;
        dir
    }

    // CLI-010 / CLI-012 / CLI-020 / CLI-022 — an opening balance, a purchase and a sale are
    // recorded through the window's rules; a sale above the holding is refused like the Sell
    // dialog refuses it, and nothing is written.
    #[tokio::test]
    async fn cli_012_records_through_the_window_rules() {
        let dir = portfolio("records").await;

        let opened = run_line(&dir, "holding open --account pea --asset cw8 --quantity 10 --total-cost 4950 --date 2026-02-01").await;
        assert_eq!(opened.exit_code, RECORDED);
        assert_eq!(
            opened.stdout.as_deref(),
            Some("Recorded: opened 10 CW8 in PEA on 2026-02-01 — 4950.00 EUR")
        );

        let bought = run_line(&dir, "holding buy --account PEA --asset CW8 --quantity 2 --price 495.10 --fees 1.99 --date 2026-03-01").await;
        assert_eq!(
            bought.stdout.as_deref(),
            Some("Recorded: bought 2 CW8 in PEA on 2026-03-01 — 992.19 EUR")
        );

        let oversold = run_line(
            &dir,
            "holding sell --account PEA --asset CW8 --quantity 20 --price 500 --date 2026-04-01",
        )
        .await;
        assert_eq!(oversold.exit_code, REFUSED);
        assert_eq!(
            oversold.stderr.as_deref(),
            Some("Refused: only 12 held, not 20")
        );

        let sold = run_line(&dir, "holding sell --account PEA --asset CW8 --quantity 2 --total 990 --date 2026-04-01 --json").await;
        assert_eq!(sold.exit_code, RECORDED);
        let value: serde_json::Value =
            serde_json::from_str(sold.stdout.as_deref().expect("json")).expect("valid json");
        assert_eq!(value["status"], "recorded");
        assert_eq!(value["transaction"]["transaction_type"], "Sell");
        assert_eq!(value["transaction"]["total_amount"], 990 * M);
    }

    // CLI-011 — an unknown account, an unknown asset and an ambiguous name are refused,
    // naming only what was typed.
    #[tokio::test]
    async fn cli_011_refuses_what_it_cannot_find_naming_only_what_was_typed() {
        let dir = portfolio("names").await;

        let no_account = run_line(
            &dir,
            "holding open --account PEA2 --asset CW8 --quantity 1 --total-cost 1",
        )
        .await;
        assert_eq!(no_account.exit_code, REFUSED);
        assert_eq!(
            no_account.stderr.as_deref(),
            Some("Refused: no account named \"PEA2\"")
        );

        let no_asset = run_line(
            &dir,
            "holding open --account PEA --asset CW9 --quantity 1 --total-cost 1 --json",
        )
        .await;
        let value: serde_json::Value =
            serde_json::from_str(no_asset.stdout.as_deref().expect("json")).expect("valid json");
        assert_eq!(value["code"], "AssetNotFound");
        assert_eq!(
            value["message"],
            "no asset matches \"CW9\" by name or reference"
        );
        assert_eq!(value.as_object().map(|object| object.len()), Some(3));

        let database = Database::new(dir.clone()).await.expect("database");
        let container = AppContainer::for_headless_writes(database.pool);
        asset(&container, "Twin", "TW1").await;
        asset(&container, "Twin", "TW2").await;
        let ambiguous = run_line(
            &dir,
            "holding open --account PEA --asset twin --quantity 1 --total-cost 1",
        )
        .await;
        assert_eq!(
            ambiguous.stderr.as_deref(),
            Some("Refused: \"twin\" matches more than one asset — use its reference")
        );
        let by_reference = run_line(
            &dir,
            "holding open --account PEA --asset TW2 --quantity 1 --total-cost 1",
        )
        .await;
        assert_eq!(by_reference.exit_code, RECORDED);
    }

    // CLI-030 — while a window holds the lock, a command refuses and writes nothing.
    #[tokio::test]
    async fn cli_030_refuses_while_the_window_is_open() {
        let dir = portfolio("window").await;
        let window = WindowLock::acquire(&dir)
            .expect("acquire")
            .expect("free lock");

        let refused = run_line(
            &dir,
            "holding open --account PEA --asset CW8 --quantity 1 --total-cost 1 --json",
        )
        .await;
        assert_eq!(refused.exit_code, REFUSED);
        let value: serde_json::Value =
            serde_json::from_str(refused.stdout.as_deref().expect("json")).expect("valid json");
        assert_eq!(value["code"], "WindowOpen");

        drop(window);
        let recorded = run_line(
            &dir,
            "holding open --account PEA --asset CW8 --quantity 1 --total-cost 1",
        )
        .await;
        assert_eq!(recorded.exit_code, RECORDED);
    }

    // CLI-032 — a command never creates a portfolio: with none yet, it refuses.
    #[tokio::test]
    async fn cli_032_refuses_where_there_is_no_portfolio() {
        let dir = fresh_dir("empty");
        let refused = run_line(
            &dir,
            "holding open --account PEA --asset CW8 --quantity 1 --total-cost 1 --json",
        )
        .await;
        assert_eq!(refused.exit_code, REFUSED);
        let value: serde_json::Value =
            serde_json::from_str(refused.stdout.as_deref().expect("json")).expect("valid json");
        assert_eq!(value["code"], "NoPortfolio");
        assert!(!Database::exists_in(&dir));
    }

    // CLI-011 — the account's cash is never an asset a command buys or sells.
    #[tokio::test]
    async fn cli_011_never_matches_the_cash_asset() {
        let dir = portfolio("cash").await;
        let refused = run_line(
            &dir,
            "holding buy --account PEA --asset EUR --quantity 1 --price 1",
        )
        .await;
        assert_eq!(refused.exit_code, REFUSED);
        assert_eq!(
            refused.stderr.as_deref(),
            Some("Refused: no asset matches \"EUR\" by name or reference")
        );
    }

    // CLI-016 / CLI-024 / CLI-022 — help prints its page on standard output and exits 0; a
    // wrong command line exits 2 with the mistake and the help to run, not the help itself.
    #[tokio::test]
    async fn cli_024_help_and_wrong_usage() {
        let line =
            |text: &str| -> Vec<String> { text.split_whitespace().map(str::to_string).collect() };

        let help = run("folioneer", &line("holding buy --help")).await;
        assert_eq!(help.exit_code, RECORDED);
        assert!(help.stderr.is_none());
        assert!(help
            .stdout
            .as_deref()
            .is_some_and(|text| text.contains("Usage: folioneer holding buy ")));

        let wrong = run("folioneer-cli", &line("holding buy --account PEA --ASML")).await;
        assert_eq!(wrong.exit_code, WRONG_USAGE);
        assert!(wrong.stdout.is_none());
        assert_eq!(
            wrong.stderr.as_deref(),
            Some("error: unknown option \"--ASML\"\nTry 'folioneer-cli holding buy --help'.")
        );

        let mistyped = run("folioneer", &line("holding buuy")).await;
        assert_eq!(
            mistyped.stderr.as_deref(),
            Some(
                "error: unknown command \"holding buuy\". Did you mean \"holding buy\"?\nTry 'folioneer --help'."
            )
        );
    }
}
