# An AI agent's connection (MCP)

What an AI agent sends and is answered, tool by tool. The core's behaviour behind each tool
is its spec rule (`docs/spec/agent-connection.md`), named and never restated. A tool takes
no other argument than its own, and one that arrives all the same is ignored (AGT-040). The
tools not described here keep their input and answer in their spec rules until a task moves
them.

## list_accounts

AGT-040, AGT-102, AGT-076. Answer: one object per account, by name — `name`, `currency`, `country`
(two letters: its own, or the portfolio's, ACC-042), `wrapper` (an identifier, or `null`), `holder` (or `null`), `cost_basis_method` (`WeightedAverage` or `Fifo`, CBM-010) and `management_fees` (`true` or `false`, FEE-075) — the name `add_account` takes.

## list_assets

AGT-040. Input: `archived`, `true` to list the archived assets too, optional. Answer: one object per asset, by name — `name`, `reference`, `kind`, `class`, `currency`, `isin` (or `null`), `exchange` (a MIC code, or `null`), `archived` and `interest_bearing` (`true` or `false`, AST-024) — the name `add_asset` takes.

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
example — `account_country`, `account_currency`, `asset_currency` (the accounts Example and Other are of that country and currency), `management_fees: true` when the example needs the account's management fees on, left out otherwise — the argument `add_account` takes — the `calls` in order (each `{ "tool",
"arguments" }`) and `answers`: `{ "at", "value" }`, the figure the last call answers at that
JSON pointer.

## add_asset

AGT-064. Input: `name`, `reference` and `currency`; `kind` or `class`, at least one, read
with case ignored; `isin` and `exchange` (a MIC code, case ignored), each optional;
`interest_bearing`, `true` or `false`, optional, `false` when left out.
`MissingArgument`: a name, a reference or a currency missing, empty or not text, or neither
kind nor class. `InvalidArguments`: a kind, a class, an ISIN or an exchange that is not
text, a kind or a class AGT-064 does not allow (`Cash` included), and every bound it sets on
the text, an `interest_bearing` that is not `true` or `false`, and `true` for a crypto
asset (AGT-064A). Answer:
`{ "status": "added", "asset": …, "reference_shared": … }`, the asset as `list_assets`
gives it.

## add_account

AGT-068. Input: `name` and `currency`; `bank`, `country` and `wrapper`, each optional;
`management_fees`, `true` or `false`, optional, `false` when left out.
`MissingArgument`: a name or a currency missing, empty, of spaces only or not text.
`InvalidArguments`: a bank, a country or a wrapper that is not text, every bound AGT-068
sets on the text, and a `management_fees` that is not `true` or `false`. Answer: `{ "status": "created", "account": … }`, the account as
`list_accounts` gives it.

## record_deposit, record_withdrawal

AGT-060. Input: `account`, its name, and `amount`, in the account's currency; `date` and
`note`, each optional; no asset. The refusals are the recording's, among them
`AmountNotPositive`, and `InsufficientCash` for a withdrawal (CSH-031, CSH-080). Answer: as a recording's (AGT-044), its asset `null`.

## record_dividend

AGT-060. Input: `account` and `asset`, named as AGT-044 says, and `amount`, the total net
amount received in the asset's currency; `rate`, the exchange rate to the account's
currency, `date` and `note`, each optional. The refusals are the recording's, among them
`AssetNotHeld` (DIV-011), and `DividendRateNotOne` (DIV-022A) for a rate other than 1 while the asset and the account share a currency. Answer: as a
recording's (AGT-044). `correct_recording` and `cancel_recording` answer
`DividendBeforeHolding` for the refusals of DIV-040 and DIV-045.

## request_transfer_replacement

AGT-090, AGT-091. Title: "Ask to replace four transactions you typed with a transfer".
Input: `transactions`, the ids of four transactions as `list_transactions` gives them
(AGT-070), and `rate`, as `record_transfer` reads it (AGT-065A), optional.
`MissingArgument`: `transactions` missing or not a list of texts. `InvalidArguments`: other
than four ids, one of them twice, or an id past AGT-048A's bound. `TransactionNotFound`,
`NotTypedByOwner`: as `request_removal` (AGT-080). `NotATransfer`: the four are no transfer,
the reason naming what they lack. Before asking, the transfer's refusals (TRF-010 to TRF-022)
— among them `ArchivedAsset`, `TransferReceivedAboveSent`, `TransferRateMissing`,
`TransferRateNotOne`, `InsufficientCash` —, `DividendBeforeHolding` (DIV-045) and
`DatabaseError`. Answer: `{"status": "replaced", "transfer": …}`, the transfer as TRF-060
reads it, when the owner replaced them; `ReplacementRefused` when the owner refused or the
session ended first; on a yes that replaced nothing, `TransactionChanged`,
`TransactionNotFound`, the code of a refusal checked before asking, or `DatabaseError`; and
`DatabaseError`, its reason saying the transfer was recorded, when it could not be read back.

## replace_with_transfer

AGT-093. Title: "Replace four AI agent recordings with a transfer". Input and the refusals
of its shape: `request_transfer_replacement`'s. `NotYourRecording` (AGT-051): an id with no mark, or that no transaction has. `SessionLimitReached` (AGT-047): the session at its limit, or the transfer's parts not all fitting in it. The four and the checks before writing:
`request_transfer_replacement`'s codes. Answer: `{"status": "replaced", "transfer": …}`, the
transfer as TRF-060 reads it. `TransactionChanged`: one of the four changed, or the transfer
would move another cost, between the check and the write; `TransactionNotFound`: one removed
in that gap; `DatabaseError`: a write that fails, or, its reason saying the transfer was
recorded, a transfer that could not be read back.

## get_rate

AGT-103. Input: `from` and `to`, two currency codes, case ignored; `date`, `YYYY-MM-DD`,
optional, today when left out. `MissingArgument`: `from` or `to` missing, empty or not text.
`InvalidCurrency`: a code that is not ISO 4217. `IdentityPair`: two equal codes.
`InvalidDate`: a date that is not a date, not text included, or in the future.
`DatabaseError`: a read that fails. Answer: `{ "status", "from", "to", "date", "rate" }` —
`status` `rated`, or `no_rate` with `rate` `null`; the codes in capitals; the date asked;
`rate`, the rate of that date or the closest earlier one, `{ "date", "rate", "source" }`:
its own date, the rate as decimal text and its source, `Manual`, `Frankfurter`, `Ecb` or
`Agent`.

## record_rate

AGT-104. Input: `from` and `to`, two currency codes, case ignored; `rate`, a figure (text or
a JSON number, AGT-048), `to` per one `from`; `date`, `YYYY-MM-DD`, optional, today when
left out. `MissingArgument`: `from`, `to` or `rate` missing or empty, or `from` or `to` not
text. `InvalidFigure`: a rate that is not a figure. `InvalidArguments`: a date that is not
text. The rate's own refusals (FXR-021–023): `InvalidCurrency` for a code that is not
ISO 4217, `IdentityPair` for two equal codes, `NotPositive` for a rate not above zero,
`RateOutOfRange` past the bound, `DateInFuture`, `InvalidDateFormat`. `SessionLimitReached`
(AGT-047). Answer: `{ "status": "recorded", "from", "to", "rate" }`, the rate as `get_rate`
gives one, its source `Agent`.

## list_rates

AGT-105. Input: `from` and `to`, two currency codes, case ignored; `from_date` and
`to_date`, optional, `YYYY-MM-DD`, both included; `page`, a JSON whole number from 1,
optional, the first when left out. `MissingArgument`, `InvalidCurrency`, `IdentityPair` and
`DatabaseError` as `get_rate`'s. `InvalidDate`: a bound that is not a date, not text
included — a bound in the future, or a start after the end, keeps no rate and is not
refused. `InvalidArguments`: a page that is not a whole number from 1, text of digits
included. Answer: `{ "status": "listed", "from", "to", "total", "page", "pages", "rates" }`
— the rates oldest first, 100 to a page, each as `get_rate` gives one; `pages` 0 when
`total` is 0.

## get_performance

AGT-106. Input: `account`, its name, optional — the portfolio's performance when left out
or `null`; `asset`, named as a recording names it (AGT-044), archived or not, optional;
`year`, a JSON whole number, optional. `AccountNotFound`, `AccountAmbiguous`,
`AssetNotFound`, `AssetAmbiguous`: a name that matches no account or asset, or several.
`InvalidArguments`: an account or an asset that is not text, or empty, or spaces only; a
year that is not a whole number, text of digits included. `DatabaseError`: a read that
fails. Answer: `{ "status": "read", "account", "asset", "currency", "yearly", "monthly",
"lifetime_unavailable" }` — the account's name, or `null` for the portfolio; the asset's
reference, or `null`; the currency the figures are in; the rows newest first, each
`{ "year", "month", "end_value", "previous_value", "cash_flow", "asset_flow", "dividends",
"pnl", "period_over_period", "year_to_date", "since_inception", "annualized_yield" }` —
`month` `null` on a year row; each performance `{ "pct", "gain" }`, or `null` where it has no place or no
baseline — `year_to_date` on a year row, `annualized_yield` on a month row or when PRF-038
leaves it absent, a first period's `period_over_period`, a scoped window with nothing held
(PRF-034, PRF-037, PRF-038, PRF-042, PRF-083) — and `pct` alone `null` when it cannot be
computed (PRF-032, PRF-087); `lifetime_unavailable`, `null`, or why the
lifetime metrics are absent: `{ "reason": "ZeroCostOpeningBalance", "account", "asset",
"date" }` — the account's name, the asset's reference and the opening balance's date — or
`{ "reason": "NoInvestedCapital" }`.
