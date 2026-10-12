fn main() {
    #[cfg(feature = "desktop")]
    {
        // Recompile the PE icon resources when the brand artwork changes.
        println!("cargo:rerun-if-changed=icons/icon.ico");
        tauri_build::build();
    }
}
