# The command line

What each command takes, prints and exits with, command by command. The core's behaviour
behind each command is its spec rule (`docs/spec/command-line.md` and the rules it names),
named and never restated. The commands not described here keep their arguments in their spec
rules until a task moves them.

## holding dividend

DIV-011, CLI-011, CLI-012, CLI-014. `folioneer holding dividend` takes `--account` and
`--asset`, and `--amount` — the total net amount received, in the asset's currency — and
optionally `--rate`, the exchange rate from the asset's currency to the account's, `--date`, `--note`
and `--json`; it takes no quantity. A dividend of an asset the account did not hold on its
date or before it exits as a refusal, its code `AssetNotHeld`.
