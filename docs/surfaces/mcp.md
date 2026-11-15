# An AI agent's connection (MCP)

What an AI agent sends and is answered, tool by tool. The core's behaviour behind each tool
is its spec rule (`docs/spec/agent-connection.md`), named and never restated. A tool takes
no other argument than its own, and one that arrives all the same is ignored (AGT-040). The
tools not described here keep their input and answer in their spec rules until a task moves
them.

## list_accounts

AGT-040, AGT-102, AGT-076. Answer: one object per account, by name — `name`, `currency`, `country`
(two letters: its own, or the portfolio's, ACC-042), `wrapper` (an identifier, or `null`), `holder` (or `null`) and `cost_basis_method` (`WeightedAverage` or `Fifo`, CBM-010).

## list_lots

AGT-076, CBM-034. Input: `account`, its name, and `asset`, named as a recording names it
(AGT-044). `MissingArgument`: either missing, empty or not text. `AccountNotFound`,
`AccountAmbiguous`, `AssetNotFound`, `AssetAmbiguous`: a name that matches no account or
asset, or several. `DatabaseError`: a read that fails. Answer: `{ "status": "listed",
"account", "asset", "cost_basis_method", "lots" }` — the account's name, the asset's
reference, the method, and the lots as CBM-034 reads them, each with the fields of the Lot
entity (`docs/spec/cost-basis.md`): `acquired_on`, `quantity`, `cost`, `remaining`, `free`.

## describe_rules

AGT-077. Input: none. Answer: `{ "status": "described", "scope", "rules" }` — `scope` says
what the rules are and that no tax is computed; each rule is `{ "rules", "topic",
"statement", "example" }`: the spec rules it states, a few words, the statement, and an
example — `account_country`, `account_currency`, `asset_currency` (the accounts Example and Other are of that country and currency), `management_fees_enabled` when the example needs the account's management fees on, the `calls` in order (each `{ "tool",
"arguments" }`) and `answers`: `{ "at", "value" }`, the figure the last call answers at that
JSON pointer.

## add_asset

AGT-064. Input: `name`, `reference` and `currency`; `kind` or `class`, at least one, read
with case ignored; `isin` and `exchange` (a MIC code, case ignored), each optional.
`MissingArgument`: a name, a reference or a currency missing, empty or not text, or neither
kind nor class. `InvalidArguments`: a kind, a class, an ISIN or an exchange that is not
text, a kind or a class AGT-064 does not allow (`Cash` included), and every bound it sets on
the text. Answer:
`{ "status": "added", "asset": …, "reference_shared": … }`, the asset as `list_assets`
gives it.

## add_account

AGT-068. Input: `name` and `currency`; `bank`, `country` and `wrapper`, each optional.
`MissingArgument`: a name or a currency missing, empty, of spaces only or not text.
`InvalidArguments`: a bank, a country or a wrapper that is not text, and every bound AGT-068
sets on the text. Answer: `{ "status": "created", "account": … }`, the account as
`list_accounts` gives it.

## record_deposit, record_withdrawal

AGT-060. Input: `account`, its name, and `amount`, in the account's currency; `date` and
`note`, each optional; no asset. The refusals are the recording's, among them
`AmountNotPositive`, and `InsufficientCash` for a withdrawal (CSH-031, CSH-080). Answer: as a recording's (AGT-044), its asset `null`.

## record_dividend

AGT-060. Input: `account` and `asset`, named as AGT-044 says, and `amount`, the total net
amount received in the asset's currency; `rate`, the exchange rate to the account's
currency, `date` and `note`, each optional. The refusals are the recording's, among them
`AssetNotHeld` (DIV-011). Answer: as a
recording's (AGT-044). `correct_recording` and `cancel_recording` answer
`DividendBeforeHolding` for the refusals of DIV-040 and DIV-045.
