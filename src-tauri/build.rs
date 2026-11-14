use std::path::Path;

/// IMP-020F — every `.json` file of `presets/` is a built-in preset, compiled into the
/// program: adding one needs no code change.
fn compile_built_in_presets() {
    println!("cargo:rerun-if-changed=presets");
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("presets");
    // A build that cannot compile the presets fails: it never ships without them.
    let mut presets: Vec<(String, String)> = std::fs::read_dir(&folder)
        .expect("presets folder")
        .map(|entry| entry.expect("presets folder entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .map(|path| {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("a preset's file name is UTF-8")
                .to_string();
            (stem, path.display().to_string())
        })
        .collect();
    presets.sort();
    let entries: String = presets
        .iter()
        .map(|(stem, path)| format!("    ({stem:?}, include_bytes!({path:?})),\n"))
        .collect();
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("built_in_presets.rs");
    std::fs::write(out, format!("&[\n{entries}]")).expect("built-in presets written");
}

fn main() {
    compile_built_in_presets();
    // The Tauri build step runs only for the `app` shell; the core builds without it.
    #[cfg(feature = "app")]
    tauri_build::build()
}
