# Product Requirements Document (PRD) — Offline File Finder

## 1. Product Overview

**Offline File Finder** is a commercial Windows desktop application designed to help users quickly and privately discover files stored on their computers by searching for file names, extracted full-text content, and semantic meaning.

### Core Philosophy & Principles
1. **100% Offline & Private:** Zero cloud LLMs, zero remote inference APIs, zero telemetry, and zero network data transmission.
2. **Read-Only with Respect to User Files:** The application treats all scanned files strictly as read-only inputs. It must **never** modify, rename, move, overwrite, clean up, or delete user documents under any circumstance.
3. **Data Separation:** All search indexes, SQLite databases, caches, metadata, and application logs must reside strictly in application-owned directories (`%LOCALAPPDATA%\com.offlinefilefinder.app`), completely segregated from scanned user folders.
4. **Predictable & Recoverable:** Scanning and indexing operations must be safe, interruptible, robust against permission errors, and resilient across application restarts.
5. **No Claims Without Verification:** Features are only considered functional when backed by executed and passing tests.

---

## 2. Target Use Cases & Example Queries

Users commonly have unorganized folders (Downloads, Documents, Desktop) containing invoices, receipts, project documents, and certificates. Offline File Finder enables fast, local discovery via:
- **Filename Queries:** `"resume"`, `"college marksheet"`, `"Q3 report"`
- **Exact Keyword / Content Matches:** `"₹15,000"`, `"Total Amount Due"`, `"Machine Learning Assignment"`, `"Confidential"`
- **Metadata Filters:** By file extension (`.pdf`, `.docx`, `.xlsx`), modification date range, and size.
- **Future Semantic Searches:** Finding concepts without exact keyword matches (e.g., `"proof of graduation"` locating a file titled `Degree_Certificate_2023.pdf`).
- **File Actions:** Opening the found file in its default system viewer or revealing its containing folder in Windows File Explorer.

---

## 3. Scope & Feature Classification

### Implemented Features (Phase 0 & Phase 1)
- [x] **Windows Build Environment:** Rust 1.99.0 (`x86_64-pc-windows-msvc`), MSVC v143, Windows SDK 10.0.26100.0, Node.js v24.15, WebView2 runtime Evergreen.
- [x] **Desktop Application Shell:** Tauri 2 application running React 19 + TypeScript + Vite + Tailwind CSS.
- [x] **Tauri IPC Command:** Genuine backend communication via `get_app_status` returning operational status, Rust toolchain version, platform metadata, and offline verification.
- [x] **Initial UI:** Clean dark desktop interface with search bar placeholder, status pills, and empty state guidance.

### Planned Features (Phases 3 – 12)
- [ ] **Phase 3:** Safe test-folder scanner (`test-files/` boundary enforcement, metadata collection, traversal resilience).
- [ ] **Phase 4:** TXT and PDF text extraction behind a common extraction interface (`DocumentExtractor`).
- [ ] **Phase 5:** Tantivy local full-text keyword indexing and search.
- [ ] **Phase 6:** DOCX document extraction and Windows Explorer "Reveal in folder" action.
- [ ] **Phase 7:** PPTX and XLSX document extraction.
- [ ] **Phase 8:** Guided folder selection (Desktop, Documents, Downloads, custom folders) and safe background rescanning.
- [ ] **Phase 9:** Local semantic search via FastEmbed (ONNX runtime executing locally).
- [ ] **Phase 10:** Optional local OCR (Tesseract) and transparent document categorization.
- [ ] **Phase 11:** Production packaging, installer generation, and security auditing.

---

## 4. Functional Requirements

### 4.1 File Discovery & Traversal
- **Strict Boundary Enforcement:** Traversal must only proceed inside explicitly approved roots. Symbolic links, NTFS junctions, and reparse points must be skipped by default to prevent directory loops and escaping the target root.
- **Metadata Collection:** For every discovered file, collect: canonical path, file name, extension, file size in bytes, creation time, and last modified time.
- **Fault Tolerance:** Permission denied (`UnauthorizedAccess`), path length limits, locked files, and I/O errors must be gracefully recorded as file-level warnings without terminating the scan.

### 4.2 Document Text Extraction
- Support extraction through a unified Rust trait (`DocumentExtractor`):
  - **Plain Text (`.txt`):** Native Rust reading with UTF-8 / fallback encoding handling.
  - **PDF (`.pdf`):** Stream text extraction (`pdf-extract` / `lopdf`).
  - **Word (`.docx`):** ZIP archive extraction parsing `word/document.xml`.
  - **PowerPoint (`.pptx`):** ZIP archive extraction parsing slide XML files.
  - **Excel (`.xlsx`):** Sheet and cell data extraction using `calamine`.
- **Extraction Limits:** Maximum extractable file size thresholds and per-file timeouts to prevent unbounded memory usage on massive archives or corrupted files.

### 4.3 Search & Indexing Engine
- **Engine:** Tantivy embedded full-text search engine.
- **Fields:** File ID (fast lookup), file name (tokenized + raw for exact match), file path, extension, file size, timestamps, extracted body text.
- **Query Support:** Term queries, phrase queries, boolean operators (`AND`, `OR`), prefix matching, and date/size range filters.
- **Consistency:** Search indexes must remain synchronized with the SQLite metadata store. Stale records must only be expunged after a complete, verified rescan confirms the file no longer exists.

### 4.4 Local Metadata Database (SQLite)
- Persist structured file records, scan root configurations, indexing status, file hash / change signatures, and error diagnostics.
- Ensure all database writes use atomic transactions with WAL (Write-Ahead Logging) mode enabled for high concurrency.

### 4.5 User Interface
- Built with React 19, TypeScript, and Tailwind CSS.
- **Key Views:**
  - **Dashboard / Search:** Immediate search input, filter pills, and live result list with relevance scoring.
  - **File Result Cards:** Filename, highlighted content snippet, path, size, filetype icon, and action buttons ("Open File", "Show in Folder").
  - **Scan Status & Controls:** Scan progress bar, discovered count, indexed count, errors drawer, and pause/resume buttons.
  - **Settings:** Configured scan folders list, rescan intervals, and index management (Rebuild Index, Clear Cache).

---

## 5. Non-Functional Requirements

| Requirement | Metric / Standard |
| :--- | :--- |
| **Network & Privacy** | Exactly zero outbound network connections. No telemetry or cloud analytics. |
| **Filesystem Safety** | Original files are 100% read-only. No write or delete operations permitted. |
| **Search Latency** | Full-text query response under 50ms for libraries up to 50,000 documents. |
| **Memory Footprint** | Background idle memory < 100 MB; active scanning memory < 350 MB. |
| **Reliability** | Corrupt files, binary junk, or zero-byte files must not crash the app or stop scans. |
| **Platform Target** | Windows 10 (version 1809+) and Windows 11 on x86_64 architecture. |

---

## 6. Acceptance Criteria

1. **Safety Boundary:** During development, scanner tests run solely within `<project-root>/test-files/`. Attempting to scan outside returns an explicit validation error.
2. **Crash Resilience:** Corrupted PDFs or password-protected Office files log an extraction failure in the database, while the remaining files continue scanning unaffected.
3. **Accuracy:** Full-text searches for unique terms inside indexed documents return the matching file in the top results within 50ms.
4. **Offline Isolation:** Network monitoring shows 0 packets transmitted during application lifecycle.
