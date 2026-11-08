# Business Rules — Profile (PFL)

## Context

The owner follows portfolios that are not all theirs: their household's and a relative's (#278). A profile is a set of accounts, each account in exactly one, so that a relative's accounts are never summed into the owner's figures. A profile is the closest thing to a login shared on one installation, isolated from the others (owner, 2026-11-08): every figure computed across accounts is to stay within one profile (Open Questions); the asset base and the prices are shared (PFL-052), and the sync carries every profile (owner, 2026-10-17). Profiles belong to the account context. This spec is the core's; the window (#767), the overview (#768), an account's holder (#769) and an AI agent bound to one profile (#770) build on it.

---

## Entity Definition

### Profile

A set of accounts the owner follows apart from the others, named or, for the default profile, not yet named.

| Field  | Business meaning                                                                |
| ------ | ------------------------------------------------------------------------------- |
| `id`   | Its identifier; the default profile's is the same on every device (PFL-011).    |
| `name` | The name the owner gave it; none for the default profile until named (PFL-011). |

### Account (amended)

| Field        | Business meaning                    |
| ------------ | ----------------------------------- |
| `profile_id` | The profile the account belongs to. |

---

## Business Rules

### Eligibility and Initiation (010–019)

**PFL-010 — One profile per account (backend)**: Every account belongs to exactly one profile.

**PFL-011 — The default profile (backend)**: A portfolio always has its default profile, created without a name; it is one and the same profile on every device of the sync.

**PFL-012 — The profile in use (backend)**: The profile in use is this installation's: kept across restarts and never synced. It is the default profile until another is chosen.

**PFL-013 — Accounts from before profiles (backend)**: Every account that existed before profiles belongs to the default profile.

**PFL-014 — The profile in use is gone (backend)**: A profile in use that no longer exists — removed here or on another device — gives way to the default profile.

### Creation (020–029)

**PFL-020 — A new profile (backend)**: A profile is created with a name, trimmed: an empty name is rejected with a specific error, and so is a name another profile has, case ignored.

**PFL-021 — A new account's profile (backend)**: A new account belongs to the profile it is created in: the profile in use, unless another is given; an unknown profile is rejected with a specific error.

**PFL-022 — Account names within a profile (backend)**: Two accounts of one profile never share a name (ACC-003); two accounts of different profiles can (owner, 2026-11-08).

**PFL-023 — Reading the profiles (backend)**: The profiles are read, each with the number of its accounts: the default profile first, named or not, then the others by name, case ignored, then by identifier; the profile in use is given with them.

### Updates and Status Changes (030–039)

**PFL-030 — Renaming (backend)**: A profile, the default one included, is renamed under PFL-020's checks; an unknown profile is rejected with a specific error.

**PFL-031 — Moving an account (backend)**: An account moves to another profile with its whole history; nothing of it is rewritten, and every figure of both profiles follows on its next read. Moving it to the profile it is in, or reads as in (PFL-061), changes nothing. An unknown account or profile is rejected with a specific error, and so is a move that would give the destination profile two accounts of one name (PFL-022). Editing an account never changes its profile; only a move does.

**PFL-032 — Choosing the profile in use (backend)**: Choosing an existing profile makes it the profile in use (PFL-012); an unknown one is rejected with a specific error; choosing the profile in use changes nothing and signals nothing.

**PFL-033 — No transfer across profiles (backend)**: A transfer links two accounts of one profile (owner, 2026-11-08): moving an account a transfer links to an account that would then be in another profile is rejected with a specific error — recording such a transfer is TRF-010's refusal: the account moves only once the transfer is cancelled, a profile being isolated (PFL-052).

**PFL-034 — Signaled (backend)**: Creating, renaming or removing a profile and choosing the profile in use are signaled by `ProfileUpdated`; moving an account by `AccountUpdated` (ACC-022), which the counts of PFL-023 follow too.

### Removal (040–049)

**PFL-040 — Removing a profile (backend)**: Only a profile holding no account is removed; one holding an account, the default profile and an unknown one are each rejected with a specific error.

### Shared across profiles (050–059)

**PFL-052 — What stays shared (backend)**: The asset base — assets and their categories — and the prices — asset prices and currency rates — are shared by every profile (owner, 2026-11-08), and so are, until the per-profile setting lands (Open Questions), the one reference currency (GPF-011) and the one portfolio country (ACC-041).

**PFL-054 — This installation's settings (backend)**: This installation's own settings (SYN-023) — among them the import mappers (IMP-020), the price provider (MKT-214), the scheduled fetch (SPF) and the agent connection (AGT-022) — are one per installation and stay the same whatever the profile in use (owner, 2026-11-08).

### Sync (060–069)

**PFL-060 — Synced (backend)**: Profiles are a kind of synced record and each account carries its profile (SYN-021, SYN-038). A change of an account written in a data format version before 17 leaves its profile as it is (SYN-035); an account such a change creates belongs to the default profile.

**PFL-061 — A profile removed elsewhere (backend)**: An account whose profile no longer exists — removed on another device — is read as belonging to the default profile; nothing is written for it.

**PFL-062 — One name twice after a merge (backend)**: A merge can leave one name twice; PFL-062A to PFL-062C say which case is reported (CFR-035), and renaming either to a free name is accepted.

**PFL-062A — Given at once (backend)**: Two profiles, or two accounts of one profile, given one name on two devices at once both keep it and are reported; setting that name on a third is rejected (PFL-020, PFL-022).

**PFL-062B — A move meets a name (backend)**: A move on one device bringing an account into a profile where another device has meanwhile given its name to an account is reported the same way, since the moved account's change carries its name.

**PFL-062C — The fallback meets a name (backend)**: The default profile's fallback (PFL-061) leaving it two accounts of one name stands, reported by no device, since no change carries the name.

**PFL-063 — A transfer across profiles after a merge (backend)**: A move on one device and a transfer recorded on another at once can leave a transfer linking two profiles; the merge keeps it as it is (CFR-017), nothing is refused for it; a later correction of it is rejected (TRF-030 re-checks TRF-010) and cancelling it (TRF-040) is accepted.

**PFL-064 — The default profile in the sync (backend)**: Like a system-seeded record (SYN-027) the default profile exists on every device with no recorded change; unlike one, a rename of it is recorded and synced (SYN-020), and a default profile named before sync was enabled is published in the first segment (SYN-013); an unnamed one is not.

---

## Open Questions

- [x] Can an account move? — Yes, its whole history with it (owner, 2026-11-07, #278).
- [x] Shared assets and prices; one sync for every profile? — Yes (owner, 2026-10-17, #278).
- [x] The default profile's name? — None until named; the window names it in its language (owner, 2026-11-08).
- [x] A transfer between two profiles? — No longer possible (owner, 2026-11-08).
- [x] Account names unique across profiles? — Within a profile only (owner, 2026-11-08).
- [ ] An export holds every profile's accounts; whether it records each account's profile is #793's or a follow-up's to settle.
- [x] What do profiles share? — Each is isolated, the closest thing to a login shared on one installation; the asset base and the prices are shared (owner, 2026-11-08).
- [ ] A reference currency and a country per profile, a new profile starting with the default profile's (owner, 2026-11-08): the rule and its amendments land with its implementation — the portfolio setting keyed by profile (FXR-015, FXR-120 to FXR-126, CFR-012, SYN-021, a SYN-038 version), GPF-011, ACC-041, ACC-042, CBM-014, EXP-021, the GPF context and the glossary's "Reference currency" and "Portfolio's country".
- [x] This computer's own settings — the import mappers (IMP-020), the price provider (MKT-214), the scheduled fetch (SPF), the agent connection (AGT-022)? — One per installation, shared by every profile (owner, 2026-11-08).
- [x] Move two accounts a transfer links together? — No: a profile is isolated; the transfer is cancelled first (owner, 2026-11-08).
- [ ] Every figure and list across accounts read within one profile — the profile in use unless another is given (owner, 2026-10-17 and 2026-11-08): the rules land with their implementation (#766 slice B), with the account lookups by name of the command line (CLI-011) and the agent (AGT-043) and whether an import into an account of another profile is refused (IMP-010).
