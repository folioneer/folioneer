fn main() {
    // The Tauri build step runs only for the `app` shell; the core builds without it.
    #[cfg(feature = "app")]
    tauri_build::build()
}
