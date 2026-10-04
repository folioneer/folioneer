# Business Rules — Numbers by language (NUM)

## Context

A number's decimal separator follows the application's language everywhere in the window: a comma in French, a dot in English. The thousands separator is out of scope: no figure is typed or read with one. Figures travel between the window and the core as integers (TRX-024); these rules are about the text a user reads and types. The command line is ruled by CLI-015.

No entity and no command: these rules add nothing to a contract.

---

## Business Rules

**NUM-001 — One separator per language (frontend)**: In the window, a number is shown and typed with the decimal separator of the application's language: a comma in French and its regional variants, a dot in every other language.

**NUM-002 — Which language, and when it changes (frontend)**: The application's language is the one chosen in the settings; with no choice, the system's language when it is French or English, English otherwise. When the language changes, every figure shown follows at once, and a number field shows again the figure its form holds, written for the new language.

**NUM-010 — Reading a number field (frontend)**: Every number field reads what is typed the same way. In French a comma or a dot is the decimal separator — a numeric keypad types a dot. In English only the dot is: a comma is left as typed. A text is a number when, so read, it is digits with at most one separator and an optional leading minus; anything else is no number — a comma in English, a text holding both a comma and a dot or a space (`1.234,5`, `1 234,5`), a text that only starts like a number — and the form refuses it as it refuses any text that is not a number. A dot typed in French is always a decimal separator: `1.234` is read as one and 234 thousandths.

**NUM-011 — Writing a number field (frontend)**: Every number field writes a figure the same way: with a comma in French, with a dot in English. In French a typed dot shows as a comma at the keystroke, the cursor staying where it was. A field filled from a recorded or computed figure is written the same way, with every decimal the figure carries (TRX-024).

**NUM-012 — A calculation typed in a field (frontend)**: A number field accepts a calculation (`+ - * / ( )`): its numbers are read per NUM-010, its result is shown per NUM-011, and the form receives the result.

**NUM-013 — Figures that are only shown (frontend)**: A figure the user does not type — in a table, a total, a form's read-only line, a message, a chart's axis, the exchange rates list, a split's factor — is written by the number format of the application's language.

---

## Where a number is typed or shown

| Where                                                                                                                                                                                                                                | Typed or shown     | Rule                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------ | ----------------------------------------- |
| Transaction forms: purchase, sale, opening balance, deposit, withdrawal, dividend, free shares, interest, management fee, the price after a split, a fee schedule's rate, a holding note's threshold, a price's entry and correction | typed              | NUM-010, NUM-011, NUM-012                 |
| The same forms opened on a recorded transaction                                                                                                                                                                                      | filled, then typed | NUM-011 (every recorded decimal, TRX-024) |
| An exchange rate's entry and correction                                                                                                                                                                                              | typed              | NUM-010, NUM-011                          |
| The price of an asset a fetch left unpriced                                                                                                                                                                                          | typed              | NUM-010, NUM-011                          |
| A split's ratio, an asset's risk level                                                                                                                                                                                               | typed              | whole numbers: no separator               |
| Tables and totals: holdings, journals, performance, price movement, accounts                                                                                                                                                         | shown              | NUM-013                                   |
| Figures shown in forms and messages: a draft's total, the quantity that can be sold, a split's preview, the average cost and the gain a sale would realize, the last known price, the balance in a refusal                           | shown              | NUM-013                                   |
| The exchange rates list, a split's factor in a journal                                                                                                                                                                               | shown              | NUM-013                                   |
| The value chart's axis                                                                                                                                                                                                               | shown              | NUM-013                                   |
| Exports                                                                                                                                                                                                                              | —                  | the application exports no figures        |
| The command line                                                                                                                                                                                                                     | typed and printed  | the dot only: CLI-015                     |

---

## UX Draft

No screen changes: the fields and the figures are where they were. In French a field shows a comma where it showed a dot.

---

## Open Questions

None.
