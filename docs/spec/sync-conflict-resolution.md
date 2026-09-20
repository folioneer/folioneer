# Business Rules — Sync Conflict Resolution (CFR)

## Context

Multi-Device Sync (SYN) lets several installations converge on one portfolio by exchanging **changes** — one recorded modification of one record, carried with the device that made it, its position in that device's sequence, and its **logical timestamp**. SYN defines how changes are recorded, published, and applied. This spec defines **only the outcome** when changes from different devices meet: for every situation in which two devices acted on related data without seeing each other, the single state every device must reach, and whether the user must be told.

Every rule is followed by a scenario. Scenarios use two devices, **Desktop** and **Laptop**, working on the same portfolio while out of sync, then syncing. "Result" is the state on _both_ devices after both have applied everything — there is never a Desktop result and a different Laptop result.

Out of scope here: how anything is shown (SYN-063/066 own the status surface), how files travel, encryption, and how the user resolves an inconsistency (SYN-040). This spec is outcomes, nothing else. It defines no entity of its own: every record kind is defined in SYN, FEE, FXR, ACC, CAT; the two carriers the outcomes rely on — each record's last-change timestamp and the **tombstone** a removal leaves — are defined in SYN's Entity Definition. The design is the plain last-writer-wins register with logical timestamps and tombstones, as used by local-first applications generally; nothing cleverer, because concurrent edits are expected to be rare.

**Design constraint.** All outcomes in this spec are decided by a single **backend** resolution component with no other responsibility; no other part of the application — and never the interface, which only displays outcomes — decides a merge outcome. A change to this spec is therefore a change to that component alone (ADR-019).

---

## Business Rules

### Foundations (010–019)

**CFR-010 — Later means greater logical timestamp (backend)**: Between two changes, the later one is the one with the greater logical timestamp; equal timestamps are ordered by the identity of the device that made them. A device's clock never decides the order of changes: a logical timestamp is always greater than every change the device had recorded or applied before it (SYN-025). (The order in which transactions are _replayed_ is a different question, CFR-041.)

> _Scenario._ Laptop's clock runs an hour slow. Laptop applies Desktop's rename of an account (timestamp 1 000), then renames it again: stamped 1 001, not its clock's earlier reading, so Laptop's rename is the later one everywhere. Separately, two changes stamped exactly 1 050 on Desktop and on Laptop: Laptop's identity sorts after Desktop's, so Laptop's is the later one — on both devices.

**CFR-011 — Concurrent changes (backend)**: Every change carries the logical timestamp of the record state it was made against (`based_on`, SYN Entity Definition; absent for a creation). A change based on a state this device has not yet received is **held back** until that state arrives (SYN-041), so by the time any change is compared, its base is known. An incoming change is then **concurrent** with this device's state of the record — live record or tombstone — when its `based_on` differs from that state's timestamp (CFR-014, CFR-015); a creation (`based_on` absent) is concurrent with any state this device already holds for that identity. A change whose `based_on` equals it is **sequential**. Concurrency never decides an outcome (CFR-020 does); it decides only whether a superseded change is reported (CFR-060).

> _Scenario._ Desktop renames "PEA" to "PEA Boursorama" (timestamp 1 010). Laptop syncs, sees it, and renames it to "PEA Bourso" — based on 1 010: sequential, applies, nobody is told. Office receives Laptop's rename before Desktop's: it is based on a state Office has not seen, so it waits; Desktop's arrives, then Laptop's applies as sequential. Had Laptop renamed before syncing, its change would be based on the older state: concurrent, and Desktop is told.

**CFR-012 — Record identity per kind (backend)**: Two changes concern the same record when they have the same kind and the same identity: accounts, categories, assets, and transactions by their identifier; fee schedules by their account and asset (CFR-034); currency pairs by their two currencies (CFR-034); asset prices by asset and observed date; currency rates by pair and observed date; holding notes by account and asset. Identity here means identity _between devices_ — whatever key a device stores a record under locally is its own business.

> _Scenario._ Desktop records the TotalEnergies price for 2026-08-20; Laptop records the TotalEnergies price for 2026-08-21. Different identities — both prices exist afterwards, no conflict.

**CFR-013 — Order of arrival never matters (backend)**: Whatever order a device receives changes in — Desktop's area first or Laptop's first, in one sync or across several — the final state is the same. Every outcome below depends only on values carried by the changes themselves. One bounded exception is stated where it lives: fields a change written in an older data format does not know keep their local value (SYN-035), so two devices can differ on such a field until the older device is updated and re-publishes.

> _Scenario._ A third device, Office, receives Laptop's segments a day before Desktop's. Once it has both, Office holds exactly what Desktop and Laptop hold.

**CFR-014 — Every record remembers its last change (backend)**: Every synced record carries the logical timestamp, origin, and device of the change that produced its current state — its rank (CFR-020) — so CFR-020, the CFR-010 tie-break, and the notice locus (CFR-060) are all evaluated from the record alone. Records that existed before sync was first enabled take the first segment's logical timestamp, the _user_ origin, and the publishing device (SYN-013) — one value, the same everywhere; generated deductions that existed before sync are therefore ranked as user records from then on, which changes nothing visible since their identity and content are deterministic.

> _Scenario._ An old edit from a long-paused device arrives at Office; Office compares its rank with the one stored on the record and ignores it correctly — no log lookup involved.

**CFR-015 — Every removal leaves a tombstone (backend)**: Removing a synced record leaves a **tombstone** — the record's kind and identity, and the removal's logical timestamp, origin, and device — that stands in for the record in CFR-020: a change of higher rank brings the record back (CFR-022), one of lower rank is ignored. A device holds exactly one state per identity — the live record or the tombstone, whichever ranks higher. Tombstones are kept permanently; because published history is never removed (SYN-037), a device that joins later derives the same tombstones by replaying it.

> _Scenario._ Desktop deletes a holding note (timestamp 1 030). Office joins six months later and replays the whole history. A long-paused Laptop resumes and publishes an edit of that note stamped 1 025. Office holds the tombstone from its replay, compares, and ignores the edit — exactly as Desktop does.

**CFR-016 — A change the user made beats a change the application made (backend)**: Every change carries its **origin**: _user_ when it records something the user did (typed, edited, removed, confirmed — including the consequences the application carries out for that action, such as a cascade or a category reassignment), _application_ when the application generated it on its own (a scheduled fee deduction, a catch-up position, an automatic price or rate download). Origin is the first part of a change's **rank** (CFR-020): a user change outranks every application change to the same record, whatever their timestamps. This binds the application's own local writes too: before the application writes a generated record, it compares as CFR-020 does, and a write that does not outrank the current state is not made and produces no change — so no device ever holds what another refuses. The application never writes to a user-created record: what it produces (a deduction, a catch-up position, an observation) is always a record of its own (CFR-034). Observations are exempt (CFR-050); catch-up positions merge by maximum (CFR-044). This rule is what keeps a generated fee deduction the user deleted from coming back.

> _Scenario._ Desktop's user deletes July's generated fee deduction (user, timestamp 1 100 → tombstone). Laptop, not yet synced, is about to regenerate July (application, 1 200): Laptop does not hold the tombstone yet, so it writes. When the two sync, both compare ranks — user beats application — and the deletion stands on both. Had Laptop already held the tombstone, its own write would have been refused locally, identically.

**CFR-017 — Applying never re-validates (backend)**: Applying a change runs none of the guards that the user's entry would have run — not the oversell or insufficient-cash guards, not archived-asset, unheld-asset, or date checks. A change that was valid where it was recorded is applied as it is. The only consequence merge surfaces is a holding invariant broken by the combination (CFR-042); every other combination simply stands, for the user to review.

> _Scenario._ Desktop archives an asset; Laptop, unsynced, records a buy on it. After sync the buy exists on both devices, on an archived asset; nothing is rejected, nothing is reported.

### Same Record Edited on Both Sides (020–029)

**CFR-020 — The higher rank prevails in full (backend)**: Every change, and every stored state (CFR-014, CFR-015), has a **rank**: first its origin (user above application, CFR-016), then its logical timestamp, then the identity of the device that made it (CFR-010). An account's tombstone has the highest rank of all (CFR-022). Ranks are totally ordered: any two differ, and the comparison is the same on every device, whatever order changes arrive in (CFR-013). On every apply — incoming or local — a change to a record prevails if its rank is higher than the rank of the record's current state, and is ignored otherwise; a record this device has never seen is created. When it prevails it prevails **entirely**: every field comes from it; fields are never merged one by one. Two record kinds are merged by their own rule instead of by rank: observations (CFR-050) and catch-up positions (CFR-044). One field-level exception: fields a change written in an older data format does not know keep their local value (SYN-035). Concurrency (CFR-011) decides only whether the superseded change is reported (CFR-060).

> _Scenario._ Desktop sets the bank name of account "CTO" to "Fortuneo" (timestamp 1 010). Laptop, unsynced, renames "CTO" to "CTO Fortuneo" (timestamp 1 020). Result: name "CTO Fortuneo", bank name **unchanged from before** — Desktop's bank-name edit is overruled and reported on Desktop.

**CFR-021 — Identical concurrent changes are not a conflict (backend)**: When two concurrent changes produce the same content, the record takes that content and nothing is reported.

> _Scenario._ Both devices archive the same asset. Result: archived; nobody is told.

**CFR-022 — Update versus removal: the higher rank prevails — except that an account's removal is final (backend)**: When one device removed a record and the other updated or re-created it concurrently, CFR-020 decides: a prevailing removal removes (leaving its tombstone, CFR-015); a prevailing update or re-creation brings the record back with its content, superseding the tombstone. The superseded side is reported — an overruled edit or an overruled removal (CFR-060). **Accounts are the exception**: an account's tombstone has the highest possible rank, so a removed account is never brought back by any change, whatever its origin or timestamp; a concurrent update of it is overruled and reported, and its children follow CFR-030/032. A deletion of an account, once made, is made everywhere.

> _Scenario._ Desktop deletes holding note on (CTO, Air Liquide) at 1 030; Laptop edits that note's text at 1 040. Result: the note exists with Laptop's text; Desktop is told its deletion was overruled. Reverse the timestamps and the note is gone; Laptop is told its edit was overruled. By contrast, Desktop deletes account "Old PEA" (1 030) while Laptop renames it (1 040): the account stays deleted on both devices and Laptop is told its rename was overruled.

**CFR-023 — Removal versus removal (backend)**: Two concurrent removals of the same record leave it removed; nothing is reported.

> _Scenario._ Both devices delete the same one-off fee transaction. Result: gone, silently.

### Parent and Child Records (030–039)

**CFR-030 — Cascading removal is explicit per record (backend)**: Removing an account — the only record that owns others — is one removal change, and one tombstone, per removed record: the account, its transactions, its holding notes, its fee schedules and their catch-up positions (SYN-024). A device applying an account's tombstone also removes every child of that account it holds, including children the removing device never knew of (CFR-032); the tombstones it derives for those carry the account tombstone's rank. Deleting a fee schedule keeps its generated deductions (FEE-062). Assets are archived, never removed (AST-006). Deleting a category reassigns its assets to the default category on the deleting device (CAT-003) as ordinary changes of user origin, which merge under CFR-020 like any other; and whatever content prevails afterwards, an asset whose category's latest state is a tombstone is **shown** in the default category — derived on read — so every device shows the same thing.

> _Scenario._ Desktop deletes account "Old PEA" holding 12 transactions and 1 note: fourteen tombstones are published and Laptop removes exactly those fourteen records. Desktop deletes the category "Tech": its three assets are reassigned on Desktop (three user changes) while Laptop, unsynced, renames asset X (still in Tech). After sync X carries whichever content ranks higher — Laptop's rename, being later — still pointing at Tech, and is shown in the default category on both devices.

**CFR-031 — Child before parent is waited for, not rejected (backend)**: A change that refers to a record this device has not received yet — a transaction on an account created elsewhere whose creating change has not arrived — is held back and applied when the record arrives (SYN-041). Until then the rest of the sync proceeds.

> _Scenario._ Laptop creates account "Livret" and records a deposit on it. Office receives Laptop's area in two pieces: the deposit first. The deposit waits; when the account arrives, the deposit applies. Office never shows a deposit without its account. "Arrived" means received — whether the awaited change is then applied, ignored, or dropped.

**CFR-032 — A child of a removed account is dropped and reported (backend)**: A change whose account — a transaction, holding note, or fee schedule of it — is a tombstone on this device (CFR-015) is **dropped**: not applied, not kept. Likewise, when an account's tombstone arrives, every child of that account this device holds is removed with it (CFR-030). In both cases the user loses the child; it is reported on the device whose change created or last edited that child (CFR-060), naming the record and the removing device, and on no other device. This is the one case in which merge drops a transaction; it is never silent. A removal change for such a child is never dropped: it is a tombstone itself.

> _Scenario._ Monday, Desktop deletes account "Old PEA". Tuesday, Laptop (unsynced) records a buy on "Old PEA". Wednesday, both sync. Desktop receives the buy, finds the tombstone, drops it; Laptop receives the tombstone and its own buy is removed with the account. Laptop — whose change lost — shows _"Buy 10 × TotalEnergies on Old PEA dropped — account deleted on Desktop"_; Desktop shows nothing. Office, which neither deleted nor bought, ends in the same state and shows nothing either.

**CFR-033 — System-seeded records never block (backend)**: A change referring to a record the application seeds itself with a fixed identity — the cash asset per currency, the cash category — is never held back: the record is ensured locally on apply (SYN-027).

> _Scenario._ Laptop opens the first USD account and records a USD deposit. Desktop has never had USD. On apply, Desktop seeds its own `system-cash-usd` and applies the deposit immediately.

**CFR-034 — Whatever the application generates has a predictable identity (backend)**: A record the application creates on its own — not typed by the user — takes its identity from what it represents, so that it is known **before the record exists** and is the same on every device: a generated fee deduction from its account, asset, and period boundary (FEE-048); a fee schedule's catch-up position from the schedule's account and asset (CFR-044); a currency pair from its two currencies (FXR-054); a fetched price or rate from what it observes and when (CFR-012); the cash asset and category from the currency and a fixed name (CSH-011/017). Only what the user types gets a fresh identity — a fee schedule, though user-typed, is likewise identified by its account and asset (FEE-031). Two devices that generate the same thing therefore produce the **same** record, resolved by CFR-020 — there is nothing to detect and no special case. A **collision** is an incoming creation (`based_on` absent) for an identity this device already holds live with different content: CFR-020 decides which content stands, and when **both** creations are of user origin it is reported on both creating devices (CFR-060); any other collision — observations (CFR-050), application against application, application against user — is never reported. Deductions already generated under superseded schedule settings remain ordinary transactions (CFR-040).

> _Scenario._ On 1 September both devices are opened before syncing; each generates August's deduction for (CTO, Amundi MSCI World). Both compute the identity from (CTO, Amundi, 2026-08-31) — the same value — so after merge there is one August deduction, charged once. Separately, Desktop and Laptop each create a schedule on that holding while unsynced (0.5 % monthly vs 0.6 % quarterly): same identity, one schedule with the outranking settings, a notice on both, Desktop's already-generated monthly deductions left in the ledger for the user to review.

**CFR-035 — Duplicate names coexist (backend)**: Accounts and categories are identified by their own identifier, not their name. When a merge leaves two of them with the same name — created independently, or one renamed into the other's name concurrently — both survive with that name, each with its own history, and it is reported on **both** devices whose changes carry that name (CFR-060). The user renames one by hand; renaming either is always accepted — the uniqueness check of ACC-003 / CAT-001 binds the name being set, not names that already clash.

> _Scenario._ Both devices create an account named "Livret A" and record deposits on it. Result: two accounts named "Livret A", each with its own deposits, and a notice on both devices. The user renames one.

### The Ledger (040–049)

**CFR-040 — Transactions accumulate (backend)**: After sync, the portfolio's transactions are every transaction created on any device, minus those removed (CFR-022, CFR-023, CFR-032). A transaction no device removed is always present everywhere.

> _Scenario._ Desktop records two buys, Laptop records a sale, on different days, without syncing. Result: all three exist on both devices.

**CFR-041 — Replay order is the same on every device (backend)**: Holdings are recomputed by replaying transactions by date, then by the creation instant the creating device recorded (TRX-036) — carried verbatim with the transaction and never re-derived — then, when both are equal, by transaction identity. All three values travel with the transaction, so every device replays in the same order and computes the same holdings and performance (SYN-022). The creation instant is the creating device's clock; it decides replay order between same-dated transactions, not merge outcomes.

> _Scenario._ Desktop and Laptop each record a buy dated 2026-08-20 while unsynced. Laptop's carries the earlier creation time. Both devices replay Laptop's buy first — including Desktop, which received it second.

**CFR-042 — A merge that breaks a holding invariant keeps every transaction (backend)**: When transactions that were each valid where they were recorded together oversell a position or overdraw the cash holding (CSH-080), all of them are kept and the holding — cash holding included — is **inconsistent**, with the reason, until the replayed ledger is valid again (SYN-040). Inconsistency is derived from the ledger on every recomputation, never stored or synced. Merge never silently drops or alters a transaction to restore an invariant.

> _Scenario._ 15 TotalEnergies held. Desktop sells 10; Laptop, unsynced, sells 10. Result: both sales exist; the TotalEnergies holding shows −5 and is marked inconsistent on both devices until the user corrects one sale.

**CFR-043 — Generated fee deductions are one record by construction (backend)**: A deduction generated for the same holding and period on two devices has the same identity (CFR-034, FEE-048) and is therefore one record after merge; the holding is charged once. Its content follows CFR-020: the later generation prevails.

> _Scenario._ Both devices are opened on 1 September before syncing; each generates August's 0.5 % deduction for (CTO, Amundi MSCI World). Same identity on both: one August deduction after merge.

**CFR-044 — A fee schedule's catch-up position is its own record, merged by maximum (backend)**: The catch-up position (FEE-043's cursor, `last_applied_period`) is not a field of the schedule: it is a separate synced record of application origin, identified by the schedule's account and asset (CFR-034), written only by the application when it generates or skips a period. Two states of it merge by **maximum**, not by rank: whenever a change to it is applied the stored position becomes the more advanced of the stored and the incoming one, so it never moves backwards, whatever order changes arrive in, and every device converges on the same value because all of them eventually see the same changes. The schedule itself stays a user record (CFR-016). A period charged or skipped on any device is never charged again.

> _Scenario._ Desktop generates August: its catch-up record for (CTO, Amundi) becomes August and that change is published. Laptop, unsynced since July, still holds July; on sync it takes the maximum — August — and generates nothing. Reverse the arrival order on a third device and the result is the same: August.

### Observations (050–059)

**CFR-050 — Observations: latest write wins, whatever the source (backend)**: Asset prices and currency rates are observations of a value on a date (CFR-012). Between two changes to the same observation, the later per CFR-010 prevails — **origin is not considered** (ADR-012: latest write wins, source is metadata) — and nothing is reported: an observation has no authorship worth defending. Observations on different dates are different records; both survive.

> _Scenario._ Desktop's user hand-corrects Friday's TotalEnergies close to 58.10 (1 000); Laptop's scheduled download records 58.25 for the same Friday (1 300). Result: 58.25 everywhere — the later write, as ADR-012 decides for a single device too. Nothing is reported.

### What Must Be Reported (060–069)

**CFR-060 — Reported outcomes (backend)**: Exactly these outcomes produce a conflict notice (SYN-066): a concurrent user update superseded (overruled edit) or a concurrent user removal superseded (overruled removal) (CFR-020, CFR-022); a child dropped or removed because its account stands removed (CFR-032); a natural-key collision between two user creations (CFR-034); and a duplicate name (CFR-035). A notice is raised **on the device whose own change lost** — the one whose change was superseded, or whose child was dropped — and, for collisions and duplicate names, on both devices whose changes clash; never elsewhere. Sequential changes (CFR-011), identical changes (CFR-021), double removals (CFR-023), held-back-then-applied changes (CFR-031), application changes outranked by user changes (CFR-016), generated-deduction convergence (CFR-043), catch-up maxima (CFR-044), and observation overwrites (CFR-050) are never reported. An inconsistent holding (CFR-042) is reported through its own derived marker (SYN-040), not as a notice.

> _Scenario._ After the week above, Desktop's status lists one overruled bank-name edit and nothing else — not the three transactions that merged cleanly, not the prices.

---

## Open Questions

None — all questions have been resolved.
