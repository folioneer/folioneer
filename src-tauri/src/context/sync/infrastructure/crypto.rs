//! Key derivation (Argon2id, SYN-051) + AEAD seal/open (XChaCha20-Poly1305, SYN-050) +
//! passphrase-check marker (SYN-055). D7: `argon2`/`password-hash` pinned to 0.5.x,
//! `chacha20poly1305` 0.11, `zeroize` wipes the passphrase and derived-key buffers (SYN-052).

use std::ops::RangeInclusive;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, Generate, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use zeroize::Zeroizing;

/// The 192-bit nonce XChaCha20-Poly1305 takes.
type XNonce = chacha20poly1305::aead::Nonce<XChaCha20Poly1305>;

use crate::context::sync::domain::DerivationParameters;
use crate::context::sync::error::SyncError;
use crate::core::logger::BACKEND;

/// The minimum passphrase length (SYN-012).
pub const MINIMUM_PASSPHRASE_LENGTH: usize = 12;

/// Argon2id memory cost this build uses for a new portfolio, in KiB (64 MiB).
const MEMORY_COST_KIB: u32 = 65_536;
/// Argon2id iteration count this build uses for a new portfolio.
const ITERATIONS: u32 = 3;
/// Argon2id parallelism this build uses for a new portfolio.
const PARALLELISM: u32 = 1;
/// Salt length for a new portfolio, in bytes — and the least a header may carry.
const SALT_LENGTH: usize = 16;
/// The Argon2id memory cost a header may ask for, in KiB (19 MiB ..= 1 GiB): less would weaken
/// the key, more could exhaust the deriving device.
const MEMORY_COST_KIB_RANGE: RangeInclusive<u32> = 19_456..=1_048_576;
/// The Argon2id iteration count a header may ask for.
const ITERATIONS_RANGE: RangeInclusive<u32> = 1..=16;
/// The Argon2id parallelism a header may ask for.
const PARALLELISM_RANGE: RangeInclusive<u32> = 1..=8;
/// XChaCha20-Poly1305 key length, in bytes.
const KEY_LENGTH: usize = 32;
/// XChaCha20-Poly1305 extended nonce length, in bytes — prefixed to every ciphertext.
const NONCE_LENGTH: usize = 24;
/// The fixed plaintext the passphrase-check marker seals (SYN-055). Part of the folder
/// format: changing it would make every existing folder's passphrase check fail.
const PASSPHRASE_CHECK_MARKER: &[u8] = b"vaultcompass-sync-passphrase-check";

/// The passphrase-derived encryption key. Zeroized on drop (SYN-052) — never `Debug`-printed,
/// never logged.
#[derive(Clone, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct Key(Vec<u8>);

impl Key {
    /// Restores a key kept on the device (`sync_device.derived_key`). Rejects bytes of any
    /// length other than the AEAD's key length — such a key can open nothing.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, SyncError> {
        if bytes.len() != KEY_LENGTH {
            return Err(SyncError::PassphraseMismatch);
        }
        Ok(Self(bytes))
    }

    /// The raw key bytes — used only to hand the key to the AEAD and to persist it.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

/// Rejects a passphrase shorter than `MINIMUM_PASSPHRASE_LENGTH` (SYN-012).
pub fn ensure_passphrase_length(passphrase: &str) -> Result<(), SyncError> {
    if passphrase.chars().count() < MINIMUM_PASSPHRASE_LENGTH {
        return Err(SyncError::PassphraseTooShort {
            minimum: MINIMUM_PASSPHRASE_LENGTH as u32,
        });
    }
    Ok(())
}

/// Generates fresh `DerivationParameters` (a random salt + this build's cost settings) for a
/// brand-new portfolio (SYN-051, first publish / start-over).
pub fn generate_derivation_parameters() -> DerivationParameters {
    DerivationParameters {
        salt: <[u8; SALT_LENGTH]>::generate().to_vec(),
        memory_cost_kib: MEMORY_COST_KIB,
        iterations: ITERATIONS,
        parallelism: PARALLELISM,
    }
}

/// Rejects derivation parameters outside the range this build runs with (SYN-051): a header
/// asking for less would weaken the key, one asking for more could exhaust this device's
/// memory or time — either way the header is `HeaderRejected`, never derived against.
pub fn ensure_derivation_parameters(params: &DerivationParameters) -> Result<(), SyncError> {
    let within_bounds = MEMORY_COST_KIB_RANGE.contains(&params.memory_cost_kib)
        && ITERATIONS_RANGE.contains(&params.iterations)
        && PARALLELISM_RANGE.contains(&params.parallelism)
        && params.salt.len() >= SALT_LENGTH;
    if !within_bounds {
        tracing::warn!(
            target: BACKEND,
            memory_cost_kib = params.memory_cost_kib,
            iterations = params.iterations,
            parallelism = params.parallelism,
            salt_length = params.salt.len(),
            "derivation parameters out of bounds"
        );
        return Err(SyncError::HeaderRejected);
    }
    Ok(())
}

/// Derives the shared encryption key from `passphrase` and the header's public
/// `DerivationParameters` (SYN-051) — the same passphrase and parameters always yield the same
/// key, on every device. Parameters outside this build's bounds, or ones Argon2id cannot run
/// with, are `HeaderRejected` — such a header cannot belong to a portfolio this device can
/// open.
pub fn derive_key(passphrase: &str, params: &DerivationParameters) -> Result<Key, SyncError> {
    ensure_derivation_parameters(params)?;
    let argon2_params = Params::new(
        params.memory_cost_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LENGTH),
    )
    .map_err(|error| {
        tracing::error!(target: BACKEND, err = %error, "derive_key: unusable derivation parameters");
        SyncError::HeaderRejected
    })?;
    let mut key = zeroize::Zeroizing::new(vec![0u8; KEY_LENGTH]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params)
        .hash_password_into(passphrase.as_bytes(), &params.salt, &mut key)
        .map_err(|error| {
            tracing::error!(target: BACKEND, err = %error, "derive_key: key derivation failed");
            SyncError::PassphraseMismatch
        })?;
    Ok(Key(key.to_vec()))
}

/// `derive_key` off the async runtime — Argon2id is deliberately slow, and a tokio worker
/// must not stall for its duration. The passphrase buffer is wiped inside the worker once the
/// key is derived (SYN-052).
pub async fn derive_key_blocking(
    passphrase: Zeroizing<String>,
    params: DerivationParameters,
) -> Result<Key, SyncError> {
    tokio::task::spawn_blocking(move || derive_key(passphrase.as_str(), &params))
        .await
        .unwrap_or_else(|error| {
            tracing::error!(target: BACKEND, err = %error, "derive_key_blocking: worker failed");
            Err(SyncError::DatabaseError)
        })
}

/// Seals `plaintext` under `key` with a fresh random nonce (SYN-050). Two calls with the same
/// plaintext produce different ciphertext (XChaCha20-Poly1305's 192-bit nonce makes a random
/// per-message nonce safe without a cross-device counter, D7). The nonce is prefixed to the
/// ciphertext.
pub fn seal(key: &Key, plaintext: &[u8]) -> Vec<u8> {
    let nonce = XNonce::generate();
    let Ok(cipher) = XChaCha20Poly1305::new_from_slice(key.as_bytes()) else {
        tracing::error!(target: BACKEND, "seal: key has an invalid length");
        return Vec::new();
    };
    let Ok(ciphertext) = cipher.encrypt(&nonce, plaintext) else {
        tracing::error!(target: BACKEND, "seal: encryption failed");
        return Vec::new();
    };
    let mut sealed = Vec::with_capacity(NONCE_LENGTH + ciphertext.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    sealed
}

/// Opens ciphertext produced by `seal`. Fails on a wrong key or tampered ciphertext.
pub fn open(key: &Key, ciphertext: &[u8]) -> Result<Vec<u8>, SyncError> {
    let (nonce, body) = ciphertext
        .split_at_checked(NONCE_LENGTH)
        .ok_or(SyncError::PassphraseMismatch)?;
    let nonce = XNonce::try_from(nonce).map_err(|_| SyncError::PassphraseMismatch)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|_| SyncError::PassphraseMismatch)?;
    cipher
        .decrypt(&nonce, body)
        .map_err(|_| SyncError::PassphraseMismatch)
}

/// Builds the folder header's `passphrase_check` marker (SYN-055): an encrypted value that
/// decrypts correctly only with the right key.
pub fn make_check(key: &Key) -> Vec<u8> {
    seal(key, PASSPHRASE_CHECK_MARKER)
}

/// Verifies the passphrase check before reading or publishing anything else (SYN-015/055).
pub fn verify_check(key: &Key, marker: &[u8]) -> bool {
    open(key, marker).is_ok_and(|plaintext| plaintext == PASSPHRASE_CHECK_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> DerivationParameters {
        DerivationParameters {
            salt: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            memory_cost_kib: 19_456,
            iterations: 2,
            parallelism: 1,
        }
    }

    fn key_for(passphrase: &str, params: &DerivationParameters) -> Key {
        derive_key(passphrase, params).expect("valid parameters must derive a key")
    }

    // SYN-012 — a passphrase below the floor is rejected.
    #[test]
    fn ensure_passphrase_length_rejects_below_minimum() {
        let result = ensure_passphrase_length("short-pw11");
        assert!(matches!(
            result,
            Err(SyncError::PassphraseTooShort { minimum: 12 })
        ));
    }

    // SYN-012 — exactly the minimum length is accepted.
    #[test]
    fn ensure_passphrase_length_accepts_exactly_the_minimum() {
        let twelve_chars = "a".repeat(MINIMUM_PASSPHRASE_LENGTH);
        assert!(ensure_passphrase_length(&twelve_chars).is_ok());
    }

    // SYN-051 — the same passphrase and parameters always derive the same key.
    #[test]
    fn derive_key_same_inputs_produce_the_same_key() {
        let params = params();
        let first = key_for("correct horse battery staple", &params);
        let second = key_for("correct horse battery staple", &params);
        assert!(first == second);
    }

    // SYN-051 — a different salt derives a different key from the same passphrase.
    #[test]
    fn derive_key_different_salt_produces_a_different_key() {
        let mut other_params = params();
        other_params.salt = vec![9; 16];
        let first = key_for("correct horse battery staple", &params());
        let second = key_for("correct horse battery staple", &other_params);
        assert!(first != second);
    }

    // SYN-051 — parameters Argon2id cannot run with are rejected, never silently replaced.
    #[test]
    fn derive_key_rejects_a_salt_shorter_than_argon2_allows() {
        let mut short_salt = params();
        short_salt.salt = vec![1, 2, 3, 4];
        let result = derive_key("correct horse battery staple", &short_salt);
        assert!(result.is_err());
    }

    // SYN-051 — a salt of fifteen bytes is below the floor; sixteen is accepted.
    #[test]
    fn ensure_derivation_parameters_bounds_the_salt_length() {
        let mut fifteen = params();
        fifteen.salt = vec![7; 15];
        assert!(matches!(
            ensure_derivation_parameters(&fifteen),
            Err(SyncError::HeaderRejected)
        ));
        let mut sixteen = params();
        sixteen.salt = vec![7; 16];
        assert!(ensure_derivation_parameters(&sixteen).is_ok());
    }

    // SYN-051 — memory cost below 19 MiB or above 1 GiB is rejected; both bounds are accepted.
    #[test]
    fn ensure_derivation_parameters_bounds_the_memory_cost() {
        for memory_cost_kib in [19_455, 1_048_577] {
            let mut out_of_bounds = params();
            out_of_bounds.memory_cost_kib = memory_cost_kib;
            assert!(
                matches!(
                    ensure_derivation_parameters(&out_of_bounds),
                    Err(SyncError::HeaderRejected)
                ),
                "memory cost {memory_cost_kib} KiB must be rejected"
            );
        }
        for memory_cost_kib in [19_456, 1_048_576] {
            let mut at_bound = params();
            at_bound.memory_cost_kib = memory_cost_kib;
            assert!(ensure_derivation_parameters(&at_bound).is_ok());
        }
    }

    // SYN-051 — iterations outside 1..=16 are rejected; both bounds are accepted.
    #[test]
    fn ensure_derivation_parameters_bounds_the_iterations() {
        for iterations in [0, 17] {
            let mut out_of_bounds = params();
            out_of_bounds.iterations = iterations;
            assert!(
                matches!(
                    ensure_derivation_parameters(&out_of_bounds),
                    Err(SyncError::HeaderRejected)
                ),
                "{iterations} iterations must be rejected"
            );
        }
        for iterations in [1, 16] {
            let mut at_bound = params();
            at_bound.iterations = iterations;
            assert!(ensure_derivation_parameters(&at_bound).is_ok());
        }
    }

    // SYN-051 — parallelism outside 1..=8 is rejected; both bounds are accepted.
    #[test]
    fn ensure_derivation_parameters_bounds_the_parallelism() {
        for parallelism in [0, 9] {
            let mut out_of_bounds = params();
            out_of_bounds.parallelism = parallelism;
            assert!(
                matches!(
                    ensure_derivation_parameters(&out_of_bounds),
                    Err(SyncError::HeaderRejected)
                ),
                "parallelism {parallelism} must be rejected"
            );
        }
        for parallelism in [1, 8] {
            let mut at_bound = params();
            at_bound.parallelism = parallelism;
            assert!(ensure_derivation_parameters(&at_bound).is_ok());
        }
    }

    // SYN-051 — derive_key rejects an out-of-bounds header before hashing, as HeaderRejected.
    #[test]
    fn derive_key_rejects_out_of_bounds_parameters_as_header_rejected() {
        let mut hostile = params();
        hostile.memory_cost_kib = 4_194_304;
        let result = derive_key("correct horse battery staple", &hostile);
        assert!(matches!(result, Err(SyncError::HeaderRejected)));
    }

    // SYN-051/052 — the off-runtime derivation yields the same key as the direct one.
    #[tokio::test]
    async fn derive_key_blocking_matches_derive_key() {
        let params = params();
        let direct = key_for("correct horse battery staple", &params);
        let off_runtime = derive_key_blocking(
            Zeroizing::new("correct horse battery staple".to_string()),
            params,
        )
        .await
        .expect("valid parameters must derive a key");
        assert!(direct == off_runtime);
    }

    // SYN-051 — freshly generated parameters derive a key.
    #[test]
    fn generate_derivation_parameters_are_usable() {
        let params = generate_derivation_parameters();
        assert_eq!(params.salt.len(), SALT_LENGTH);
        assert!(derive_key("correct horse battery staple", &params).is_ok());
    }

    // SYN-050 — seal then open recovers the original plaintext.
    #[test]
    fn seal_then_open_round_trips_the_plaintext() {
        let key = key_for("correct horse battery staple", &params());
        let plaintext = b"segment payload".to_vec();
        let ciphertext = seal(&key, &plaintext);
        let opened = open(&key, &ciphertext).expect("seal/open round trip must succeed");
        assert_eq!(opened, plaintext);
    }

    // D7 — two seals of the same plaintext produce different ciphertext (random per-message
    // nonce, no cross-device counter).
    #[test]
    fn seal_produces_different_ciphertext_for_the_same_plaintext_twice() {
        let key = key_for("correct horse battery staple", &params());
        let plaintext = b"segment payload".to_vec();
        let first = seal(&key, &plaintext);
        let second = seal(&key, &plaintext);
        assert_ne!(first, second);
    }

    // SYN-050 — opening with the wrong key fails.
    #[test]
    fn open_with_wrong_key_fails() {
        let key = key_for("correct horse battery staple", &params());
        let wrong_key = key_for("a different passphrase entirely", &params());
        let ciphertext = seal(&key, b"segment payload");
        let result = open(&wrong_key, &ciphertext);
        assert!(result.is_err());
    }

    // SYN-055 — the passphrase-check marker verifies true with the same key it was made under.
    #[test]
    fn make_check_verifies_true_with_the_same_key() {
        let key = key_for("correct horse battery staple", &params());
        let marker = make_check(&key);
        assert!(verify_check(&key, &marker));
    }

    // SYN-015/055 — the passphrase-check marker verifies false with a different key.
    #[test]
    fn verify_check_fails_with_a_different_key() {
        let key = key_for("correct horse battery staple", &params());
        let wrong_key = key_for("a different passphrase entirely", &params());
        let marker = make_check(&key);
        assert!(!verify_check(&wrong_key, &marker));
    }

    // SYN-052 — a kept key of the wrong length is refused at restore time; it can open nothing.
    #[test]
    fn key_from_bytes_rejects_a_wrong_length() {
        assert!(Key::from_bytes(vec![0]).is_err());
        assert!(Key::from_bytes(vec![0; KEY_LENGTH]).is_ok());
    }
}
