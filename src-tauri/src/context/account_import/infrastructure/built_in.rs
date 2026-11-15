//! IMP-020F — the built-in presets, compiled into the program from `presets/` (build.rs).

/// Each built-in preset: its file's name without `.json`, and its bytes.
pub fn built_in_presets() -> Vec<(String, Vec<u8>)> {
    const PRESETS: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/built_in_presets.rs"));
    PRESETS
        .iter()
        .map(|(stem, bytes)| ((*stem).to_string(), bytes.to_vec()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account_import::domain::{ImportMapper, ImportMapperDraft};

    // IMP-020F — every preset of `presets/` is compiled in and reads as a mapper.
    #[test]
    fn imp_020f_every_built_in_preset_reads_as_a_mapper() {
        let presets = built_in_presets();
        assert!(presets
            .iter()
            .any(|(stem, _)| stem == "interactive-brokers-trades"));
        let mut providers: Vec<String> = Vec::new();
        for (stem, bytes) in presets {
            let draft = ImportMapperDraft::from_file(&bytes).expect(&stem);
            let mapper = ImportMapper::with_id(
                crate::context::account_import::domain::built_in_id(&stem),
                draft,
            )
            .expect(&stem);
            // IMP-024 — each preset has a provider of its own.
            assert!(
                !providers.iter().any(|held| mapper.has_provider(held)),
                "{stem}: provider held twice"
            );
            providers.push(mapper.info.provider);
        }
    }
}
