use std::{env, fs, path::PathBuf};

fn main() {
    write_legacy_metadata_without_command_attrs();
    tauri_build::build()
}

fn write_legacy_metadata_without_command_attrs() {
    println!("cargo:rerun-if-changed=src/metadata.rs");

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by Cargo"),
    );
    let source_path = manifest_dir.join("src").join("metadata.rs");
    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set by Cargo"))
        .join("metadata_without_commands.rs");

    let source = fs::read_to_string(&source_path)
        .expect("failed to read legacy metadata module")
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let command_functions = [
        "get_metadata_providers",
        "fetch_metadata",
        "search_metadata",
        "check_media_item_metadata",
        "get_provider_status",
        "test_api_key",
        "set_api_key",
        "get_api_keys",
    ];

    let mut sanitized = source;
    let mut stripped = 0usize;
    for function_name in command_functions {
        for prefix in ["pub fn", "pub async fn"] {
            let needle = format!("#[tauri::command]\n{prefix} {function_name}");
            if sanitized.contains(&needle) {
                sanitized = sanitized.replace(&needle, &format!("{prefix} {function_name}"));
                stripped += 1;
            }
        }
    }

    assert_eq!(
        stripped,
        command_functions.len(),
        "legacy metadata command sanitization did not find every expected command"
    );

    fs::write(out_path, sanitized).expect("failed to write sanitized legacy metadata module");
}
