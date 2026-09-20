//! Header / manifest / segment serialization (D8): the header is plaintext JSON (SYN-050); the
//! manifest and segment are sealed JSON, prefixed with a cleartext `data_format_version` so a
//! too-new file is detected without the key (SYN-035).
//!
//! Sealed file layout: `[data_format_version: u32 big-endian][nonce ‖ ciphertext]`.

use crate::context::sync::domain::{FolderHeader, FolderProblem, Manifest, Segment};
use crate::context::sync::error::SyncError;
use crate::context::sync::infrastructure::crypto::{open, seal, Key};
use crate::core::logger::BACKEND;

/// The data format this build writes and reads (SYN-035). Bumped whenever the written form of
/// what is synced changes: `tests/sync_format/v{N}.json` pins that form per version, and the
/// test suite fails while this build writes anything else (SYN-038).
pub const DATA_FORMAT_VERSION: u32 = 1;

/// Width of the cleartext `data_format_version` prefix, in bytes.
const VERSION_PREFIX_LENGTH: usize = 4;

/// A value that could not be serialized cannot be published: `PublishFailed { IoFailure }`,
/// never an empty placeholder written into the folder.
fn encode_failed(context: &'static str, error: serde_json::Error) -> SyncError {
    tracing::error!(target: BACKEND, err = %error, "{context}");
    SyncError::PublishFailed {
        problem: FolderProblem::IoFailure,
    }
}

/// Encodes the folder header as plaintext JSON (SYN-050 — the only readable file).
pub fn encode_header(header: &FolderHeader) -> Result<Vec<u8>, SyncError> {
    serde_json::to_vec(header)
        .map_err(|error| encode_failed("encode_header: serialization failed", error))
}

/// Decodes the folder header's plaintext JSON.
pub fn decode_header(bytes: &[u8]) -> Result<FolderHeader, SyncError> {
    serde_json::from_slice(bytes).map_err(|error| {
        tracing::error!(target: BACKEND, err = %error, "decode_header: malformed header");
        SyncError::DatabaseError
    })
}

/// Reads only the header's `data_format_version`, tolerating every other field being
/// unreadable — a header written by a newer build may carry fields this build does not know
/// (SYN-035). `None` when the bytes carry no such field.
pub fn header_data_format_version(bytes: &[u8]) -> Option<u32> {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()?
        .get("data_format_version")?
        .as_u64()
        .and_then(|version| u32::try_from(version).ok())
}

/// Seals a manifest under `key`, prefixed with the cleartext `data_format_version` (SYN-035).
pub fn encode_manifest(key: &Key, manifest: &Manifest) -> Result<Vec<u8>, SyncError> {
    encode_sealed(key, manifest.data_format_version, manifest)
}

/// Decodes a manifest sealed by `encode_manifest`.
pub fn decode_manifest(key: &Key, bytes: &[u8]) -> Result<Manifest, SyncError> {
    decode_sealed(key, bytes)
}

/// Seals a segment under `key`, prefixed with the cleartext `data_format_version` (SYN-035).
pub fn encode_segment(key: &Key, segment: &Segment) -> Result<Vec<u8>, SyncError> {
    encode_sealed(key, segment.data_format_version, segment)
}

/// Decodes a segment sealed by `encode_segment`.
pub fn decode_segment(key: &Key, bytes: &[u8]) -> Result<Segment, SyncError> {
    decode_sealed(key, bytes)
}

/// Reads the cleartext `data_format_version` prefix of a manifest or segment file without
/// needing the key (SYN-035) — what lets a too-new file be recognised before decryption is
/// even attempted.
pub fn peek_data_format_version(bytes: &[u8]) -> Result<u32, SyncError> {
    let prefix: [u8; VERSION_PREFIX_LENGTH] = bytes
        .get(..VERSION_PREFIX_LENGTH)
        .and_then(|prefix| prefix.try_into().ok())
        .ok_or(SyncError::DatabaseError)?;
    Ok(u32::from_be_bytes(prefix))
}

/// Rejects a file whose `data_format_version` is newer than `DATA_FORMAT_VERSION` (SYN-035).
pub fn ensure_supported_data_format_version(data_format_version: u32) -> Result<(), SyncError> {
    if data_format_version > DATA_FORMAT_VERSION {
        return Err(SyncError::UpdateRequired {
            data_format_version,
        });
    }
    Ok(())
}

fn encode_sealed<T: serde::Serialize>(
    key: &Key,
    data_format_version: u32,
    value: &T,
) -> Result<Vec<u8>, SyncError> {
    let plaintext = serde_json::to_vec(value)
        .map_err(|error| encode_failed("encode_sealed: serialization failed", error))?;
    let mut bytes = data_format_version.to_be_bytes().to_vec();
    bytes.extend(seal(key, &plaintext));
    Ok(bytes)
}

fn decode_sealed<T: serde::de::DeserializeOwned>(key: &Key, bytes: &[u8]) -> Result<T, SyncError> {
    ensure_supported_data_format_version(peek_data_format_version(bytes)?)?;
    let sealed = bytes
        .get(VERSION_PREFIX_LENGTH..)
        .ok_or(SyncError::DatabaseError)?;
    let plaintext = open(key, sealed)?;
    serde_json::from_slice(&plaintext).map_err(|error| {
        tracing::error!(target: BACKEND, err = %error, "decode_sealed: malformed payload");
        SyncError::DatabaseError
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::domain::{DerivationParameters, SegmentChange, APP_VERSION};
    use crate::context::sync::infrastructure::crypto::derive_key;
    use crate::shared::domain::{Operation, Origin, RecordKind};

    fn key() -> Key {
        let params = DerivationParameters {
            salt: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            memory_cost_kib: 19_456,
            iterations: 2,
            parallelism: 1,
        };
        derive_key("correct horse battery staple", &params).expect("valid parameters")
    }

    fn sample_header() -> FolderHeader {
        FolderHeader {
            derivation_parameters: DerivationParameters {
                salt: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                memory_cost_kib: 19_456,
                iterations: 2,
                parallelism: 1,
            },
            passphrase_check: vec![9, 9, 9],
            data_format_version: DATA_FORMAT_VERSION,
            created_at: "00000000000000000001".into(),
            created_by_device_id: "desktop-device".into(),
        }
    }

    fn sample_manifest() -> Manifest {
        Manifest {
            device_id: "desktop-device".into(),
            device_name: "Desktop".into(),
            data_format_version: DATA_FORMAT_VERSION,
            app_version: Some(APP_VERSION.to_string()),
            latest_sequence: 3,
        }
    }

    fn sample_segment() -> Segment {
        Segment {
            device_id: "desktop-device".into(),
            first_sequence: 1,
            last_sequence: 1,
            data_format_version: DATA_FORMAT_VERSION,
            changes: vec![SegmentChange {
                sequence: 1,
                logical_timestamp: "00000000000000000001".into(),
                based_on: None,
                record_kind: RecordKind::Account,
                record_identity: "account-1".into(),
                operation: Operation::Created,
                origin: Origin::User,
                content: Some("{\"id\":\"account-1\"}".into()),
            }],
        }
    }

    // SYN-050 — the header round-trips through plaintext JSON.
    #[test]
    fn header_round_trips_through_plaintext_encoding() {
        let header = sample_header();
        let bytes = encode_header(&header).expect("a valid header encodes");
        let decoded = decode_header(&bytes).expect("valid header must decode");
        assert_eq!(decoded, header);
    }

    // SYN-035 — the header's data format version is readable even when the rest of the
    // header is not this build's shape.
    #[test]
    fn header_data_format_version_reads_a_foreign_header() {
        assert_eq!(
            header_data_format_version(b"{\"data_format_version\":99,\"unknown\":true}"),
            Some(99)
        );
        assert_eq!(
            header_data_format_version(b"{\"passphrase_check\":\"x\"}"),
            None
        );
        assert_eq!(header_data_format_version(b"not json"), None);
    }

    // SYN-050 — the manifest round-trips through the sealed encoding.
    #[test]
    fn manifest_round_trips_through_sealed_encoding() {
        let key = key();
        let manifest = sample_manifest();
        let bytes = encode_manifest(&key, &manifest).expect("a valid manifest encodes");
        let decoded = decode_manifest(&key, &bytes).expect("valid manifest must decode");
        assert_eq!(decoded, manifest);
    }

    // SYN-050 — the segment round-trips through the sealed encoding.
    #[test]
    fn segment_round_trips_through_sealed_encoding() {
        let key = key();
        let segment = sample_segment();
        let bytes = encode_segment(&key, &segment).expect("a valid segment encodes");
        let decoded = decode_segment(&key, &bytes).expect("valid segment must decode");
        assert_eq!(decoded, segment);
    }

    // SYN-035 — the data format version can be read without the key.
    #[test]
    fn peek_data_format_version_reads_the_cleartext_prefix_without_a_key() {
        let manifest = sample_manifest();
        let bytes = encode_manifest(&key(), &manifest).expect("a valid manifest encodes");
        let version = peek_data_format_version(&bytes).expect("prefix must be readable");
        assert_eq!(version, DATA_FORMAT_VERSION);
    }

    // SYN-035 — a data format newer than this build's is rejected with UpdateRequired.
    #[test]
    fn ensure_supported_data_format_version_rejects_a_newer_version() {
        let result = ensure_supported_data_format_version(DATA_FORMAT_VERSION + 1);
        assert!(matches!(
            result,
            Err(SyncError::UpdateRequired { data_format_version }) if data_format_version == DATA_FORMAT_VERSION + 1
        ));
    }

    // SYN-035 — this build's own version is accepted.
    #[test]
    fn ensure_supported_data_format_version_accepts_its_own_version() {
        assert!(ensure_supported_data_format_version(DATA_FORMAT_VERSION).is_ok());
    }

    // Tampered ciphertext fails to decode: flipping a byte inside a sealed manifest must not
    // silently decode into something else.
    #[test]
    fn decode_manifest_rejects_tampered_ciphertext() {
        let key = key();
        let mut bytes =
            encode_manifest(&key, &sample_manifest()).expect("a valid manifest encodes");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let result = decode_manifest(&key, &bytes);
        assert!(
            result.is_err(),
            "tampering must be detected, not silently decoded"
        );
    }

    /// The manifest as the release before `app_version` existed reads it.
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct ManifestBeforeAppVersion {
        device_id: String,
        device_name: String,
        data_format_version: u32,
        latest_sequence: i64,
    }

    // SYN-037 — the app version is an addition older builds ignore: a manifest written by
    // this build is still read by the release that does not know the field, so it needs no
    // data format bump.
    #[test]
    fn syn_037_a_build_that_does_not_know_the_app_version_still_reads_the_manifest() {
        let key = key();
        let bytes = encode_manifest(&key, &sample_manifest()).expect("a valid manifest encodes");
        let read: ManifestBeforeAppVersion =
            decode_sealed(&key, &bytes).expect("the older shape must still decode");
        assert_eq!(read.device_id, "desktop-device");
        assert_eq!(read.device_name, "Desktop");
        assert_eq!(read.data_format_version, DATA_FORMAT_VERSION);
        assert_eq!(read.latest_sequence, 3);
    }

    // SYN-037 — a manifest written before the field existed reads with no app version.
    #[test]
    fn syn_037_a_manifest_written_before_the_app_version_existed_reads_with_none() {
        let key = key();
        let older = ManifestBeforeAppVersion {
            device_id: "laptop-device".into(),
            device_name: "Laptop".into(),
            data_format_version: DATA_FORMAT_VERSION,
            latest_sequence: 7,
        };
        let bytes = encode_sealed(&key, DATA_FORMAT_VERSION, &older).expect("it encodes");
        let manifest = decode_manifest(&key, &bytes).expect("an older manifest must decode");
        assert_eq!(manifest.app_version, None);
        assert_eq!(manifest.latest_sequence, 7);
    }

    // SYN-037 — this build states its own version in the manifest it publishes.
    #[test]
    fn syn_037_a_manifest_round_trips_with_its_app_version() {
        let key = key();
        let bytes = encode_manifest(&key, &sample_manifest()).expect("a valid manifest encodes");
        let manifest = decode_manifest(&key, &bytes).expect("it decodes");
        assert_eq!(manifest.app_version.as_deref(), Some(APP_VERSION));
    }
}
