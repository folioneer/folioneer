# Interactive Brokers — built-in presets

Two built-in presets read Interactive Brokers' history, each from its own Flex Query exported
as CSV: **Interactive Brokers — Trades** reads purchases and sales, **Interactive Brokers —
Cash** reads dividends, deposits and withdrawals. A Flex Query holding both sections writes two
headers in one file, which the import does not read: make two queries.

## Create the two Flex Queries

In the Client Portal: **Performance & Reports → Flex Queries → Activity Flex Query → +**.

For each query, under **Delivery Configuration**:

- **Format**: CSV
- **Include header and trailer records**: No
- **Include column headers**: Yes
- **Include section code and line descriptor**: No
- **Date format**: yyyy-MM-dd
- **Period**: the dates you want to import

### Query 1 — Trades

Section **Trades**, with these fields:

`CurrencyPrimary`, `Symbol`, `Description`, `ISIN`, `TradeDate`, `Buy/Sell`, `Quantity`,
`TradePrice`, `IBCommission`, `NetCash`

### Query 2 — Cash Transactions

Section **Cash Transactions**, with these fields:

`CurrencyPrimary`, `Symbol`, `Description`, `ISIN`, `SettleDate`, `Amount`, `Type`

## Import

Run each query and download its CSV. Import them into the account in this order:

1. The Cash Transactions file, with **Interactive Brokers — Cash**: your deposits and
   withdrawals come in. Its dividends on shares not yet in the account are refused for now.
2. The Trades file, with **Interactive Brokers — Trades**, the added deposits left out: your
   purchases, paid from those deposits, and your sales. A purchase paid with dividend cash is
   refused for now.
3. The Cash Transactions file again: its dividends now come in, and a withdrawal of sale
   proceeds refused in step 1; the deposits and withdrawals already imported show as already
   recorded and are not imported twice.
4. The Trades file again, if step 2 refused a purchase: it now comes in, and the rest show as
   already recorded.

Importing the Trades file first would add a deposit for each purchase the cash cannot pay, and
the real deposits of the Cash file would then count the money twice.

## What is and is not imported

- **Deposits/Withdrawals** — the amount's sign tells them apart: positive a deposit, negative
  a withdrawal.
- **Dividends** and **Payment In Lieu Of Dividends** — dividends, as the file writes them:
  gross, the tax withheld at source being a row of its own.
- **Withholding Tax**, interest, other fees and commission adjustments — not imported; record
  them by hand where they matter.
- A row in another currency than the account's — US shares and dollar dividends in a euro
  account — is refused; record it by hand.
- A file whose columns differ from its preset — a field left out of the query — is refused,
  naming the columns it lacks.
