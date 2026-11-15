# Scalable Capital — built-in preset

The **Scalable Capital** preset reads the broker's transactions export, a CSV file the client
area offers PRIME and PRIME+ customers.

## Export

In the client area's transactions view, export the transactions as CSV, with the semicolon as
separator. The file's header is:

`date;time;status;reference;description;assetType;type;isin;shares;price;amount;fee;tax;currency`

## Import

Import the file into the account with **Scalable Capital**.

## What is and is not imported

- **Buy** and **Sell** — purchases and sales, with their fees.
- Any other value of the `type` column — a distribution, a deposit, a withdrawal — has no type in
  the preset: its rows show as untyped in the preview. Save a copy of the preset, give each such
  label its type, and import with the copy. The preset was built from a published sample that
  holds only purchases and sales.
- The `status` column is not read: export the executed transactions only.
- A row in another currency than the account's is refused; record it by hand.
- A file whose columns differ from the preset is refused, naming the columns it lacks.
