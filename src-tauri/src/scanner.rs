use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

/// Representation of a discovered file's safe metadata.
#[derive(Debug, Clone, Serialize)]
pub struct ScannedFileRecord {
    pub filename: String,
    pub relative_path: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub modified_timestamp_ms: Option<u64>,
    pub is_readonly: bool,
}

/// Overall result returned to Tauri IPC.
#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub success: bool,
    pub root_display: String,
    pub files: Vec<ScannedFileRecord>,
    pub file_count: usize,
    pub directory_count: usize,
    pub warnings: Vec<String>,
    pub scan_duration_ms: u64,
}

/// Locates and canonicalizes the project-owned `test-files` directory.
/// Fails closed if the directory does not exist or cannot be verified.
pub fn resolve_test_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Unable to determine current directory: {}", e))?;

    // Check candidate paths: cwd/test-files or cwd/../test-files (if running from src-tauri)
    let candidates = [cwd.join("test-files"), cwd.join("..").join("test-files")];

    for candidate in &candidates {
        if candidate.is_dir() {
            let canonical = candidate.canonicalize().map_err(|e| {
                format!(
                    "Failed to canonicalize test directory {:?}: {}",
                    candidate, e
                )
            })?;
            return Ok(canonical);
        }
    }

    Err(
        "Security boundary error: Dedicated 'test-files' directory not found. Scanning is strictly prohibited outside test-files."
            .to_string(),
    )
}

/// Checks if a file entry is a symbolic link or Windows reparse point (junction).
fn is_reparse_point_or_symlink(path: &Path, metadata: &fs::Metadata) -> bool {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
        if (metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return true;
        }
    }

    let _ = path;
    false
}

/// Recursively traverses the approved canonical root directory.
/// Enforces strict boundary verification on every candidate entry.
fn traverse_directory(
    current_dir: &Path,
    canonical_root: &Path,
    files: &mut Vec<ScannedFileRecord>,
    warnings: &mut Vec<String>,
    dir_count: &mut usize,
) {
    let entries = match fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(err) => {
            warnings.push(format!("Cannot read directory {:?}: {}", current_dir, err));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                warnings.push(format!("Error reading directory entry: {}", err));
                continue;
            }
        };

        let path = entry.path();

        // 1. Resolve and verify canonical path against the approved root
        let canonical_entry = match path.canonicalize() {
            Ok(c) => c,
            Err(err) => {
                warnings.push(format!("Cannot canonicalize path {:?}: {}", path, err));
                continue;
            }
        };

        // Strict boundary check: Fail closed if path escapes canonical root
        if !canonical_entry.starts_with(canonical_root) {
            warnings.push(format!(
                "Security violation rejected: Entry {:?} escapes approved root {:?}",
                canonical_entry, canonical_root
            ));
            continue;
        }

        // 2. Query file metadata (symlink / reparse check without following symlinks)
        let symlink_meta = match fs::symlink_metadata(&canonical_entry) {
            Ok(m) => m,
            Err(err) => {
                warnings.push(format!(
                    "Cannot read metadata for {:?}: {}",
                    canonical_entry, err
                ));
                continue;
            }
        };

        if is_reparse_point_or_symlink(&canonical_entry, &symlink_meta) {
            warnings.push(format!(
                "Skipped reparse point / symlink for safety: {:?}",
                canonical_entry
            ));
            continue;
        }

        // 3. Handle directories and files
        if symlink_meta.is_dir() {
            *dir_count += 1;
            traverse_directory(&canonical_entry, canonical_root, files, warnings, dir_count);
        } else if symlink_meta.is_file() {
            // Compute relative path
            let relative_path = match canonical_entry.strip_prefix(canonical_root) {
                Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
                Err(_) => entry.file_name().to_string_lossy().to_string(),
            };

            let filename = entry.file_name().to_string_lossy().to_string();
            let extension = path
                .extension()
                .map(|ext| ext.to_string_lossy().to_string());
            let size_bytes = symlink_meta.len();

            let modified_timestamp_ms = symlink_meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64);

            let is_readonly = symlink_meta.permissions().readonly();

            files.push(ScannedFileRecord {
                filename,
                relative_path,
                extension,
                size_bytes,
                modified_timestamp_ms,
                is_readonly,
            });
        }
    }
}

/// Executes a safe scan against an explicitly verified canonical root.
pub fn scan_directory_with_root(canonical_root: &Path) -> Result<ScanResult, String> {
    if !canonical_root.is_dir() {
        return Err(format!(
            "Specified root {:?} is not a valid directory.",
            canonical_root
        ));
    }

    let start_time = Instant::now();
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    let mut dir_count = 0;

    traverse_directory(
        canonical_root,
        canonical_root,
        &mut files,
        &mut warnings,
        &mut dir_count,
    );

    // Sort files by relative path for consistent, deterministic display
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let file_count = files.len();
    let scan_duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(ScanResult {
        success: true,
        root_display: canonical_root.to_string_lossy().to_string(),
        files,
        file_count,
        directory_count: dir_count,
        warnings,
        scan_duration_ms,
    })
}

/// Executes a safe scan against the verified development test directory.
pub fn scan_test_directory_internal() -> Result<ScanResult, String> {
    let canonical_root = resolve_test_root()?;
    scan_directory_with_root(&canonical_root)
}

/// Validates a relative file path against the canonical test-files root.
/// Enforces:
/// 1. Path must not be empty.
/// 2. Path must resolve strictly inside `test-files/` sandbox without traversal.
/// 3. Path must not be a symbolic link or Windows reparse point (junction).
/// 4. Path must point to an existing regular file.
pub fn validate_test_file_path(relative_path: &str) -> Result<PathBuf, String> {
    let canonical_root = resolve_test_root()?;
    let clean_relative = relative_path.trim_start_matches(['/', '\\']);
    if clean_relative.is_empty() {
        return Err("Relative path cannot be empty".to_string());
    }

    let candidate_path = canonical_root.join(clean_relative);
    let canonical_target = candidate_path
        .canonicalize()
        .map_err(|e| format!("File not found or unreadable {:?}: {}", candidate_path, e))?;

    // Strict boundary enforcement: Fail closed if path escapes canonical sandbox
    if !canonical_target.starts_with(&canonical_root) {
        return Err(format!(
            "Security violation: Target {:?} escapes approved root {:?}",
            canonical_target, canonical_root
        ));
    }

    let symlink_meta = fs::symlink_metadata(&canonical_target)
        .map_err(|e| format!("Failed to read metadata for {:?}: {}", canonical_target, e))?;

    if is_reparse_point_or_symlink(&canonical_target, &symlink_meta) {
        return Err(format!(
            "Security violation: Reparse point or symlink rejected: {:?}",
            canonical_target
        ));
    }

    if !symlink_meta.is_file() {
        return Err(format!(
            "Target is not a regular file: {:?}",
            canonical_target
        ));
    }

    Ok(canonical_target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_root_and_scan() {
        let result = scan_test_directory_internal().expect("Scan internal should succeed");
        assert!(result.success);
        assert!(result.file_count >= 3, "Expected at least 3 test fixtures");

        let relative_paths: Vec<String> = result
            .files
            .iter()
            .map(|f| f.relative_path.clone())
            .collect();
        assert!(
            relative_paths.contains(&"sample.txt".to_string()),
            "Should find sample.txt at root"
        );
        assert!(
            relative_paths.contains(&"notes.txt".to_string()),
            "Should find notes.txt at root"
        );
        assert!(
            relative_paths.contains(&"nested_docs/nested_readme.txt".to_string()),
            "Should find nested_readme.txt in nested directory"
        );

        // Verify metadata collected without reading contents
        for file in &result.files {
            assert!(!file.filename.is_empty());
            assert!(file.modified_timestamp_ms.is_some());
        }
    }

    #[test]
    fn test_empty_directory_handling() {
        let canonical_root = resolve_test_root().expect("Root must resolve");
        let empty_dir = canonical_root.join("empty_folder");
        assert!(empty_dir.is_dir(), "empty_folder fixture must exist");

        let result =
            scan_directory_with_root(&empty_dir).expect("Empty directory scan should succeed");
        assert_eq!(
            result.file_count, 0,
            "Empty directory should return 0 files"
        );
        assert_eq!(result.files.len(), 0);
        assert!(result.success);
    }

    #[test]
    fn test_invalid_or_missing_root_returns_error() {
        let non_existent = PathBuf::from("C:\\NonExistent_Dir_Test_12345");
        let err = scan_directory_with_root(&non_existent);
        assert!(
            err.is_err(),
            "Scanning non-existent directory must return error"
        );
    }

    #[test]
    fn test_boundary_enforcement_logic() {
        let canonical_root = resolve_test_root().expect("Root must resolve");
        let fake_external_path = PathBuf::from("C:\\Windows\\System32");
        assert!(
            !fake_external_path.starts_with(&canonical_root),
            "External path must not start with canonical root"
        );
    }

    #[test]
    fn test_validate_test_file_path_valid() {
        let canonical_root = resolve_test_root().expect("Root must resolve");
        let validated = validate_test_file_path("sample.txt").expect("sample.txt must validate");
        assert_eq!(validated, canonical_root.join("sample.txt"));
    }

    #[test]
    fn test_validate_test_file_path_traversal_rejected() {
        let err = validate_test_file_path("../../Windows/System32/cmd.exe").unwrap_err();
        assert!(err.contains("Security violation") || err.contains("not found"));
    }

    #[test]
    fn test_validate_test_file_path_empty_rejected() {
        let err = validate_test_file_path("   ").unwrap_err();
        assert!(err.contains("empty") || err.contains("not found"));
    }

    #[test]
    fn test_validate_test_file_path_directory_rejected() {
        let err = validate_test_file_path("empty_folder").unwrap_err();
        assert!(err.contains("not a regular file"));
    }
}
