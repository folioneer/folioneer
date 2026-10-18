# TODO-109 — (fullstack) — An asset whose price is typed by hand says so, not "locked"

From the owner (2026-10-18), on TODO-068. The per-asset setting of MKT-150 exists for an asset whose price cannot be fetched: the fetch and the backfill leave it, and nothing warns that it is not fetched. The spec and the field call it a lock ("Price refresh lock", `price_refresh_blocked`), the window says both "Block automatic price updates" and "Automatic price updates are locked for this asset", and the word suggests a protection; set by mistake, it left a listed stock's price unmoved for ten weeks with nothing saying why (gh242). "Lock" is kept for TODO-068: one checked price nothing automatic replaces.

**User value:** An owner reads at once that an asset's price is theirs to type, and why it is not fetched.
**Done when:** the setting is named "Price typed by hand" everywhere — the asset's actions "Type its price by hand" / "Fetch its price automatically", the asset's row "Price typed by hand", its messages, in every interface language; the market-price spec (MKT-150 and after) and the glossary say so, "lock" left to TODO-068; TODO-085's reason for a price not updated names it in the same words; nothing else of its behaviour changes.
**Size:** M — one pull request across several lanes: the strings in every language, the market-price spec and the glossary; the wording is given.
**Design:** validated
**Open questions:**

- [x] The code's names too, or the words only? — The words only: `price_refresh_blocked` and its actions already say what it is; "lock" was the wrong word, on screen and in the spec. (owner, 2026-10-18)
