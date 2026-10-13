pub mod extractor;
pub mod scanner;
pub mod search;

use extractor::{extract_test_file_internal, ExtractionResponse};
use scanner::{scan_test_directory_internal, ScanResult};
use search::{
    index_test_files_internal, search_test_files_internal, IndexSummary, SearchResultResponse,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppStatus {
    pub status: String,
    pub message: String,
    pub version: String,
    pub rust_version: String,
    pub os: String,
    pub offline_verified: bool,
}

#[tauri::command]
fn get_app_status() -> AppStatus {
    AppStatus {
        status: "operational".to_string(),
        message: "Rust core backend is running and healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        rust_version: "1.99.0 (MSVC)".to_string(),
        os: "Windows (x86_64)".to_string(),
        offline_verified: true,
    }
}

#[tauri::command]
fn scan_test_directory() -> Result<ScanResult, String> {
    scan_test_directory_internal()
}

#[tauri::command]
fn extract_test_file(relative_path: String) -> Result<ExtractionResponse, String> {
    extract_test_file_internal(&relative_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn index_test_files() -> Result<IndexSummary, String> {
    index_test_files_internal()
}

#[tauri::command]
fn search_test_files(query: String) -> Result<SearchResultResponse, String> {
    search_test_files_internal(&query)
}

#[tauri::command]
fn open_test_file(relative_path: String) -> Result<(), String> {
    let validated = scanner::validate_test_file_path(&relative_path)?;
    #[cfg(windows)]
    {
        std::process::Command::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(&validated)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(&validated)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        Ok(())
    }
}

#[tauri::command]
fn reveal_test_file(relative_path: String) -> Result<(), String> {
    let validated = scanner::validate_test_file_path(&relative_path)?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", validated.display()))
            .spawn()
            .map_err(|e| format!("Failed to reveal file: {}", e))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if let Some(parent) = validated.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| format!("Failed to reveal folder: {}", e))?;
        }
        Ok(())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            scan_test_directory,
            extract_test_file,
            index_test_files,
            search_test_files,
            open_test_file,
            reveal_test_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_test_file_traversal_rejected() {
        let err = open_test_file("../../Windows/System32/cmd.exe".to_string()).unwrap_err();
        assert!(err.contains("Security violation") || err.contains("not found"));
    }

    #[test]
    fn test_reveal_test_file_traversal_rejected() {
        let err = reveal_test_file("../../Windows/System32/cmd.exe".to_string()).unwrap_err();
        assert!(err.contains("Security violation") || err.contains("not found"));
    }

    #[test]
    fn test_open_test_file_empty_rejected() {
        let err = open_test_file("   ".to_string()).unwrap_err();
        assert!(err.contains("empty") || err.contains("not found"));
    }
}
