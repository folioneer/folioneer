# Scalable Capital — built-in preset

The **Scalable Capital** preset reads the broker's transactions export, a CSV file the client
area offers PRIME and PRIME+ customers (Scalable Capital, "Export all broker transactions with
ease"). It was built from one user's published file, which holds only purchases and sales.

## Export

In the client area's transactions view, export the transactions as CSV, keeping the semicolon
as separator. The file's header is:

`date;time;status;reference;description;assetType;type;isin;shares;price;amount;fee;tax;currency`

## Import

Import the file into the account with **Scalable Capital**.

## What is and is not imported

- **Buy** and **Sell** — purchases and sales, with their fees.
- Any other value of the `type` column — a distribution, a deposit, a withdrawal — has no type in
  the preset: its rows are rejected in the preview, the label as reason. Save a copy of the
  preset, give each such label its type, and import with the copy.
- The `status` column is not read: a row whose status is not `Executed` — a cancelled order —
  is read as the others. Delete such rows before importing.
- The `tax` column is not read: a tax withheld on a sale is not taken from its proceeds. Record
  it by hand where it matters.
- The file is read as UTF-8; a file in another encoding is refused.
- A row in another currency than the account's is refused; record it by hand.
- A file lacking a column the preset names is refused, naming each one.
