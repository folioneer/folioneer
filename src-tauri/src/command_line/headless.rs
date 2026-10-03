//! Headless entry of the command line (CLI-030, CLI-031): no window, no event, the same
//! data folder as the application — refused while the window owns it.

use std::path::Path;

use crate::core::{Database, BACKEND};
use crate::shared::infrastructure::app_directories;
use crate::shared::infrastructure::container::AppContainer;
use crate::shared::infrastructure::window_lock::WindowLock;

use super::args::{parse, Command, Invocation, Listed};
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
pub(crate) async fn run_in(data_dir: &Path, command: Command, json: bool) -> Printed {
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
    let command = match command {
        Command::List(listed) => return list_in(data_dir, listed, json).await,
        writing => writing,
    };
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
    let outcome = match command {
        Command::Record(recording) => runner.run(recording, &today).await,
        Command::AddAsset(named) => runner.add_asset(named).await,
        Command::List(listed) => runner.list(listed).await,
    };
    render(&outcome, json)
}

/// CLI-018 / CLI-019 — a list only reads: it takes no lock and opens the portfolio for
/// reading, so it answers while the window is open and never changes the portfolio file.
async fn list_in(data_dir: &Path, listed: Listed, json: bool) -> Printed {
    let database = match Database::open_read_only(data_dir).await {
        Ok(database) => database,
        Err(error) => {
            tracing::error!(target: BACKEND, err = %format!("{error:#}"), "command line: read-only open failed");
            return refused(&unavailable("the portfolio could not be opened"), json);
        }
    };
    let container = AppContainer::for_headless_reads(database.pool.clone());
    let runner = CommandRunner::new(container.account_service, container.asset_service);
    let printed = render(&runner.list(listed).await, json);
    database.pool.close().await;
    printed
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
    use crate::command_line::args::{parse, Invocation};
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

    // CLI-018 — the accounts as a table of what `--account` takes: name and currency, with
    // a header; as JSON with `--json`. A list answers while the window is open (CLI-030).
    #[tokio::test]
    async fn cli_018_lists_the_accounts_even_while_the_window_is_open() {
        let dir = portfolio("list-accounts").await;
        let _window = WindowLock::acquire(&dir)
            .expect("acquire")
            .expect("free lock");

        let listed = run_line(&dir, "account list").await;
        assert_eq!(listed.exit_code, RECORDED);
        assert_eq!(
            listed.stdout.as_deref(),
            Some("NAME   CURRENCY\nPEA    EUR")
        );

        let json = run_line(&dir, "account list --json").await;
        let value: serde_json::Value =
            serde_json::from_str(json.stdout.as_deref().expect("json")).expect("valid json");
        assert_eq!(
            value,
            serde_json::json!({ "status": "listed", "accounts": [{ "name": "PEA", "currency": "EUR" }] })
        );
    }

    // CLI-019 — the assets as a table of what `--asset` takes: name and reference, sorted
    // by name; never a Cash Asset; an archived asset only with `--archived`; every field as
    // JSON.
    #[tokio::test]
    async fn cli_019_lists_the_assets_a_command_can_name() {
        let dir = portfolio("list-assets").await;
        {
            let database = Database::new(dir.clone()).await.expect("database");
            let container = AppContainer::for_headless_writes(database.pool);
            let assets = container
                .asset_service
                .get_non_cash_assets()
                .await
                .expect("assets");
            let world = assets
                .iter()
                .find(|asset| asset.reference == "CW8")
                .expect("CW8");
            container
                .asset_service
                .archive_asset(&world.id)
                .await
                .expect("archived");
            asset(&container, "amundi alpha", "ALP").await;
        }

        let listed = run_line(&dir, "asset list").await;
        assert_eq!(
            listed.stdout.as_deref(),
            Some("NAME                REFERENCE\namundi alpha        ALP\nAmundi Euro Stoxx   C50")
        );

        let with_archived = run_line(&dir, "asset list --archived").await;
        assert_eq!(
            with_archived.stdout.as_deref(),
            Some("NAME                REFERENCE\namundi alpha        ALP\nAmundi Euro Stoxx   C50\nAmundi MSCI World   CW8")
        );

        let json = run_line(&dir, "asset list --archived --json").await;
        let value: serde_json::Value =
            serde_json::from_str(json.stdout.as_deref().expect("json")).expect("valid json");
        assert_eq!(value["status"], "listed");
        assert_eq!(
            value["assets"][2],
            serde_json::json!({
                "name": "Amundi MSCI World",
                "reference": "CW8",
                "class": "ETF",
                "currency": "EUR",
                "isin": null,
                "archived": true
            })
        );
        assert_eq!(value["assets"].as_array().map(Vec::len), Some(3));
    }

    // CLI-018 — a list reads a portfolio the window closed cleanly, and leaves the portfolio
    // file byte for byte as it found it.
    #[tokio::test]
    async fn cli_018_a_list_never_changes_the_portfolio_file() {
        let dir = portfolio("list-closed").await;
        Database::new(dir.clone())
            .await
            .expect("database")
            .pool
            .close()
            .await;
        let portfolio_file = dir.join("portfolio");
        let before = std::fs::read(&portfolio_file).expect("portfolio file");

        let listed = run_line(&dir, "account list").await;

        assert_eq!(
            listed.stdout.as_deref(),
            Some("NAME   CURRENCY\nPEA    EUR")
        );
        assert_eq!(
            std::fs::read(&portfolio_file).expect("portfolio file"),
            before
        );
    }

    // CLI-026 — an asset is added with what the command left out decided by the core (its
    // class's risk level, the system category), can then be named by a command, and is
    // refused while the window is open.
    #[tokio::test]
    async fn cli_026_adds_an_asset_with_the_core_defaults() {
        let dir = portfolio("asset-add").await;
        let line = "asset add --name ASML --reference asml --class stocks --currency EUR";
        {
            let _window = WindowLock::acquire(&dir)
                .expect("acquire")
                .expect("free lock");
            let refused = run_line(&dir, line).await;
            assert_eq!(
                refused.stderr.as_deref(),
                Some("Refused: close Folioneer first — it is open and owns the portfolio")
            );
        }

        let added = run_line(&dir, line).await;
        assert_eq!(added.exit_code, RECORDED);
        assert_eq!(
            added.stdout.as_deref(),
            Some("Recorded: added ASML (ASML) — Stocks, EUR")
        );

        let database = Database::new(dir.clone()).await.expect("database");
        let container = AppContainer::for_headless_writes(database.pool);
        let assets = container
            .asset_service
            .get_non_cash_assets()
            .await
            .expect("assets");
        let asml = assets
            .iter()
            .find(|asset| asset.reference == "ASML")
            .expect("ASML");
        assert_eq!(asml.risk_level, AssetClass::Stocks.default_risk());
        assert_eq!(asml.category.id, "default-uncategorized");
        assert!(!asml.is_archived);

        let opened = run_line(
            &dir,
            "holding open --account PEA --asset ASML --quantity 1 --total-cost 600",
        )
        .await;
        assert_eq!(opened.exit_code, RECORDED);
    }

    // CLI-026 — every optional figure is taken as typed: the category by its name, the
    // exchange by its code, both with case ignored; the result as JSON.
    #[tokio::test]
    async fn cli_026_takes_the_optional_figures_as_typed() {
        let dir = portfolio("asset-add-options").await;
        {
            let database = Database::new(dir.clone()).await.expect("database");
            let container = AppContainer::for_headless_writes(database.pool);
            container
                .asset_service
                .create_category("Tech")
                .await
                .expect("category");
        }

        let added = run_line(
            &dir,
            "asset add --name Apple --reference AAPL --class Stocks --currency USD --isin US0378331005 --exchange xnas --risk 5 --category tech --json",
        )
        .await;

        assert_eq!(added.exit_code, RECORDED);
        let value: serde_json::Value =
            serde_json::from_str(added.stdout.as_deref().expect("json")).expect("valid json");
        assert_eq!(
            value,
            serde_json::json!({
                "status": "recorded",
                "asset": {
                    "name": "Apple",
                    "reference": "AAPL",
                    "class": "Stocks",
                    "currency": "USD",
                    "isin": "US0378331005",
                    "archived": false
                }
            })
        );
        let database = Database::new(dir.clone()).await.expect("database");
        let container = AppContainer::for_headless_writes(database.pool);
        let assets = container
            .asset_service
            .get_non_cash_assets()
            .await
            .expect("assets");
        let apple = assets
            .iter()
            .find(|asset| asset.reference == "AAPL")
            .expect("AAPL");
        assert_eq!(apple.risk_level, 5);
        assert_eq!(apple.category.name, "Tech");
        assert_eq!(
            apple
                .exchange
                .as_ref()
                .map(|exchange| exchange.code.as_str()),
            Some("XNAS")
        );
    }

    // AST-009 / CLI-026 — a reference another asset has is added and said: one ticker,
    // several markets. A command then names that asset by its name.
    #[tokio::test]
    async fn cli_026_a_shared_reference_is_recorded_with_a_warning() {
        let dir = portfolio("asset-add-shared").await;

        let added = run_line(
            &dir,
            "asset add --name World_USD --reference cw8 --class ETF --currency USD",
        )
        .await;
        assert_eq!(added.exit_code, RECORDED);
        assert_eq!(
            added.stdout.as_deref(),
            Some("Recorded: added World_USD (CW8) — ETF, USD")
        );
        assert_eq!(
            added.stderr.as_deref(),
            Some("Warning: another asset has the reference CW8; name this one by its name in --asset.")
        );

        let json = run_line(
            &dir,
            "asset add --name World_GBP --reference CW8 --class ETF --currency GBP --json",
        )
        .await;
        let value: serde_json::Value =
            serde_json::from_str(json.stdout.as_deref().expect("json")).expect("valid json");
        assert_eq!(value["warning"], "ReferenceShared");

        let by_reference = run_line(
            &dir,
            "holding open --account PEA --asset CW8 --quantity 1 --total-cost 1 --json",
        )
        .await;
        let refusal: serde_json::Value =
            serde_json::from_str(by_reference.stdout.as_deref().expect("json")).expect("json");
        assert_eq!(refusal["code"], "AssetAmbiguous");
        let by_name = run_line(
            &dir,
            "holding open --account PEA --asset World_USD --quantity 1 --total-cost 1",
        )
        .await;
        assert_eq!(by_name.exit_code, RECORDED);
    }

    // CLI-026 / CLI-012 — an asset whose category does
    // not exist, or that the window's rules reject, is refused with its code and not added.
    #[tokio::test]
    async fn cli_026_refuses_what_the_core_refuses() {
        let dir = portfolio("asset-add-refused").await;
        let code = |printed: Printed| -> String {
            assert_eq!(printed.exit_code, REFUSED);
            let value: serde_json::Value =
                serde_json::from_str(printed.stdout.as_deref().expect("json")).expect("json");
            value["code"].as_str().expect("code").to_string()
        };
        let add = |options: &str| format!("asset add {options} --json");

        for (options, expected) in [
            (
                "--name New --reference NEW --class ETF --currency EUR --category Nope",
                "CategoryNotFound",
            ),
            (
                "--name New --reference NEW --class ETF --currency EURO",
                "InvalidCurrency",
            ),
            (
                "--name New --reference NEW --class ETF --currency EUR --isin 123",
                "InvalidIsinFormat",
            ),
            (
                "--name New --reference NEW --class ETF --currency EUR --exchange XXXX",
                "InvalidExchange",
            ),
            (
                "--name New --reference NEW --class ETF --currency EUR --risk 9",
                "InvalidRiskLevel",
            ),
        ] {
            assert_eq!(
                code(run_line(&dir, &add(options)).await),
                expected,
                "{options}"
            );
        }

        let listed = run_line(&dir, "asset list").await;
        assert_eq!(
            listed.stdout.as_deref(),
            Some("NAME                REFERENCE\nAmundi Euro Stoxx   C50\nAmundi MSCI World   CW8")
        );
    }

    // CLI-032 — a list refuses where no portfolio exists, and creates none.
    #[tokio::test]
    async fn cli_032_a_list_refuses_where_there_is_no_portfolio() {
        let dir = fresh_dir("list-nothing");

        let refused = run_line(&dir, "asset list").await;

        assert_eq!(refused.exit_code, REFUSED);
        assert_eq!(
            refused.stderr.as_deref(),
            Some("Refused: no portfolio for this user on this computer")
        );
        assert!(!Database::exists_in(&dir));
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
