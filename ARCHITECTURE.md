# System Architecture — Offline File Finder

## 1. Architectural Overview & Design Philosophy

Offline File Finder is architected as an offline-only Windows desktop application utilizing a high-performance **Rust** backend paired with a modern **React / TypeScript / Tailwind CSS** presentation layer hosted within the native **Windows WebView2** runtime via **Tauri 2**.

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        WINDOWS DESKTOP APPLICATION                     │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    PRESENTATION LAYER (React)                    │  │
│  │  - React 19 + TypeScript + Vite + Tailwind CSS                   │  │
│  │  - Components: Search Bar, Results List, Scan Monitor, Settings  │  │
│  └──────────────────────────────────┬───────────────────────────────┘  │
│                                     │                                  │
│                 Tauri 2 IPC Bridge (`invoke` / Events)                 │
│                                     │                                  │
│  ┌──────────────────────────────────▼───────────────────────────────┐  │
│  │                      RUST APPLICATION CORE                       │  │
│  │                                                                  │  │
│  │  ┌────────────────────────────────────────────────────────────┐  │  │
│  │  │ Command / API Layer                                        │  │  │
│  │  │ - Implemented: `get_app_status`                            │  │  │
│  │  │ - Planned: `scan_test_dir`, `search_files`, `reveal_path`  │  │  │
│  │  └───────────────┬────────────────────────────┬───────────────┘  │  │
│  │                  │                            │                  │  │
│  │  ┌───────────────▼───────────────┐  ┌─────────▼───────────────┐  │  │
│  │  │ Scan Engine [PLANNED]         │  │ Search Coordinator      │  │  │
│  │  │ - Traversal (`walkdir`)       │  │   [PLANNED]             │  │  │
│  │  │ - Boundary & Reparse Check    │  │ - Tantivy Index Manager │  │  │
│  │  │ - Metadata Collection         │  │ - Query Parser & Ranker │  │  │
│  │  └───────────────┬───────────────┘  └─────────┬───────────────┘  │  │
│  │                  │                            │                  │  │
│  │  ┌───────────────▼───────────────┐            │                  │  │
│  │  │ Document Extractors [PLANNED] │            │                  │  │
│  │  │ - TXT | PDF | DOCX | PPTX |   │            │                  │  │
│  │  │   XLSX                        │            │                  │  │
│  │  └───────────────┬───────────────┘            │                  │  │
│  │                  │                            │                  │  │
│  │  ┌───────────────▼────────────────────────────▼───────────────┐  │  │
│  │  │ SQLite Metadata Storage (`rusqlite`) [PLANNED]             │  │  │
│  │  │ - File records, hashes, scan states, config                │  │  │
│  │  └────────────────────────────────────────────────────────────┘  │  │
│  │                                                                  │  │
│  │  FUTURE MODULES [PLANNED]:                                       │  │
│  │  - Local FastEmbed (ONNX) Semantic Vector Search                 │  │
│  │  - Local Tesseract OCR for Scanned Documents                     │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│  APPLICATION-OWNED STORAGE (`%LOCALAPPDATA%\com.offlinefilefinder.app`)│
│  - database/metadata.sqlite  - index/tantivy_files/  - logs/  - cache/ │
└────────────────────────────────────────────────────────────────────────┘

            User Documents: READ-ONLY (No modifications allowed)
            Network: 0 outbound connections / No cloud telemetry
```

---

## 2. Implemented Architecture (Phases 1 – 5 Baseline)

### 2.1 Presentation Layer
- **Source Files:** `src/App.tsx`, `src/main.tsx`, `src/index.css`.
- **Technologies:** React 19, TypeScript, Vite 8, Tailwind CSS 3.4, Lucide React 1.55.
- **State & UI Modules:**
  - `get_app_status`: Displays live connection state, backend message, version, toolchain, and verified offline mode.
  - `scan_test_directory`: Triggers safe recursive traversal of the `test-files/` sandbox, displaying metrics and file metadata table.
  - `extract_test_file`: Triggers on-demand text extraction for `.txt` and `.pdf` files, rendering character count, duration, and scrollable text preview.
  - `index_test_files`: Triggers Tantivy indexing with progress indicators and metrics display (indexed, skipped, failed files, duration).
  - `search_test_files`: Debounced and on-demand keyword search returning BM25 ranked hits, snippets with highlighted terms, and direct text inspection actions.

### 2.2 Tauri 2 IPC Layer
- **Entrypoint:** `src-tauri/src/main.rs` invoking `offline_file_finder_lib::run()`.
- **Library & Registered Commands:** `src-tauri/src/lib.rs` registers commands:
  - `get_app_status()` -> `AppStatus`
  - `scan_test_directory()` -> `Result<ScanResult, String>`
  - `extract_test_file(relative_path: String)` -> `Result<ExtractionResponse, String>`
  - `index_test_files()` -> `Result<IndexSummary, String>`
  - `search_test_files(query: String)` -> `Result<SearchResultResponse, String>`
- **Capabilities & Permissions:** `src-tauri/capabilities/default.json` with scoped permissions (`core:default`, `opener:default`). Window title, dimensions (1024x720), and minimum constraints defined in `src-tauri/tauri.conf.json`.

### 2.3 Scanner Engine (`src-tauri/src/scanner.rs`) — *Implemented*
- **Path Canonicalization:** Dynamic resolution (`resolve_test_root()`) locating `<project-root>/test-files/`.
- **Boundary Verification:** Strict `.starts_with(&canonical_root)` check on every candidate entry. Fails closed on any path escaping.
- **Reparse Point Guard:** Checks `FILE_ATTRIBUTE_REPARSE_POINT` (0x400) and `file_type.is_symlink()` to skip Windows junctions and symlinks.
- **Read-Only Metadata:** Collects filename, relative path, extension, byte size, modification timestamp (ms), and readonly flag without reading file contents.

### 2.4 Document Extraction Module (`src-tauri/src/extractor.rs`) — *Implemented*
- **Trait Definition:** `DocumentExtractor` with `supports(&str)` and `extract_text(&Path)`.
- **TextExtractor (`.txt`):** Read-only byte reader with lossy UTF-8 decoding and 10MB safety limit.
- **PdfExtractor (`.pdf`):** Stream text extraction using `pdf-extract` and `lopdf` with 25MB safety limit, non-panicking error mapping (`ParseError`, `EncryptedDocument`).
- **DocxExtractor (`.docx`):** Streaming OpenXML reader using `zip` and `quick-xml` parsing `word/document.xml`. Enforces 25MB archive limit and 50MB decompressed XML limit (ZIP bomb defense). Extracts paragraphs and text runs in document order, handles paragraph boundaries, and decodes XML entities without extracting files to disk.
- **Layer Diagnostics & Page Detection:** Uses `lopdf::Document::load` to retrieve document page counts. Distinguishes genuine 0-byte/empty files from scanned or image-only documents lacking a digital text layer, providing structured diagnostic notices for the UI without silent empty reporting.
- **Sandbox Boundary Enforcement:** Validates `clean_relative` against canonical test root before reading. Rejects path escaping.

### 2.5 Local Search & Indexing Engine (`src-tauri/src/search.rs`) — *Implemented*
- **Tantivy Index Schema:**
  - `id` (STRING, STORED): Unique document identifier mapped to relative path.
  - `relative_path` (STRING, STORED): Relative path within sandbox.
  - `filename` (TEXT, STORED): Multi-value indexed field containing both literal filename and decomposed subwords for camelCase/alphanumeric keyword searchability.
  - `extension` (STRING, STORED): File extension for metadata display.
  - `body` (TEXT, STORED): Extracted text content tokenized with Tantivy's default text analyzer.
- **Isolated Index Storage:**
  - Stored in `<project-root>/.tantivy-index/` (ignored by Git, completely separated from `test-files/` and user folders).
- **Subword & CamelCase Tokenizer Decomposition:**
  - Custom `decompose_filename()` breaks acronyms, camelCase, and alphanumeric transitions (e.g., `DSAssignment07` -> `DS Assignment 07`), enabling natural search queries (e.g. `assignment`) to match compound filenames.
- **Windows File Locking & Concurrency Protection:**
  - Synchronized via `SearchIndexManager` holding an exclusive write lock.
  - Uses `ReloadPolicy::Manual` on `IndexReader` to eliminate unmanaged background poll threads.
  - Gracefully awaits merge threads (`wait_merging_threads()`) and consumes the `IndexWriter` before reader reload.
  - Implements exponential retry backoff for transient Windows OS Error 5 (`PermissionDenied`) file system contention (antivirus/filter driver scanning).
  - Clean rebuild uses `delete_all_documents()` before re-indexing, preventing duplicate hits while respecting Windows memory-mapped file locking (`MmapDirectory`).
- **Query & Snippet Engine:**
  - Multi-field BM25 query parsing across `filename` and `body`.
  - Snippet generation via `tantivy::snippet::SnippetGenerator` with 160-character highlighted terms (`<b>` tags).
  - Gracefully handles empty queries and syntax errors without crashing.

### 2.6 Desktop Shell Integration & Safe File Actions (`src-tauri/src/lib.rs`) — *Implemented*
- **Path Validation Guard (`validate_test_file_path`):** Resolves candidate relative paths against canonical test root, verifies `.starts_with(&canonical_root)`, rejects symlinks and Windows reparse points, and verifies regular file existence.
- **Safe Open Action (`open_test_file`):** Invokes Windows default application handler using `rundll32.exe url.dll,FileProtocolHandler <path>` with separate argument passing, avoiding `cmd.exe` or PowerShell shell concatenation.
- **Safe Reveal Action (`reveal_test_file`):** Highlights file in Windows File Explorer using `explorer.exe /select,"<canonical-path>"` with separate argv parameters.

---

## 3. Planned Architecture (Phases 7 – 12)

### 3.1 Future Document Extractors — *Planned: Phase 8*
- Dedicated implementations behind `DocumentExtractor`:
  - `PptxExtractor` (`.pptx`): ZIP reader extracting slide XML files.
  - `XlsxExtractor` (`.xlsx`): Sheet iterator using `calamine`.

### 3.4 Local Metadata Database (`database/`) — *Planned: Phase 3+*
- Embedded **SQLite** via `rusqlite` using WAL mode:
  ```sql
  CREATE TABLE IF NOT EXISTS file_records (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      canonical_path TEXT UNIQUE NOT NULL,
      filename TEXT NOT NULL,
      extension TEXT,
      size_bytes INTEGER NOT NULL,
      created_at INTEGER,
      modified_at INTEGER NOT NULL,
      content_hash TEXT,
      extraction_status TEXT NOT NULL, -- 'pending', 'indexed', 'failed', 'unsupported'
      error_message TEXT,
      last_scanned_at INTEGER NOT NULL
  );

  CREATE TABLE IF NOT EXISTS scan_roots (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      path TEXT UNIQUE NOT NULL,
      is_active INTEGER NOT NULL DEFAULT 1,
      last_scan_completed_at INTEGER
  );
  ```

### 3.5 Future Modules — *Planned: Phases 9, 10*
- **Local Semantic Search:** Embedded model execution via `fastembed` (running ONNX Runtime locally on CPU/DirectML). Embeddings stored in a local vector table or sqlite-vec. Zero remote API dependencies.
- **Optional Local OCR:** Tesseract CLI / library integration. Only run when enabled by the user for image/scanned PDF files.

---

## 4. Application Data Storage Layout

All application data is isolated within the platform-standard local AppData directory:
```text
%LOCALAPPDATA%\com.offlinefilefinder.app\
├── database\
│   └── metadata.sqlite
├── index\
│   └── tantivy\
│       ├── meta.json
│       └── *.bin
├── cache\
└── logs\
    └── app.log
```
*Note: Under no condition will databases, indexes, or temporary files be written to user scan folders.*

---

## 5. Security Boundaries & Filesystem Safety

1. **Absolute Read-Only Constraint:** The Rust scanner opens files exclusively with read-only flags (`std::fs::File::open`). No file modification APIs (`set_permissions`, `remove_file`, `rename`, `write`) are imported or used in scanner code.
2. **Development Sandbox:** During all development and automated testing, the scanner is hard-coded to reject any root other than `<project-root>\test-files\`.
3. **No Shell Injection:** Opening files or revealing paths in Explorer is executed via standard Windows API (`ShellExecuteExW` / `explorer.exe /select,"<path>"`) using verified canonical paths, never constructed via untrusted shell string concatenation.
4. **Air-Gapped Operation:** No HTTP client libraries (`reqwest`, `hyper`, etc.) are linked in the application runtime.
