//! How two names a user typed are told apart — accounts, profiles and categories.

/// Whether two names are one name: the case is ignored for every letter, an accented one
/// included ("épargne" is "ÉPARGNE"). SQLite's `LOWER()` folds ASCII letters only, so a
/// uniqueness check compares here, not in a query (ACC-003, PFL-020, CAT-001).
pub fn same_name(one: &str, other: &str) -> bool {
    one.to_lowercase() == other.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::same_name;

    #[test]
    fn the_case_is_ignored_for_every_letter() {
        assert!(same_name("Épargne", "éPARGNE"));
        assert!(same_name("Alpha", "ALPHA"));
        assert!(!same_name("Epargne", "Épargne"));
    }

    // ACC-003 — compared letter by letter as typed.
    #[test]
    fn two_letters_for_one_and_a_combining_accent_are_other_names() {
        assert!(!same_name("Straße", "STRASSE"));
        assert!(!same_name("\u{c9}pargne", "E\u{301}pargne"));
    }
}
