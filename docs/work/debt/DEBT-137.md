# 2026-10-17 — DEBT-137 — Crypto quantities need more than six decimals

- Found by: the owner, answering gh274 (2026-10-17): the MVP only fixes the parsing of small numbers; the precision itself is deferred here
- Where: the quantity type shared by the core, the database and the wire (millionths, TRX-024)
- Severity: 🔵
- Size: L — a schema migration, the contract and every layer that reads a quantity.
- Observation: quantities are stored in millionths. Bitcoin counts in satoshis (8 decimals), Ethereum in wei (18): network fees of a few hundred satoshis and the last digits of a wallet balance cannot be recorded, and a rebuilt account drifts a few satoshis from the wallet (seen on 2026-10-17 rebuilding four crypto accounts).
- User value: A crypto holder records fees and balances exactly as the wallet shows them.
- Done when: a satoshi (0.00000001 BTC) can be recorded and read back unchanged through the window, an agent and the command line; existing quantities migrate without change; the golden portfolio does not move.
