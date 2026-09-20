//! Shared kernel vocabulary for the multi-device sync change log (ADR-019, D1).
//!
//! Pure value objects — no sqlx, no I/O, no clock. `specta::Type` is derived only where the
//! contract will expose these on the wire (PR-B/PR-C); nothing here decides a merge outcome —
//! that is `context::sync::domain::resolution` alone (constraint 2, ADR-019).

use serde::{Deserialize, Serialize};
use specta::Type;

/// The ten synced record kinds (SYN-021). `holdings`, performance figures, and device-local
/// data (scheduled fetch config, window state, the sync configuration itself) are deliberately
/// absent (SYN-022/023).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Type,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum RecordKind {
    /// An account, identified by its own id.
    Account,
    /// An asset category, identified by its own id.
    Category,
    /// An asset, identified by its own id.
    Asset,
    /// A transaction, identified by its own id.
    Transaction,
    /// A management fee schedule, identified by (account_id, asset_id).
    FeeSchedule,
    /// A fee catch-up position, identified by (account_id, asset_id).
    FeeCatchUpPosition,
    /// An asset price, identified by (asset_id, date).
    AssetPrice,
    /// A currency pair, identified by (from_currency, to_currency).
    CurrencyPair,
    /// A currency rate, identified by (from_currency, to_currency, date).
    CurrencyRate,
    /// A holding note, identified by (account_id, asset_id).
    HoldingNote,
}

/// Whether the user made the change or the application generated it on its own (CFR-016).
/// `Application` is declared before `User` so the derived order agrees with CFR-016's "a user
/// change outranks every application change" — asserted directly by
/// `rank_orders_user_origin_above_application_regardless_of_timestamp` below.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Type,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum Origin {
    /// The application generated the change on its own (a fee deduction, an auto-fetched price).
    Application,
    /// The user made the change.
    User,
}

/// Whether the record was created, updated, or removed (SYN Entity Definition — Change).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Type,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum Operation {
    /// The record came into existence.
    Created,
    /// The record's content changed.
    Updated,
    /// The record was removed.
    Removed,
}

/// The per-change ordering value (CFR-010): a Lamport counter serialized as a zero-padded
/// 20-character decimal string so lexicographic order equals numeric order (D6). Backed by
/// `sync_device.logical_clock` (INTEGER); this type is its wire/comparison form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
pub struct LogicalTimestamp(String);

impl LogicalTimestamp {
    /// Builds the zero-padded 20-character decimal wire form of `value` (CFR-010, D6). Pure
    /// encoding only — deciding what the *next* value to advance to is belongs to the
    /// `SqliteChangeRecorder`'s Lamport clock (SYN-025), not to this type.
    pub fn new(value: u64) -> Self {
        Self(format!("{value:020}"))
    }

    /// The value one greater than the counter this timestamp encodes. Pure arithmetic on the
    /// encoded value; it is the recorder's job (SYN-025), not this type's, to ensure the
    /// counter it advances is never behind every change the device had recorded or applied
    /// before it (CFR-010).
    pub fn next(&self) -> Self {
        let value: u64 = self.0.parse().unwrap_or(0);
        Self::new(value + 1)
    }

    /// Reads a wire-form timestamp back (a received change's `logical_timestamp` or
    /// `based_on`); `None` when the text is not a decimal counter.
    pub fn from_wire(text: &str) -> Option<Self> {
        text.parse::<u64>().ok().map(Self::new)
    }

    /// The Lamport counter this timestamp encodes; 0 when the wire form is not a counter.
    pub fn value(&self) -> u64 {
        self.0.parse().unwrap_or(0)
    }

    /// The zero-padded 20-character wire form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Which record changed — the record's own identity, as defined per kind in CFR-012. The wire
/// form is the canonical string built by `canonical()`; identity here means identity *between
/// devices* — whatever key a device stores a record under locally is its own business.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct RecordIdentity(String);

impl RecordIdentity {
    /// Builds the canonical cross-device identity string for `kind` from its natural key
    /// (CFR-012): account/category/asset/transaction by their own id; fee schedules and fee
    /// catch-up positions by (account_id, asset_id, CFR-034); currency pairs by
    /// (from_currency, to_currency); asset prices by (asset_id, date); currency rates by
    /// (from_currency, to_currency, date); holding notes by (account_id, asset_id).
    pub fn canonical(_kind: RecordKind, key: &[&str]) -> Self {
        Self(key.join(":"))
    }

    /// The canonical wire-form string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The total order CFR-020 compares: origin first (`User` above `Application`, CFR-016), then
/// logical timestamp (CFR-010), then the identity of the device that made the change (CFR-010's
/// tie-break). Totally ordered — any two ranks differ (CFR-020) — and the comparison is the
/// same on every device however changes arrive (CFR-013).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Rank {
    /// Who made the change (CFR-016) — compared first.
    pub origin: Origin,
    /// When, on the Lamport clock (CFR-010) — compared second.
    pub logical_timestamp: LogicalTimestamp,
    /// Which device made the change — the final tie-break.
    pub device_id: String,
}

impl Rank {
    /// The NULL sentinel (D6): a record that has never been ranked. Modeled as `None` so it
    /// compares below every real rank via `Option<Rank>`'s derived order — matching the
    /// nullable `sync_logical_timestamp` / `sync_origin` / `sync_device_id` columns (M1).
    pub const NEVER: Option<Rank> = None;
}

impl PartialOrd for Rank {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rank {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.origin
            .cmp(&other.origin)
            .then_with(|| self.logical_timestamp.cmp(&other.logical_timestamp))
            .then_with(|| self.device_id.cmp(&other.device_id))
    }
}

/// A synced record as this device currently holds it (CFR-014): the rank of the change that
/// produced it — `None` for a row that has never been ranked (D6) — and its full state,
/// JSON-encoded exactly as the owning repository's change capture serializes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedRecord {
    /// The rank of the record's current state, or the NULL sentinel.
    pub rank: Option<Rank>,
    /// The record's full state, JSON-encoded.
    pub content: String,
}

/// One child record an account owns (CFR-030): its kind, its cross-device identity, and the
/// rank of its current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedChild {
    /// The child's kind.
    pub record_kind: RecordKind,
    /// The child's canonical identity (CFR-012).
    pub record_identity: String,
    /// The rank of the child's current state, or the NULL sentinel.
    pub rank: Option<Rank>,
}

/// The not-yet-recorded shape of one change (SYN Entity Definition — Change), built by a
/// repository write and handed to `ChangeRecorder::record`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDraft {
    /// Which kind of record changed (SYN-021).
    pub record_kind: RecordKind,
    /// Which record changed (CFR-012).
    pub record_identity: RecordIdentity,
    /// Created, Updated, or Removed.
    pub operation: Operation,
    /// Who made the change (CFR-016).
    pub origin: Origin,
    /// The logical timestamp of the record state this change was made against; absent for a
    /// creation (CFR-011).
    pub based_on: Option<LogicalTimestamp>,
    /// The full state of the record after the change, JSON-encoded; absent for a removal.
    pub content: Option<String>,
}

impl ChangeDraft {
    /// Builds the draft from its fields, unchanged.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        record_kind: RecordKind,
        record_identity: RecordIdentity,
        operation: Operation,
        origin: Origin,
        based_on: Option<LogicalTimestamp>,
        content: Option<String>,
    ) -> Self {
        Self {
            record_kind,
            record_identity,
            operation,
            origin,
            based_on,
            content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SYN-021 — exactly the ten synced record kinds, pairwise distinct.
    #[test]
    fn record_kind_has_exactly_ten_pairwise_distinct_synced_kinds() {
        let kinds = [
            RecordKind::Account,
            RecordKind::Category,
            RecordKind::Asset,
            RecordKind::Transaction,
            RecordKind::FeeSchedule,
            RecordKind::FeeCatchUpPosition,
            RecordKind::AssetPrice,
            RecordKind::CurrencyPair,
            RecordKind::CurrencyRate,
            RecordKind::HoldingNote,
        ];
        let unique: std::collections::HashSet<_> = kinds.iter().collect();
        assert_eq!(
            unique.len(),
            10,
            "SYN-021 lists exactly ten synced record kinds"
        );
    }

    // SYN Entity Definition — Change.operation — exactly Created / Updated / Removed.
    #[test]
    fn operation_has_created_updated_removed() {
        let ops = [Operation::Created, Operation::Updated, Operation::Removed];
        let unique: std::collections::HashSet<_> = ops.iter().map(|o| format!("{o:?}")).collect();
        assert_eq!(unique.len(), 3);
    }

    // CFR-010/D6 — the logical timestamp wire form is a zero-padded 20-char decimal string.
    #[test]
    fn logical_timestamp_wire_form_is_zero_padded_twenty_chars() {
        let ts = LogicalTimestamp::new(42);
        assert_eq!(ts.as_str().len(), 20);
        assert_eq!(ts.as_str(), "00000000000000000042");
    }

    // CFR-010 — lexicographic order over the wire form equals numeric order.
    #[test]
    fn logical_timestamp_lexicographic_order_equals_numeric_order() {
        let earlier = LogicalTimestamp::new(9);
        let later = LogicalTimestamp::new(10);
        assert!(
            earlier.as_str() < later.as_str(),
            "lexicographic comparison of the zero-padded wire strings"
        );
        assert!(earlier < later, "the typed comparison must agree");
    }

    // CFR-010 — next() is always strictly greater, regardless of wall-clock drift (SYN-025).
    #[test]
    fn logical_timestamp_next_is_strictly_greater() {
        let current = LogicalTimestamp::new(1_000);
        let advanced = current.next();
        assert!(
            advanced > current,
            "next() must produce a strictly greater timestamp"
        );
    }

    // CFR-012 — accounts/categories/assets/transactions are identified by their own id.
    #[test]
    fn record_identity_canonical_by_own_id_for_account_category_asset_transaction() {
        let id = RecordIdentity::canonical(RecordKind::Account, &["account-1"]);
        assert_eq!(id.as_str(), "account-1");
    }

    // CFR-012/CFR-034 — fee schedules take their identity from (account_id, asset_id).
    #[test]
    fn record_identity_canonical_fee_schedule_by_account_and_asset() {
        let id = RecordIdentity::canonical(RecordKind::FeeSchedule, &["acc-1", "asset-1"]);
        assert_eq!(id.as_str(), "acc-1:asset-1");
    }

    // CFR-044 — a fee catch-up position takes its identity from the schedule's
    // (account_id, asset_id), same as the schedule itself.
    #[test]
    fn record_identity_canonical_fee_catch_up_position_by_account_and_asset() {
        let id = RecordIdentity::canonical(RecordKind::FeeCatchUpPosition, &["acc-1", "asset-1"]);
        assert_eq!(id.as_str(), "acc-1:asset-1");
    }

    // CFR-012/CFR-034 — a currency pair is identified by its two currencies.
    #[test]
    fn record_identity_canonical_currency_pair_by_from_and_to() {
        let id = RecordIdentity::canonical(RecordKind::CurrencyPair, &["USD", "EUR"]);
        assert_eq!(id.as_str(), "USD:EUR");
    }

    // CFR-012 — an asset price is identified by asset and observed date.
    #[test]
    fn record_identity_canonical_asset_price_by_asset_and_date() {
        let id = RecordIdentity::canonical(RecordKind::AssetPrice, &["asset-1", "2026-08-20"]);
        assert_eq!(id.as_str(), "asset-1:2026-08-20");
    }

    // CFR-012 — a currency rate is identified by pair and observed date.
    #[test]
    fn record_identity_canonical_currency_rate_by_pair_and_date() {
        let id = RecordIdentity::canonical(RecordKind::CurrencyRate, &["USD", "EUR", "2026-08-20"]);
        assert_eq!(id.as_str(), "USD:EUR:2026-08-20");
    }

    // CFR-012 — a holding note is identified by (account_id, asset_id).
    #[test]
    fn record_identity_canonical_holding_note_by_account_and_asset() {
        let id = RecordIdentity::canonical(RecordKind::HoldingNote, &["acc-1", "asset-1"]);
        assert_eq!(id.as_str(), "acc-1:asset-1");
    }

    // CFR-016/CFR-020 — origin is the first rank component: a user rank always outranks an
    // application rank, whatever their timestamps.
    #[test]
    fn rank_orders_user_origin_above_application_regardless_of_timestamp() {
        let user_rank = Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(1),
            device_id: "desktop".into(),
        };
        let application_rank = Rank {
            origin: Origin::Application,
            logical_timestamp: LogicalTimestamp::new(1_000_000),
            device_id: "laptop".into(),
        };
        assert!(
            user_rank > application_rank,
            "CFR-016: user beats application whatever the timestamp"
        );
    }

    // CFR-010 — same origin: the greater logical timestamp prevails.
    #[test]
    fn rank_orders_by_logical_timestamp_when_origin_is_equal() {
        let earlier = Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(1_000),
            device_id: "desktop".into(),
        };
        let later = Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(1_050),
            device_id: "desktop".into(),
        };
        assert!(later > earlier);
    }

    // CFR-010 — equal origin and timestamp: the device identity is the tie-break (the
    // spec's scenario: two changes stamped exactly 1050, Laptop sorts after Desktop).
    #[test]
    fn rank_orders_by_device_identity_when_origin_and_timestamp_tie() {
        let desktop = Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(1_050),
            device_id: "desktop".into(),
        };
        let laptop = Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(1_050),
            device_id: "laptop".into(),
        };
        assert!(
            laptop > desktop,
            "CFR-010 scenario: laptop's identity sorts after desktop's"
        );
    }

    // D6 — the NULL sentinel sorts below every real rank.
    #[test]
    fn rank_never_sentinel_sorts_below_every_real_rank() {
        let real_rank = Some(Rank {
            origin: Origin::Application,
            logical_timestamp: LogicalTimestamp::new(1),
            device_id: "desktop".into(),
        });
        assert!(
            Rank::NEVER < real_rank,
            "D6: NULL is the never-ranked sentinel, below every real rank"
        );
    }

    // ChangeDraft — plain construction carries every field through unchanged.
    #[test]
    fn change_draft_construction_carries_every_field() {
        let draft = ChangeDraft::new(
            RecordKind::Account,
            RecordIdentity::canonical(RecordKind::Account, &["account-1"]),
            Operation::Updated,
            Origin::User,
            Some(LogicalTimestamp::new(10)),
            Some("{\"name\":\"CTO\"}".to_string()),
        );
        assert_eq!(draft.record_kind, RecordKind::Account);
        assert_eq!(draft.operation, Operation::Updated);
        assert_eq!(draft.origin, Origin::User);
        assert_eq!(draft.content.as_deref(), Some("{\"name\":\"CTO\"}"));
    }
}
