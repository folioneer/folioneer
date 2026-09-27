# ADR 021 — Withdraw the keyless price source decision

**Date**: 2026-09-27
**Status**: Accepted — supersedes ADR-017

## Context

ADR-017 set up a keyless architecture for prices: Yahoo Finance's chart endpoint as the sole automated source, and, as its consequence, no user-supplied API key. Yahoo's terms forbid automated access ([`external-dependencies.md`](../external-dependencies.md)), so the public build no longer calls it (#038): it has no automated price source, and prices are entered by hand (MKT-210). The owner's private build keeps a Yahoo client through its extension file (ADR-020). What will provide prices to others — a hosted feed, and how its subscribers identify themselves (#039) — is not decided.

## Decision

ADR-017 is withdrawn in full. Neither of its two choices holds any longer: there is no sole price source to build around, and the absence of user-supplied credentials was a consequence of that source, not a decision of its own. No replacement is decided here; the next price source and its credentials are decided when that source exists. ADR-020's closing remark that Yahoo Finance remains the price source the public file plugs in no longer holds: the public file plugs in none.

## Consequences

- **Pros**: no record states an architecture the code has left; a paid feed or any keyed provider can be designed without first overturning a standing decision.
- **Cons**: until a new source is decided, nothing records how prices will reach users other than the owner; the question stays open in #039.
