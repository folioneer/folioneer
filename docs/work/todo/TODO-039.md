# TODO-039 — (service) — A hosted price feed the application can subscribe to (deferred)

What is sold is what a server of the owner's provides: first a price feed under a licence that allows it, later bank feeds, perhaps advice. The application stays free and open, so nothing sold can live in it — a switch in an AGPL client is one fork away from being flipped. The client is ordinary public code: one more price provider behind the seam of TODO-036 and TODO-037, which calls the owner's service with the subscriber's token. The inventory found no licensed replacement — finding and pricing one is this entry's first task. Deferred until then, and until the application is worth showing (TODO-030).

**User value:** A subscriber gets prices fetched for them, daily and on demand, without typing them and without the application standing on an endpoint it has no right to use.
**Done when:** To be written when the entry is scheduled — the service, its licensed source, the subscription and the client each deserve their own entry.
**Design:** none
**Open questions:**

- [x] Which words name the paid feeds? "Sync" already means multi-device sync in the vocabulary. (Recommended: "price feed" and "bank feed".) — "Price feed" ("Folioneer price feed" in the provider list of TODO-093) and "bank feed"; "sync" keeps its one meaning. The terms enter the glossary with the entry that ships them. (owner, 2026-10-18)
- [x] Does multi-device sync through a folder the user owns stay free, a hosted sync being a separate paid offer? (Assumed yes when the paid offer was settled, 2026-09-20.) — Yes: sync through a folder the user owns stays free and open source for good; only what costs to run (the price feed, a hosted sync) is paid. (owner, 2026-10-18)
