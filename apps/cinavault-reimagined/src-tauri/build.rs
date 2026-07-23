use std::{fs, path::Path};

fn main() {
    let icon_dir = Path::new("icons");
    let icon_path = icon_dir.join("icon.ico");

    if !icon_path.exists() {
        fs::create_dir_all(icon_dir).expect("failed to create Tauri icon directory");
        fs::copy("../../../src-tauri/icons/icon.ico", &icon_path)
            .expect("failed to copy the existing CinaVault Windows icon");
    }

    tauri_build::build()
}
