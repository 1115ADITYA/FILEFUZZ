# Development Roadmap & Task Tracker — Offline File Finder

This tracker defines the sequential phases for building **Offline File Finder**. Tasks are only marked complete (`[x]`) when backed by executed, verified tests.

---

## Phase 1: Environment Verification
- **Status:** `COMPLETE`
- **Goal:** Verify that all Windows C++ build tools, Rust toolchains, Node/npm, WebView2, and Git prerequisites are present and operational.
- **Tasks:**
  - [x] Check Node.js (`v24.15.0`) and npm (`11.12.1`).
  - [x] Verify Git (`2.54.0.windows.1`).
  - [x] Install and verify official Rust toolchain (`1.99.0`, `x86_64-pc-windows-msvc`).
  - [x] Install and verify Visual Studio 2022 Build Tools (MSVC `v14.44.35207`, Windows SDK `10.0.26100.0`).
  - [x] Verify Microsoft Edge WebView2 Evergreen Runtime (`154.0.4258.62`).
- **Acceptance Criteria:** All CLI tool checks pass with 0 exit codes. C++ compilers and Windows SDK libraries verified on disk.

---

## Phase 2: Minimal Desktop Shell
- **Status:** `COMPLETE`
- **Goal:** Bootstrap a stable, compiling, and launching Tauri 2 + React + TypeScript + Vite + Tailwind CSS desktop shell.
- **Tasks:**
  - [x] Scaffold Tauri 2 + React + TypeScript in workspace without destructive commands.
  - [x] Configure Tailwind CSS 3.4 with PostCSS and base styles.
  - [x] Configure `tauri.conf.json` with app title ("Offline File Finder"), window sizing (1024x720), and identifiers.
  - [x] Implement backend IPC health command `get_app_status` in Rust (`src-tauri/src/lib.rs`).
  - [x] Create initial desktop UI with search bar placeholder, status pills, and empty-state guidance.
  - [x] Connect frontend to Rust IPC and display live backend status.
  - [x] Run `npm run build` (typecheck + Vite production bundle).
  - [x] Run `cargo fmt --check`, `cargo check`, and `cargo build`.
  - [x] Launch the compiled desktop executable and verify window + WebView2 initialization.
- **Acceptance Criteria:** Desktop binary launches, runs WebView2 runtime without crashing, and successfully displays genuine Rust status over IPC.

---

## Phase 3: Documentation & Project Controls
- **Status:** `COMPLETE`
- **Goal:** Formalize product specifications, architecture documentation, task tracking, progress history, and strict agent safety rules before writing scanner code.
- **Tasks:**
  - [x] Create `PRD.md` with complete functional, privacy, and non-functional requirements.
  - [x] Create `ARCHITECTURE.md` accurately reflecting Phase 1 and detailing planned modules.
  - [x] Create `TASKS.md` with testable criteria for all 12 roadmap phases.
  - [x] Create `PROGRESS.md` logging execution history, verified results, and current Git state.
  - [x] Create `.agent/rules/project-rules.md` documenting the 15 mandatory safety and workflow constraints.
  - [x] Verify that all 5 files exist, are mutually consistent, and do not modify source code.
- **Acceptance Criteria:** All 5 documentation files exist, are consistent, and distinguish implemented from planned features.

---

## Phase 4: Safe Test-Folder Scanner (User Prompt Phase 3)
- **Status:** `COMPLETE`
- **Goal:** Implement secure, read-only directory traversal and metadata collection strictly limited to `<project-root>/test-files/`.
- **Tasks:**
  - [x] Create synthetic test fixture directory `<project-root>/test-files/` with nested folders, empty folders, and mixed files.
  - [x] Implement canonical path validation to reject any scan root outside `test-files/`.
  - [x] Implement NTFS reparse point and symlink detection to prevent out-of-boundary traversal.
  - [x] Implement read-only metadata collection (filename, path, extension, size, created, modified).
  - [x] Implement Tauri command `scan_test_directory` returning `ScanResult`.
  - [x] Build UI scan controls, progress indicators, metrics bar, and tabular file metadata display in `App.tsx`.
  - [x] Verify scanner unit tests (`cargo test` 4 passing tests).
  - [x] Verify frontend build (`npm run build`) and desktop binary execution.
- **Acceptance Criteria:** Scanner successfully discovers all fixture files, collects accurate metadata, refuses to scan any path outside `test-files/`, and never modifies test files.

---

## Phase 5: TXT and PDF Text Extraction (User Prompt Phase 4)
- **Status:** `COMPLETE`
- **Goal:** Extract raw text from `.txt` and `.pdf` files behind a unified `DocumentExtractor` trait.
- **Tasks:**
  - [x] Define the `DocumentExtractor` trait and `ExtractionResponse` / `ExtractionError` types.
  - [x] Implement `TextExtractor` for `.txt` files with UTF-8 / lossy fallback handling.
  - [x] Integrate `pdf-extract` and `lopdf` and implement `PdfExtractor` for text-based PDFs.
  - [x] Add extraction safety limits (10MB for TXT, 25MB for PDF).
  - [x] Handle malformed, empty, or corrupted files gracefully without aborting scans or crashing.
  - [x] Add synthetic PDF, empty TXT, and corrupt PDF test fixtures.
  - [x] Implement Tauri command `extract_test_file(relative_path)` enforcing `test-files/` sandbox boundary.
  - [x] Add UI extraction actions and text preview panel in `App.tsx`.
  - [x] Verify unit tests (12 tests passing in `src-tauri`).
- **Acceptance Criteria:** Valid TXT and PDF files have text extracted accurately; corrupt/encrypted files record non-fatal errors; scan process remains stable.

---

## Phase 6: Tantivy Local Keyword Search
- **Status:** `COMPLETED`
- **Goal:** Enable instant local full-text search across filenames and extracted content using Tantivy.
- **Tasks:**
  - [x] Define Tantivy index schema and initialize index in dedicated development directory (`.tantivy-index/`).
  - [x] Implement document indexing pipeline for scanned metadata and extracted text (`index_test_files`).
  - [x] Implement Tauri command `search_test_files(query: String)`.
  - [x] Add search query parsing, BM25 scoring, and snippet generation.
  - [x] Update React UI with search input, explicit "Index Test Files" button, indexing metrics, and result cards with snippets.
  - [x] Fix Windows memory-mapped file locking on repeated commits via unified `SearchIndexManager`, `ReloadPolicy::Manual`, `wait_merging_threads()`, and Vite watcher exclusion.
  - [x] Test empty queries, special characters, unmatched queries, and duplicate prevention on re-indexing.
- **Acceptance Criteria:** Search queries return relevant files in < 50ms; extracted document terms are searchable; index survives application restarts.

---

## Pre-Phase-6 Quality Gate: PDF Extraction & Search Diagnostics
- **Status:** `COMPLETE`
- **Goal:** Diagnose PDF 0-char extraction issue and search term mismatch on `DSAssignment07-25101A0022.pdf`.
- **Tasks:**
  - [x] Diagnose PDF internal layer structure (8 pages, zero `/Font` objects, purely scanned raster `/XObject` images).
  - [x] Enhance extraction preview with page counts and diagnostic notice distinguishing empty files from image-only scans.
  - [x] Explain Tantivy 6 vs Scanner 7 count mismatch (6 indexed + 1 failed extraction `corrupt.pdf` = 7 discovered).
  - [x] Implement subword/camelCase filename decomposition in `src-tauri/src/search.rs` to allow searches like `assignment` to match `DSAssignment07`.
  - [x] Fix repeated indexing file locking via explicit writer dropping and retry backoff on Windows OS error 5.
  - [x] Add regression tests for scanned PDF diagnostics, compound filename search, and repeated indexing without permission errors.
- **Acceptance Criteria:** All 16 unit tests pass, `npm run build` succeeds, `cargo fmt` and `cargo check` pass.

---

## Phase 7: DOCX Extraction & File Location Actions
- **Status:** `COMPLETE`
- **Goal:** Add Word `.docx` extraction and desktop integration actions (Open File, Reveal in Explorer).
- **Tasks:**
  - [x] Implement `DocxExtractor` reading `word/document.xml` from the DOCX ZIP archive in document order with paragraph boundaries and entity decoding.
  - [x] Add zip bomb and memory safety limits (25MB archive size, 50MB decompressed XML limit).
  - [x] Implement `open_test_file(relative_path)` with strict sandbox and symlink/reparse point validation before launching default handler via `rundll32.exe`.
  - [x] Implement `reveal_test_file(relative_path)` using Windows `explorer.exe /select,"<path>"` with separate argument passing without shell concatenation.
  - [x] Integrate DOCX extraction into Tantivy indexer and search query pipeline.
  - [x] Add "Open" and "Show in Folder" action buttons to search result cards, scanned files table, and extraction preview.
  - [x] Add synthetic DOCX test fixture (`sample_document.docx`) and comprehensive unit tests.
- **Acceptance Criteria:** Text inside `.docx` files is indexed and searchable; clicking "Show in Folder" highlights the exact file in Windows File Explorer without using shell concatenation; all 28 automated tests pass.

---

## Phase 8: PPTX and XLSX Extraction
- **Status:** `PLANNED`
- **Goal:** Extend document extraction to PowerPoint (`.pptx`) and Excel (`.xlsx`) files.
- **Tasks:**
  - [ ] Implement `PptxExtractor` extracting text from slide XML streams.
  - [ ] Integrate `calamine` to implement `XlsxExtractor` extracting tabular sheet cell contents.
  - [ ] Benchmark memory consumption on large spreadsheets and enforce row/cell limits.
- **Acceptance Criteria:** Presentations and spreadsheets have content extracted and indexed; memory remains bounded during large workbook parsing.

---

## Phase 9: Guided Scanning & Background Rescanning
- **Status:** `PLANNED`
- **Goal:** Allow user-directed selection of real folders (Documents, Downloads, Desktop) with persistent SQLite configuration and background change detection.
- **Tasks:**
  - [ ] Implement SQLite metadata database (`rusqlite`) for tracking file hashes, timestamps, and scan roots.
  - [ ] Build UI folder picker dialog for user-approved scan roots.
  - [ ] Implement background rescan scheduling with change detection (hash/mtime check).
  - [ ] Implement safe removal of stale index entries only after a full, verified scan.
- **Acceptance Criteria:** Users can select and remove scan roots; background rescan detects added/modified/deleted files without blocking the UI; stale entries safely purged.

---

## Phase 10: Local Semantic Search (FastEmbed)
- **Status:** `PLANNED`
- **Goal:** Enable semantic query matching using local ONNX embeddings without any cloud API.
- **Tasks:**
  - [ ] Evaluate `fastembed` crate running local quantized embedding models (e.g., `all-MiniLM-L6-v2`).
  - [ ] Implement explicit user-controlled local model download/verification.
  - [ ] Generate and store local embeddings for document chunks.
  - [ ] Add semantic search toggle to UI and combine keyword + vector scores.
- **Acceptance Criteria:** Semantic search operates completely offline; missing models fall back cleanly to Tantivy keyword search.

---

## Phase 11: OCR & Categorization
- **Status:** `PLANNED`
- **Goal:** Add optional OCR for scanned images/PDFs and automatic document categorization.
- **Tasks:**
  - [ ] Integrate optional local Tesseract OCR engine for scanned files.
  - [ ] Implement rule-based and content-based document categorization (e.g. Invoices, Receipts, Resumes, Certificates).
  - [ ] Ensure OCR is strictly opt-in and does not degrade default scan speed.
- **Acceptance Criteria:** Scanned PDFs have text extracted via local OCR; category badges appear on result cards.

---

## Phase 12: Final Testing, Security Review & Distribution
- **Status:** `PLANNED`
- **Goal:** Harden the application, conduct comprehensive security auditing, and generate production Windows installers.
- **Tasks:**
  - [ ] Conduct air-gap network audit to verify zero outbound network packets.
  - [ ] Verify read-only guarantees across all edge cases (locked files, readonly files, special permissions).
  - [ ] Run full automated test suite (Rust unit/integration tests + frontend tests).
  - [ ] Build production NSIS / MSI installer via `npm run tauri build`.
  - [ ] Document user manual, troubleshooting guide, and system requirements.
- **Acceptance Criteria:** Production installer builds and installs cleanly on fresh Windows 10/11 machines; verified 100% offline and read-only.
