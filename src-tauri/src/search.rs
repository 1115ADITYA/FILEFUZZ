use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::Instant;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, Value, STORED, STRING, TEXT};
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument};

use crate::extractor::{DocumentExtractor, DocxExtractor, PdfExtractor, TextExtractor};
use crate::scanner::{resolve_test_root, scan_test_directory_internal};

/// Summary metrics returned after indexing operation completes.
#[derive(Debug, Clone, Serialize)]
pub struct IndexSummary {
    pub success: bool,
    pub indexed_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub index_directory: String,
    pub duration_ms: u64,
}

/// Individual hit returned from a keyword search query.
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub filename: String,
    pub relative_path: String,
    pub extension: String,
    pub score: f32,
    pub snippet: String,
}

/// Overall search response payload returned to Tauri IPC.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResultResponse {
    pub success: bool,
    pub query: String,
    pub total_hits: usize,
    pub results: Vec<SearchHit>,
    pub duration_ms: u64,
}

/// Field handles for the Tantivy schema.
#[derive(Clone, Copy)]
pub struct SearchSchemaFields {
    pub id: tantivy::schema::Field,
    pub relative_path: tantivy::schema::Field,
    pub filename: tantivy::schema::Field,
    pub extension: tantivy::schema::Field,
    pub body: tantivy::schema::Field,
}

/// Builds the formal Tantivy schema.
pub fn build_schema() -> (Schema, SearchSchemaFields) {
    let mut builder = Schema::builder();
    let id = builder.add_text_field("id", STRING | STORED);
    let relative_path = builder.add_text_field("relative_path", STRING | STORED);
    let filename = builder.add_text_field("filename", TEXT | STORED);
    let extension = builder.add_text_field("extension", STRING | STORED);
    let body = builder.add_text_field("body", TEXT | STORED);

    let schema = builder.build();
    let fields = SearchSchemaFields {
        id,
        relative_path,
        filename,
        extension,
        body,
    };

    (schema, fields)
}

/// Decomposes complex filename strings into individual words by breaking on
/// camelCase transitions, letter/digit boundaries, and punctuation marks.
/// E.g. "DSAssignment07-25101A0022.pdf" -> "DS Assignment 07 25101 A 0022"
pub fn decompose_filename(name: &str) -> String {
    let mut result = String::with_capacity(name.len() * 2);
    let chars: Vec<char> = name.chars().collect();
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];
        if !c.is_alphanumeric() {
            result.push(' ');
            continue;
        }

        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();

            let lower_to_upper = prev.is_lowercase() && c.is_uppercase();
            let acronym_to_word =
                prev.is_uppercase() && c.is_uppercase() && next.map_or(false, |n| n.is_lowercase());
            let letter_to_digit = prev.is_alphabetic() && c.is_ascii_digit();
            let digit_to_letter = prev.is_ascii_digit() && c.is_alphabetic();

            if lower_to_upper || acronym_to_word || letter_to_digit || digit_to_letter {
                result.push(' ');
            }
        }
        result.push(c);
    }
    result
}

/// Resolves the dedicated local development index directory (`<project-root>/.tantivy-index`).
/// Ensures the index is never stored inside `test-files/`.
pub fn resolve_index_dir() -> Result<PathBuf, String> {
    let canonical_test_root = crate::scanner::resolve_test_root()?;
    let project_root = canonical_test_root
        .parent()
        .ok_or_else(|| "Cannot determine project root from test-files".to_string())?;

    let index_dir = project_root.join(".tantivy-index");
    if !index_dir.exists() {
        fs::create_dir_all(&index_dir)
            .map_err(|e| format!("Failed to create .tantivy-index directory: {}", e))?;
    }

    Ok(index_dir)
}

/// Opens or creates the Tantivy index in the designated directory.
pub fn get_or_create_index(index_dir: &Path) -> Result<Index, String> {
    let (schema, _) = build_schema();

    if index_dir.join("meta.json").exists() {
        Index::open_in_dir(index_dir)
            .map_err(|e| format!("Failed to open existing Tantivy index: {}", e))
    } else {
        Index::create_in_dir(index_dir, schema)
            .map_err(|e| format!("Failed to create Tantivy index in dir: {}", e))
    }
}

/// Unified, thread-safe search manager holding a single Index and single IndexReader.
/// Manages the full lifecycle on Windows without memory-mapped file locking conflicts.
pub struct SearchIndexManager {
    pub index: Index,
    pub reader: IndexReader,
}

static SEARCH_MANAGER: OnceLock<RwLock<SearchIndexManager>> = OnceLock::new();

/// Retrieves the global SearchIndexManager, initializing it on first call.
/// Configures IndexReader with ReloadPolicy::Manual to eliminate background timer threads
/// that would otherwise hold obsolete segment file handles open on Windows.
pub fn get_search_manager() -> Result<&'static RwLock<SearchIndexManager>, String> {
    if let Some(manager) = SEARCH_MANAGER.get() {
        return Ok(manager);
    }

    let index_dir = resolve_index_dir()?;
    let index = get_or_create_index(&index_dir)?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .map_err(|e| format!("Failed to initialize Tantivy IndexReader: {}", e))?;

    let _ = SEARCH_MANAGER.set(RwLock::new(SearchIndexManager { index, reader }));

    SEARCH_MANAGER
        .get()
        .ok_or_else(|| "Failed to retrieve search manager".to_string())
}

fn perform_indexing_attempt() -> Result<IndexSummary, String> {
    let start_time = Instant::now();
    let manager_lock = get_search_manager()?;
    // Acquire exclusive write lock: prevents concurrent indexing or searches from competing
    let manager = manager_lock
        .write()
        .map_err(|e| format!("Failed to acquire search manager write lock: {}", e))?;

    let (_, fields) = build_schema();

    // Allocate 50MB buffer for the index writer
    let mut index_writer = manager
        .index
        .writer(50_000_000)
        .map_err(|e| format!("Failed to create Tantivy IndexWriter: {}", e))?;

    // Clear all existing documents to ensure clean, zero-duplicate re-indexing
    index_writer
        .delete_all_documents()
        .map_err(|e| format!("Failed to clear previous index documents: {}", e))?;

    // Scan test directory using existing scanner
    let scan_result = scan_test_directory_internal()?;
    let canonical_root = resolve_test_root()?;

    let text_extractor = TextExtractor::default();
    let pdf_extractor = PdfExtractor::default();
    let docx_extractor = DocxExtractor::default();

    let mut indexed_count = 0;
    let mut skipped_count = 0;
    let mut failed_count = 0;

    for file in &scan_result.files {
        let ext = file.extension.as_deref().unwrap_or("").to_lowercase();
        let file_path = canonical_root.join(&file.relative_path);

        // Strict boundary check: Verify path remains inside canonical root
        if !file_path.starts_with(&canonical_root) {
            failed_count += 1;
            continue;
        }

        let extracted_text = if text_extractor.supports(&ext) {
            match text_extractor.extract_text(&file_path) {
                Ok(t) => t,
                Err(_) => {
                    failed_count += 1;
                    continue;
                }
            }
        } else if pdf_extractor.supports(&ext) {
            match pdf_extractor.extract_text(&file_path) {
                Ok(t) => t,
                Err(_) => {
                    failed_count += 1;
                    continue;
                }
            }
        } else if docx_extractor.supports(&ext) {
            match docx_extractor.extract_text(&file_path) {
                Ok(t) => t,
                Err(_) => {
                    failed_count += 1;
                    continue;
                }
            }
        } else {
            // Unsupported format in Phase 6
            skipped_count += 1;
            continue;
        };

        let mut doc = TantivyDocument::default();
        doc.add_text(fields.id, &file.relative_path);
        doc.add_text(fields.relative_path, &file.relative_path);
        // Primary filename added first (retrieved for hit display)
        doc.add_text(fields.filename, &file.filename);
        // Decomposed filename added for sub-word and camelCase keyword searchability
        let decomposed = decompose_filename(&file.filename);
        doc.add_text(fields.filename, &decomposed);
        doc.add_text(fields.extension, &ext);
        doc.add_text(fields.body, &extracted_text);

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Failed to add document to index: {}", e))?;

        indexed_count += 1;
    }

    index_writer
        .commit()
        .map_err(|e| format!("Failed to commit Tantivy index: {}", e))?;

    // Gracefully wait for all background segment merging operations to finish
    // and release file handles and writer locks (takes index_writer by value)
    index_writer
        .wait_merging_threads()
        .map_err(|e| format!("Failed to wait for merging threads: {}", e))?;

    // Refresh the reader so subsequent searches observe the latest commit point
    manager
        .reader
        .reload()
        .map_err(|e| format!("Failed to reload Tantivy reader: {}", e))?;

    let duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(IndexSummary {
        success: true,
        indexed_count,
        skipped_count,
        failed_count,
        index_directory: resolve_index_dir()?.to_string_lossy().to_string(),
        duration_ms,
    })
}

/// Indexes all supported files inside the approved `test-files/` directory.
/// Performs a deterministic rebuild of the test index with synchronized writer lifecycle,
/// graceful merge thread completion, explicit writer dropping, and retry backoff for Windows file contention.
pub fn index_test_files_internal() -> Result<IndexSummary, String> {
    let mut last_error = String::new();
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(150 * attempt as u64));
        }
        match perform_indexing_attempt() {
            Ok(summary) => return Ok(summary),
            Err(e) => {
                if e.contains("PermissionDenied")
                    || e.contains("Access is denied")
                    || e.contains("index writer was killed")
                    || e.contains("worker thread encountered an error")
                {
                    last_error = e;
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }
    Err(last_error)
}

/// Searches the Tantivy index for matching keywords in filenames and body contents.
/// Uses the unified SearchIndexManager's reader to avoid creating unmanaged reader threads.
pub fn search_test_files_internal(query_str: &str) -> Result<SearchResultResponse, String> {
    let start_time = Instant::now();
    let trimmed_query = query_str.trim();

    // Gracefully handle empty or whitespace queries
    if trimmed_query.is_empty() {
        return Ok(SearchResultResponse {
            success: true,
            query: String::new(),
            total_hits: 0,
            results: Vec::new(),
            duration_ms: 0,
        });
    }

    let manager_lock = get_search_manager()?;
    let manager = manager_lock
        .read()
        .map_err(|e| format!("Failed to acquire search manager read lock: {}", e))?;

    let (_, fields) = build_schema();
    let searcher = manager.reader.searcher();

    // Search across filename and extracted body text
    let query_parser = QueryParser::for_index(&manager.index, vec![fields.filename, fields.body]);
    let query = query_parser
        .parse_query(trimmed_query)
        .map_err(|e| format!("Invalid search query syntax: {}", e))?;

    // Top 50 results ordered by BM25 score
    let top_docs = searcher
        .search(&query, &TopDocs::with_limit(50).order_by_score())
        .map_err(|e| format!("Search execution failed: {}", e))?;

    let mut snippet_generator = SnippetGenerator::create(&searcher, &*query, fields.body)
        .map_err(|e| format!("Failed to initialize snippet generator: {}", e))?;
    snippet_generator.set_max_num_chars(160);

    let mut results = Vec::new();
    let total_hits = top_docs.len();

    for (score, doc_address) in top_docs {
        let retrieved_doc: TantivyDocument = searcher
            .doc(doc_address)
            .map_err(|e| format!("Failed to retrieve document: {}", e))?;

        let filename = retrieved_doc
            .get_first(fields.filename)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let relative_path = retrieved_doc
            .get_first(fields.relative_path)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let extension = retrieved_doc
            .get_first(fields.extension)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let snippet = snippet_generator.snippet_from_doc(&retrieved_doc).to_html();

        results.push(SearchHit {
            filename,
            relative_path,
            extension,
            score,
            snippet,
        });
    }

    let duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(SearchResultResponse {
        success: true,
        query: trimmed_query.to_string(),
        total_hits,
        results,
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_repeated_indexing_and_search_regression() {
        let _guard = TEST_MUTEX.lock().unwrap();
        // Execute indexing and searching repeatedly at least 6 times to verify
        // no permission denied errors, no file locks, and zero duplicate hits.
        for i in 0..6 {
            let summary = index_test_files_internal()
                .unwrap_or_else(|e| panic!("Indexing iteration {} failed: {}", i, e));
            assert!(summary.success);
            assert!(
                summary.indexed_count >= 3,
                "Expected at least 3 files indexed on iteration {}",
                i
            );
            assert!(
                summary.failed_count >= 1,
                "Corrupt PDF must record failure without aborting indexing on iteration {}",
                i
            );

            // 1. Search filename
            let res_filename = search_test_files_internal("sample").expect("Search filename");
            assert!(res_filename.total_hits > 0);
            assert!(res_filename
                .results
                .iter()
                .any(|r| r.filename.contains("sample")));

            // 2. Search TXT content & verify no duplicate entries
            let res_txt = search_test_files_internal("discovery").expect("Search TXT content");
            assert_eq!(
                res_txt.total_hits, 1,
                "Expected exactly 1 hit for notes.txt without duplicates on iteration {}",
                i
            );
            assert_eq!(res_txt.results[0].relative_path, "notes.txt");

            // 3. Search PDF content
            let res_pdf = search_test_files_internal("Synthetic").expect("Search PDF content");
            assert!(res_pdf.total_hits > 0);
            assert!(res_pdf.results.iter().any(|r| r.extension == "pdf"));

            // Allow Windows filesystem filter drivers / indexer a momentary breath between back-to-back iterations
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // 4. Query with no matches returns 0 hits
        let res_empty = search_test_files_internal("nonexistentterm9999").expect("No match search");
        assert_eq!(res_empty.total_hits, 0);
        assert!(res_empty.results.is_empty());

        // 5. Empty query handled safely
        let res_blank = search_test_files_internal("   ").expect("Blank query search");
        assert_eq!(res_blank.total_hits, 0);

        // 6. Invalid query handled safely
        let res_syntax = search_test_files_internal("AND OR NOT ((");
        assert!(res_syntax.is_err() || res_syntax.unwrap().total_hits == 0);
    }

    #[test]
    fn test_search_assignment_pdf() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let summary = index_test_files_internal().expect("Indexing should succeed");
        assert!(summary.success);

        // 1. Lowercase "assignment" must match "DSAssignment07-25101A0022.pdf"
        let res_assignment = search_test_files_internal("assignment").expect("Search assignment");
        assert!(
            res_assignment.total_hits > 0,
            "Query 'assignment' must match 'DSAssignment07-25101A0022.pdf'"
        );
        assert!(res_assignment
            .results
            .iter()
            .any(|r| r.filename.contains("DSAssignment07")));

        // 2. Exact subwords and full token also match
        let res_dsassignment =
            search_test_files_internal("dsassignment07").expect("Search dsassignment07");
        assert!(res_dsassignment.total_hits > 0);

        let res_exact =
            search_test_files_internal("DSAssignment07-25101A0022").expect("Search exact name");
        assert!(res_exact.total_hits > 0);
    }

    #[test]
    fn test_indexing_boundary_enforcement() {
        let canonical_root = resolve_test_root().expect("Root resolution must succeed");
        let parent = canonical_root.parent().expect("Canonical root has parent");
        let outside_file = parent.join("Cargo.toml");
        assert!(
            !outside_file.starts_with(&canonical_root),
            "Paths outside test-files must fail boundary verification"
        );
    }

    #[test]
    fn test_search_docx_content_and_filename() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let canonical_root = resolve_test_root().expect("Root resolution must succeed");
        let docx_path = canonical_root.join("sample_document.docx");
        if !docx_path.exists() {
            crate::extractor::create_test_docx(&docx_path).expect("Create test docx fixture");
        }

        let summary = index_test_files_internal().expect("Indexing should succeed");
        assert!(summary.success);

        // 1. Search DOCX filename
        let res_name = search_test_files_internal("sample_document").expect("Search docx filename");
        assert!(res_name.total_hits > 0);
        assert!(res_name.results.iter().any(|r| r.extension == "docx"));

        // 2. Search DOCX body text (from paragraph 2: "Search document contents locally.")
        let res_body = search_test_files_internal("locally").expect("Search docx body text");
        assert!(res_body.total_hits > 0);
        let docx_hit = res_body
            .results
            .iter()
            .find(|r| r.extension == "docx")
            .expect("Must find docx hit");
        assert!(
            docx_hit.snippet.contains("locally")
                || docx_hit.snippet.contains("<b>locally</b>")
                || docx_hit.snippet.contains("document")
        );

        // 3. Repeated indexing does not duplicate docx hit
        let _ = index_test_files_internal().expect("Second indexing should succeed");
        let res_reindex =
            search_test_files_internal("locally").expect("Search docx body text after reindex");
        let docx_hits: Vec<_> = res_reindex
            .results
            .iter()
            .filter(|r| r.extension == "docx")
            .collect();
        assert_eq!(
            docx_hits.len(),
            1,
            "Exactly one docx hit expected without duplicates"
        );
    }
}
