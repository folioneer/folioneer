# An AI agent's connection (MCP)

What an AI agent sends and is answered, tool by tool. The core's behaviour behind each tool
is its spec rule (`docs/spec/agent-connection.md`), named and never restated. A tool takes
no other argument than its own, and one that arrives all the same is ignored (AGT-040). The
tools not described here keep their input and answer in their spec rules until a task moves
them.

## list_accounts

AGT-040, AGT-102. Answer: one object per account, by name — `name`, `currency`, `country`
(two letters: its own, or the portfolio's, ACC-042), `wrapper` (an identifier, or `null`) and `holder` (or `null`).

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
