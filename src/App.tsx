import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Search,
  Shield,
  HardDrive,
  Cpu,
  AlertCircle,
  CheckCircle2,
  Clock,
  FolderCheck,
  RefreshCw,
  FileText,
  FileCode,
  FolderOpen,
  Info,
  FileSpreadsheet,
  Copy,
  Check,
  X,
  Database,
  Sparkles,
  ExternalLink,
} from "lucide-react";

interface AppStatus {
  status: string;
  message: string;
  version: string;
  rust_version: string;
  os: string;
  offline_verified: boolean;
}

interface ScannedFileRecord {
  filename: string;
  relative_path: string;
  extension: string | null;
  size_bytes: number;
  modified_timestamp_ms: number | null;
  is_readonly: boolean;
}

interface ScanResult {
  success: boolean;
  root_display: string;
  files: ScannedFileRecord[];
  file_count: usize;
  directory_count: usize;
  warnings: string[];
  scan_duration_ms: number;
}

interface ExtractionResponse {
  success: boolean;
  relative_path: string;
  filename: string;
  extension: string;
  extracted_text: string;
  character_count: number;
  file_size_bytes: number;
  page_count: number | null;
  has_text_layer: boolean;
  diagnostic_notice: string | null;
  duration_ms: number;
  is_empty: boolean;
}

interface IndexSummary {
  success: boolean;
  indexed_count: number;
  skipped_count: number;
  failed_count: number;
  index_directory: string;
  duration_ms: number;
}

interface SearchHit {
  filename: string;
  relative_path: string;
  extension: string;
  score: number;
  snippet: string;
}

interface SearchResultResponse {
  success: boolean;
  query: string;
  total_hits: number;
  results: SearchHit[];
  duration_ms: number;
}

type usize = number;

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatTimestamp(timestampMs: number | null): string {
  if (!timestampMs) return "Unknown";
  const date = new Date(timestampMs);
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export default function App() {
  // Application backend status state
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [statusLoading, setStatusLoading] = useState(true);
  const [statusError, setStatusError] = useState<string | null>(null);

  // Tantivy Indexing state
  const [indexLoading, setIndexLoading] = useState(false);
  const [indexSummary, setIndexSummary] = useState<IndexSummary | null>(null);
  const [indexError, setIndexError] = useState<string | null>(null);

  // Tantivy Search state
  const [searchQuery, setSearchQuery] = useState("");
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchResult, setSearchResult] = useState<SearchResultResponse | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);

  // Scanner state
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);
  const [scanLoading, setScanLoading] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);

  // Document Text Extractor state
  const [extractingPath, setExtractingPath] = useState<string | null>(null);
  const [extractionResult, setExtractionResult] = useState<ExtractionResponse | null>(null);
  const [extractionError, setExtractionError] = useState<string | null>(null);
  const [copiedText, setCopiedText] = useState(false);
  const [fileActionNotice, setFileActionNotice] = useState<{
    type: "success" | "error";
    message: string;
  } | null>(null);

  // Query app status on startup
  useEffect(() => {
    async function fetchStatus() {
      try {
        setStatusLoading(true);
        setStatusError(null);
        const res = await invoke<AppStatus>("get_app_status");
        setStatus(res);
      } catch (err) {
        console.error("Failed to invoke get_app_status:", err);
        setStatusError(
          err instanceof Error
            ? err.message
            : typeof err === "string"
            ? err
            : "Failed to connect to Rust backend IPC"
        );
      } finally {
        setStatusLoading(false);
      }
    }

    fetchStatus();
  }, []);

  // Debounced search when query changes (only when index exists)
  useEffect(() => {
    if (!indexSummary) return;
    const trimmed = searchQuery.trim();
    if (!trimmed) {
      setSearchResult(null);
      setSearchError(null);
      return;
    }

    const timer = setTimeout(() => {
      performSearch(trimmed);
    }, 300);

    return () => clearTimeout(timer);
  }, [searchQuery, indexSummary]);

  // Trigger Tantivy indexing pipeline
  async function handleIndexTestFiles() {
    try {
      setIndexLoading(true);
      setIndexError(null);
      const res = await invoke<IndexSummary>("index_test_files");
      setIndexSummary(res);
      // If a query is already entered, trigger search immediately
      if (searchQuery.trim()) {
        await performSearch(searchQuery.trim());
      }
    } catch (err) {
      console.error("Failed to invoke index_test_files:", err);
      setIndexError(
        err instanceof Error
          ? err.message
          : typeof err === "string"
          ? err
          : "Failed to index test files"
      );
    } finally {
      setIndexLoading(false);
    }
  }

  // Execute Tantivy search
  async function performSearch(queryToSearch: string) {
    if (!queryToSearch.trim()) {
      setSearchResult(null);
      return;
    }

    try {
      setSearchLoading(true);
      setSearchError(null);
      const res = await invoke<SearchResultResponse>("search_test_files", {
        query: queryToSearch,
      });
      setSearchResult(res);
    } catch (err) {
      console.error("Failed to invoke search_test_files:", err);
      setSearchError(
        err instanceof Error
          ? err.message
          : typeof err === "string"
          ? err
          : "Search failed"
      );
    } finally {
      setSearchLoading(false);
    }
  }

  function handleSearchSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!indexSummary) {
      setSearchError("Please index test files first by clicking 'Index Test Files'.");
      return;
    }
    performSearch(searchQuery.trim());
  }

  // Trigger safe test-folder scan via Tauri IPC
  async function handleScanTestDirectory() {
    try {
      setScanLoading(true);
      setScanError(null);
      const res = await invoke<ScanResult>("scan_test_directory");
      setScanResult(res);
    } catch (err) {
      console.error("Failed to invoke scan_test_directory:", err);
      setScanError(
        err instanceof Error
          ? err.message
          : typeof err === "string"
          ? err
          : "Failed to scan test directory"
      );
    } finally {
      setScanLoading(false);
    }
  }

  // Trigger text extraction on a supported file via Tauri IPC
  async function handleExtractText(relativePath: string) {
    try {
      setExtractingPath(relativePath);
      setExtractionError(null);
      setExtractionResult(null);
      setCopiedText(false);

      const res = await invoke<ExtractionResponse>("extract_test_file", {
        relativePath,
      });
      setExtractionResult(res);
    } catch (err) {
      console.error("Failed to extract file:", err);
      setExtractionError(
        err instanceof Error
          ? err.message
          : typeof err === "string"
          ? err
          : "Failed to extract text from file"
      );
    } finally {
      setExtractingPath(null);
    }
  }

  function handleCopyText(text: string) {
    navigator.clipboard.writeText(text);
    setCopiedText(true);
    setTimeout(() => setCopiedText(false), 2000);
  }

  async function handleOpenFile(relativePath: string) {
    try {
      setFileActionNotice(null);
      await invoke("open_test_file", { relativePath });
      setFileActionNotice({
        type: "success",
        message: `Opened "${relativePath}" with default application`,
      });
      setTimeout(() => setFileActionNotice(null), 3500);
    } catch (err) {
      console.error("Failed to open file:", err);
      setFileActionNotice({
        type: "error",
        message:
          err instanceof Error
            ? err.message
            : typeof err === "string"
            ? err
            : "Failed to open file",
      });
    }
  }

  async function handleRevealFile(relativePath: string) {
    try {
      setFileActionNotice(null);
      await invoke("reveal_test_file", { relativePath });
      setFileActionNotice({
        type: "success",
        message: `Revealed "${relativePath}" in Windows File Explorer`,
      });
      setTimeout(() => setFileActionNotice(null), 3500);
    } catch (err) {
      console.error("Failed to reveal file:", err);
      setFileActionNotice({
        type: "error",
        message:
          err instanceof Error
            ? err.message
            : typeof err === "string"
            ? err
            : "Failed to reveal file in folder",
      });
    }
  }

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100 flex flex-col font-sans">
      {/* Top Header Bar */}
      <header className="border-b border-slate-800 bg-slate-900/60 backdrop-blur-md px-6 py-4 flex items-center justify-between sticky top-0 z-10">
        <div className="flex items-center space-x-3">
          <div className="p-2 bg-emerald-500/10 border border-emerald-500/20 rounded-lg text-emerald-400">
            <HardDrive className="w-5 h-5" />
          </div>
          <div>
            <h1 className="text-lg font-bold tracking-tight text-white flex items-center gap-2">
              Offline File Finder
              <span className="text-[10px] font-medium tracking-wide uppercase px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                Phase 5 Tantivy Search
              </span>
            </h1>
            <p className="text-xs text-slate-400">
              Find your files locally. Your files stay on your computer.
            </p>
          </div>
        </div>

        {/* Backend Status Indicators */}
        <div className="flex items-center space-x-2">
          {statusLoading && (
            <div className="flex items-center space-x-2 text-xs text-amber-400 bg-amber-400/10 border border-amber-400/20 px-3 py-1.5 rounded-full">
              <Clock className="w-3.5 h-3.5 animate-spin" />
              <span>Connecting to Rust...</span>
            </div>
          )}

          {!statusLoading && status && (
            <div className="flex items-center space-x-2 text-xs text-emerald-400 bg-emerald-400/10 border border-emerald-400/20 px-3 py-1.5 rounded-full">
              <CheckCircle2 className="w-3.5 h-3.5" />
              <span className="font-medium">Rust IPC: Operational</span>
            </div>
          )}

          {!statusLoading && statusError && (
            <div className="flex items-center space-x-2 text-xs text-rose-400 bg-rose-400/10 border border-rose-400/20 px-3 py-1.5 rounded-full">
              <AlertCircle className="w-3.5 h-3.5" />
              <span className="font-medium">IPC Error</span>
            </div>
          )}

          <div className="flex items-center space-x-1.5 text-xs text-slate-400 bg-slate-800/80 border border-slate-700/60 px-3 py-1.5 rounded-full">
            <Shield className="w-3.5 h-3.5 text-emerald-400" />
            <span>100% Offline</span>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1 max-w-5xl w-full mx-auto p-6 space-y-6">
        {/* File Action Feedback Banner */}
        {fileActionNotice && (
          <div
            className={`p-3.5 rounded-xl border flex items-center justify-between text-xs transition-all shadow-sm ${
              fileActionNotice.type === "success"
                ? "bg-emerald-500/10 border-emerald-500/20 text-emerald-300"
                : "bg-rose-500/10 border-rose-500/20 text-rose-300"
            }`}
          >
            <div className="flex items-center space-x-2">
              {fileActionNotice.type === "success" ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
              ) : (
                <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
              )}
              <span className="font-medium">{fileActionNotice.message}</span>
            </div>
            <button
              onClick={() => setFileActionNotice(null)}
              className="p-1 hover:bg-slate-800 rounded text-slate-400 hover:text-slate-200 cursor-pointer"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {/* SECTION: Tantivy Local Keyword Search */}
        <section className="bg-slate-900/60 border border-slate-800 rounded-2xl p-6 shadow-sm space-y-5">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-4 border-b border-slate-800/80">
            <div>
              <div className="flex items-center space-x-2">
                <Database className="w-5 h-5 text-emerald-400" />
                <h2 className="text-base font-semibold text-slate-100">
                  Tantivy Local Keyword Search Engine
                </h2>
                <span className="text-[11px] font-mono px-2 py-0.5 rounded bg-slate-800 text-emerald-300 border border-slate-700">
                  BM25 Index
                </span>
              </div>
              <p className="text-xs text-slate-400 mt-1">
                Real local full-text search indexing filenames and extracted text of supported{" "}
                <span className="text-emerald-400 font-mono">.txt</span>,{" "}
                <span className="text-rose-400 font-mono">.pdf</span>, and{" "}
                <span className="text-blue-400 font-mono">.docx</span> fixtures.
              </p>
            </div>

            {/* Explicit Index Test Files Button */}
            <button
              id="btn-index-test-files"
              onClick={handleIndexTestFiles}
              disabled={indexLoading}
              className="inline-flex items-center justify-center space-x-2 px-4 py-2.5 bg-emerald-600 hover:bg-emerald-500 active:bg-emerald-700 disabled:opacity-50 text-white font-medium text-xs rounded-xl shadow transition-colors cursor-pointer shrink-0"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${indexLoading ? "animate-spin" : ""}`} />
              <span>{indexLoading ? "Indexing Sandbox..." : "Index Test Files"}</span>
            </button>
          </div>

          {/* Indexing Error Banner */}
          {indexError && (
            <div className="p-4 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-300 text-xs flex items-start space-x-3">
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5 text-rose-400" />
              <div>
                <p className="font-semibold text-rose-200">Indexing Operation Failed</p>
                <p className="font-mono text-rose-300/90 text-[11px] mt-0.5">{indexError}</p>
              </div>
            </div>
          )}

          {/* Indexing Summary Metrics (When Built) */}
          {indexSummary && (
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs">
              <div className="bg-slate-950/70 border border-slate-800 rounded-lg p-3">
                <span className="text-slate-500 block text-[11px]">Indexed Documents</span>
                <span className="text-base font-bold text-emerald-400 mt-0.5 block">
                  {indexSummary.indexed_count} files
                </span>
              </div>
              <div className="bg-slate-950/70 border border-slate-800 rounded-lg p-3">
                <span className="text-slate-500 block text-[11px]">Skipped (Unsupported)</span>
                <span className="text-base font-bold text-slate-300 mt-0.5 block">
                  {indexSummary.skipped_count} files
                </span>
              </div>
              <div className="bg-slate-950/70 border border-slate-800 rounded-lg p-3">
                <span className="text-slate-500 block text-[11px]">Extraction Failures</span>
                <span className="text-base font-bold text-amber-400 mt-0.5 block">
                  {indexSummary.failed_count} files
                </span>
              </div>
              <div className="bg-slate-950/70 border border-slate-800 rounded-lg p-3">
                <span className="text-slate-500 block text-[11px]">Index Duration</span>
                <span className="text-base font-bold text-slate-200 mt-0.5 block">
                  {indexSummary.duration_ms} ms
                </span>
              </div>
            </div>
          )}

          {/* Unindexed Reminder Notice Banner */}
          {!indexSummary && !indexLoading && (
            <div className="p-4 rounded-xl bg-slate-950/80 border border-dashed border-slate-700/80 text-xs flex items-center justify-between gap-4">
              <div className="flex items-center space-x-3">
                <Info className="w-5 h-5 text-amber-400 shrink-0" />
                <div>
                  <p className="font-semibold text-slate-200">Index Not Yet Built</p>
                  <p className="text-slate-400 text-[11px] mt-0.5">
                    Tantivy search requires an indexed document catalog. Click{" "}
                    <span className="text-emerald-400 font-medium">"Index Test Files"</span> to scan fixtures, extract text, and build the local Tantivy BM25 inverted index.
                  </p>
                </div>
              </div>
              <button
                onClick={handleIndexTestFiles}
                className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-emerald-400 text-xs font-medium rounded-lg border border-slate-700 transition-colors shrink-0 cursor-pointer"
              >
                Index Now
              </button>
            </div>
          )}

          {/* Search Input Form */}
          <form onSubmit={handleSearchSubmit} className="relative flex items-center gap-2">
            <div className="relative flex-1">
              <div className="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none text-slate-400">
                <Search className="w-4 h-4" />
              </div>
              <input
                id="search-input"
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder={
                  indexSummary
                    ? "Search terms (e.g., 'sample', 'discovery', 'synthetic', 'offline', 'document')..."
                    : "Index files first to enable search..."
                }
                className="w-full pl-11 pr-10 py-2.5 bg-slate-950 border border-slate-800 rounded-xl text-slate-100 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 focus:border-emerald-500 transition-all text-xs shadow-inner"
              />
              {searchQuery && (
                <button
                  type="button"
                  onClick={() => {
                    setSearchQuery("");
                    setSearchResult(null);
                    setSearchError(null);
                  }}
                  className="absolute inset-y-0 right-0 pr-3 flex items-center text-slate-400 hover:text-slate-200 cursor-pointer"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              )}
            </div>

            <button
              id="btn-search"
              type="submit"
              disabled={searchLoading || !searchQuery.trim()}
              className="inline-flex items-center space-x-1.5 px-4 py-2.5 bg-slate-800 hover:bg-slate-700 disabled:opacity-40 text-emerald-400 font-medium text-xs rounded-xl border border-slate-700 transition-colors cursor-pointer shrink-0"
            >
              {searchLoading ? (
                <>
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                  <span>Searching...</span>
                </>
              ) : (
                <>
                  <Search className="w-3.5 h-3.5" />
                  <span>Search</span>
                </>
              )}
            </button>
          </form>

          {/* Search Error State */}
          {searchError && (
            <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/20 text-rose-300 text-xs flex items-start space-x-2">
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5 text-rose-400" />
              <div>
                <p className="font-semibold text-rose-200">Query Parsing / Search Error</p>
                <p className="font-mono text-[11px] mt-0.5">{searchError}</p>
              </div>
            </div>
          )}

          {/* Search Results Display Area */}
          {searchResult && (
            <div className="space-y-3 pt-2 border-t border-slate-800/80">
              <div className="flex items-center justify-between text-xs text-slate-400">
                <span className="flex items-center gap-1.5">
                  <Sparkles className="w-3.5 h-3.5 text-emerald-400" />
                  <span>
                    Found <strong className="text-emerald-400">{searchResult.total_hits}</strong> matching document{searchResult.total_hits === 1 ? "" : "s"} for{" "}
                    <span className="font-mono text-slate-200">"{searchResult.query}"</span>
                  </span>
                </span>
                <span className="text-[11px] font-mono text-slate-500">
                  {searchResult.duration_ms} ms
                </span>
              </div>

              {/* Zero Hits Empty State */}
              {searchResult.results.length === 0 ? (
                <div className="py-8 text-center text-xs text-slate-400 border border-dashed border-slate-800 rounded-xl bg-slate-950/40">
                  <p className="font-medium text-slate-300">No matching files found</p>
                  <p className="text-slate-500 mt-1">
                    No documents matched your query in filenames or extracted text.
                  </p>
                </div>
              ) : (
                /* Results List Cards */
                <div className="space-y-2.5">
                  {searchResult.results.map((hit, idx) => (
                    <div
                      key={idx}
                      className="p-3.5 rounded-xl bg-slate-950/70 border border-slate-800/90 hover:border-slate-700 transition-all text-xs space-y-2"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <div className="flex items-center space-x-2 truncate">
                          {hit.extension === "pdf" ? (
                            <FileSpreadsheet className="w-4 h-4 text-rose-400 shrink-0" />
                          ) : hit.extension === "docx" ? (
                            <FileText className="w-4 h-4 text-blue-400 shrink-0" />
                          ) : (
                            <FileText className="w-4 h-4 text-emerald-400 shrink-0" />
                          )}
                          <span className="font-semibold text-slate-200 truncate">
                            {hit.filename}
                          </span>
                          <span className="font-mono text-[11px] text-slate-400 bg-slate-900 px-2 py-0.5 rounded border border-slate-800 truncate">
                            {hit.relative_path}
                          </span>
                        </div>

                        <div className="flex items-center space-x-2 shrink-0">
                          <span className="text-[10px] font-mono uppercase px-1.5 py-0.5 rounded bg-slate-900 text-slate-400 border border-slate-800">
                            {hit.extension}
                          </span>
                          <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                            BM25: {hit.score.toFixed(2)}
                          </span>
                          <button
                            onClick={() => handleOpenFile(hit.relative_path)}
                            title="Open file in default application"
                            className="inline-flex items-center space-x-1 px-2 py-0.5 bg-slate-800 hover:bg-slate-700 active:bg-slate-600 text-sky-400 hover:text-sky-300 text-[11px] font-medium rounded border border-slate-700 transition-colors cursor-pointer"
                          >
                            <ExternalLink className="w-3 h-3" />
                            <span>Open</span>
                          </button>
                          <button
                            onClick={() => handleRevealFile(hit.relative_path)}
                            title="Show in Windows File Explorer"
                            className="inline-flex items-center space-x-1 px-2 py-0.5 bg-slate-800 hover:bg-slate-700 active:bg-slate-600 text-amber-400 hover:text-amber-300 text-[11px] font-medium rounded border border-slate-700 transition-colors cursor-pointer"
                          >
                            <FolderOpen className="w-3 h-3" />
                            <span>Folder</span>
                          </button>
                          <button
                            onClick={() => handleExtractText(hit.relative_path)}
                            className="px-2 py-0.5 bg-slate-800 hover:bg-slate-700 text-emerald-400 text-[11px] font-medium rounded border border-slate-700 transition-colors cursor-pointer"
                          >
                            View Extracted Text
                          </button>
                        </div>
                      </div>

                      {/* Content Snippet */}
                      {hit.snippet && (
                        <div className="mt-1.5 text-xs text-slate-300 font-mono bg-slate-900/90 p-2.5 rounded-lg border border-slate-800/80 leading-relaxed search-snippet select-text">
                          <p
                            dangerouslySetInnerHTML={{
                              __html: hit.snippet,
                            }}
                          />
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </section>

        {/* SECTION: Safe Test-Folder Scanner */}
        <section className="bg-slate-900/40 border border-slate-800 rounded-2xl p-6 shadow-sm">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-5 border-b border-slate-800/80">
            <div>
              <div className="flex items-center space-x-2">
                <FolderCheck className="w-5 h-5 text-emerald-400" />
                <h2 className="text-base font-semibold text-slate-100">
                  Safe Test-Folder Scanner &amp; Text Extractor
                </h2>
                <span className="text-[11px] font-mono px-2 py-0.5 rounded bg-slate-800 text-slate-300 border border-slate-700">
                  test-files/ Sandbox
                </span>
              </div>
              <p className="text-xs text-slate-400 mt-1">
                Discovers files and extracts raw text from <span className="text-emerald-400 font-mono">.txt</span> and <span className="text-emerald-400 font-mono">.pdf</span> documents using safe native Rust extractors.
              </p>
            </div>

            <button
              id="btn-scan-test-folder"
              onClick={handleScanTestDirectory}
              disabled={scanLoading}
              className="inline-flex items-center justify-center space-x-2 px-4 py-2.5 bg-slate-800 hover:bg-slate-700 active:bg-slate-600 disabled:opacity-50 text-slate-200 font-medium text-xs rounded-xl border border-slate-700 shadow transition-colors cursor-pointer shrink-0"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${scanLoading ? "animate-spin" : ""}`} />
              <span>{scanLoading ? "Scanning Sandbox..." : "Scan Test Folder"}</span>
            </button>
          </div>

          {/* Scanner Loading State */}
          {scanLoading && (
            <div className="py-12 flex flex-col items-center justify-center space-y-3 text-center">
              <RefreshCw className="w-7 h-7 text-emerald-400 animate-spin" />
              <div className="space-y-1">
                <p className="text-sm font-medium text-slate-200">Executing Safe Directory Traversal</p>
                <p className="text-xs text-slate-400 font-mono">
                  Enforcing boundary checks and collecting file metadata...
                </p>
              </div>
            </div>
          )}

          {/* Scanner Error State */}
          {!scanLoading && scanError && (
            <div className="mt-4 p-4 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-300 text-xs flex items-start space-x-3">
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5 text-rose-400" />
              <div className="space-y-1">
                <p className="font-semibold text-rose-200">Scan Failed / Boundary Error</p>
                <p className="font-mono text-rose-300/90">{scanError}</p>
              </div>
            </div>
          )}

          {/* Scanner Initial Empty State */}
          {!scanLoading && !scanError && !scanResult && (
            <div className="py-10 text-center flex flex-col items-center justify-center space-y-3">
              <div className="w-12 h-12 rounded-xl bg-slate-800/60 border border-slate-700/50 flex items-center justify-center text-slate-400">
                <FolderOpen className="w-6 h-6" />
              </div>
              <div className="max-w-md space-y-1">
                <p className="text-sm font-medium text-slate-200">Ready to Scan Test Fixtures</p>
                <p className="text-xs text-slate-400">
                  Click <span className="text-emerald-400 font-medium">"Scan Test Folder"</span> to discover test files and view metadata.
                </p>
              </div>
            </div>
          )}

          {/* Scanner Results Display */}
          {!scanLoading && !scanError && scanResult && (
            <div className="mt-5 space-y-4">
              {/* Scan Metrics Bar */}
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs">
                <div className="bg-slate-950/70 border border-slate-800 rounded-lg p-3">
                  <span className="text-slate-500 block text-[11px]">Discovered Files</span>
                  <span className="text-base font-bold text-emerald-400 mt-0.5 block">
                    {scanResult.file_count}
                  </span>
                </div>
                <div className="bg-slate-950/70 border border-slate-800 rounded-lg p-3">
                  <span className="text-slate-500 block text-[11px]">Subdirectories Scanned</span>
                  <span className="text-base font-bold text-slate-200 mt-0.5 block">
                    {scanResult.directory_count}
                  </span>
                </div>
                <div className="bg-slate-950/70 border border-slate-800 rounded-lg p-3">
                  <span className="text-slate-500 block text-[11px]">Scan Duration</span>
                  <span className="text-base font-bold text-slate-200 mt-0.5 block">
                    {scanResult.scan_duration_ms} ms
                  </span>
                </div>
                <div className="bg-slate-950/70 border border-slate-800 rounded-lg p-3">
                  <span className="text-slate-500 block text-[11px]">Sandbox Root</span>
                  <span className="text-xs font-mono text-slate-300 mt-1 block truncate" title={scanResult.root_display}>
                    test-files
                  </span>
                </div>
              </div>

              {/* Traversal Notices */}
              {scanResult.warnings.length > 0 && (
                <div className="p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg text-xs text-amber-300 space-y-1">
                  <div className="font-semibold flex items-center gap-1.5">
                    <Info className="w-3.5 h-3.5" />
                    <span>Traversal Notices ({scanResult.warnings.length})</span>
                  </div>
                  <ul className="list-disc list-inside text-amber-200/90 text-[11px] space-y-0.5">
                    {scanResult.warnings.map((w, idx) => (
                      <li key={idx} className="font-mono">{w}</li>
                    ))}
                  </ul>
                </div>
              )}

              {/* Discovered Files Table with Extraction Actions */}
              {scanResult.files.length === 0 ? (
                <div className="p-6 text-center text-xs text-slate-400 border border-dashed border-slate-800 rounded-xl">
                  No files were found inside the test directory.
                </div>
              ) : (
                <div className="border border-slate-800 rounded-xl overflow-hidden bg-slate-950/40">
                  <div className="overflow-x-auto">
                    <table className="w-full text-left text-xs">
                      <thead className="bg-slate-900/90 border-b border-slate-800 text-slate-400 uppercase text-[10px] tracking-wider">
                        <tr>
                          <th className="py-2.5 px-4">Filename</th>
                          <th className="py-2.5 px-4">Relative Path</th>
                          <th className="py-2.5 px-3">Type</th>
                          <th className="py-2.5 px-3">Size</th>
                          <th className="py-2.5 px-4">Last Modified</th>
                          <th className="py-2.5 px-4 text-right">Actions</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-slate-800/60 font-sans">
                        {scanResult.files.map((file, idx) => {
                          const ext = (file.extension || "").toLowerCase();
                          const isSupported = ext === "txt" || ext === "pdf" || ext === "docx";
                          const isExtracting = extractingPath === file.relative_path;

                          return (
                            <tr key={idx} className="hover:bg-slate-900/40 transition-colors">
                              <td className="py-2.5 px-4 font-medium text-slate-200 flex items-center space-x-2">
                                {ext === "pdf" ? (
                                  <FileSpreadsheet className="w-4 h-4 text-rose-400 shrink-0" />
                                ) : ext === "docx" ? (
                                  <FileText className="w-4 h-4 text-blue-400 shrink-0" />
                                ) : ext === "txt" ? (
                                  <FileText className="w-4 h-4 text-emerald-400 shrink-0" />
                                ) : (
                                  <FileCode className="w-4 h-4 text-slate-400 shrink-0" />
                                )}
                                <span className="truncate max-w-[160px]">{file.filename}</span>
                              </td>
                              <td className="py-2.5 px-4 font-mono text-[11px] text-slate-300">
                                <span className="bg-slate-900 px-2 py-0.5 rounded border border-slate-800">
                                  {file.relative_path}
                                </span>
                              </td>
                              <td className="py-2.5 px-3 text-slate-400 uppercase text-[11px] font-mono">
                                {file.extension || "none"}
                              </td>
                              <td className="py-2.5 px-3 font-mono text-slate-300">
                                {formatBytes(file.size_bytes)}
                              </td>
                              <td className="py-2.5 px-4 text-slate-400 text-[11px]">
                                {formatTimestamp(file.modified_timestamp_ms)}
                              </td>
                              <td className="py-2.5 px-4 text-right">
                                <div className="flex items-center justify-end space-x-1.5">
                                  <button
                                    onClick={() => handleOpenFile(file.relative_path)}
                                    title="Open file in default application"
                                    className="p-1 bg-slate-800 hover:bg-slate-700 active:bg-slate-600 text-slate-300 hover:text-sky-300 rounded border border-slate-700 transition-colors cursor-pointer"
                                  >
                                    <ExternalLink className="w-3.5 h-3.5 text-sky-400" />
                                  </button>
                                  <button
                                    onClick={() => handleRevealFile(file.relative_path)}
                                    title="Show in Windows File Explorer"
                                    className="p-1 bg-slate-800 hover:bg-slate-700 active:bg-slate-600 text-slate-300 hover:text-amber-300 rounded border border-slate-700 transition-colors cursor-pointer"
                                  >
                                    <FolderOpen className="w-3.5 h-3.5 text-amber-400" />
                                  </button>
                                  {isSupported ? (
                                    <button
                                      onClick={() => handleExtractText(file.relative_path)}
                                      disabled={isExtracting}
                                      className="inline-flex items-center space-x-1.5 px-2.5 py-1 bg-slate-800 hover:bg-slate-700 active:bg-slate-600 disabled:opacity-50 text-emerald-400 hover:text-emerald-300 font-medium text-[11px] rounded-lg border border-slate-700 transition-colors cursor-pointer"
                                    >
                                      {isExtracting ? (
                                        <>
                                          <RefreshCw className="w-3 h-3 animate-spin" />
                                          <span>Extracting...</span>
                                        </>
                                      ) : (
                                        <>
                                          <FileText className="w-3 h-3" />
                                          <span>Extract Text</span>
                                        </>
                                      )}
                                    </button>
                                  ) : (
                                    <span className="text-[10px] text-slate-500 italic">
                                      Unsupported
                                    </span>
                                  )}
                                </div>
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* EXTRACTED TEXT PREVIEW PANEL */}
          {(extractionResult || extractionError || extractingPath) && (
            <div className="mt-6 border border-slate-800 rounded-xl bg-slate-950/70 p-4 space-y-3">
              <div className="flex items-center justify-between pb-3 border-b border-slate-800/80">
                <div className="flex items-center space-x-2">
                  <FileText className="w-4 h-4 text-emerald-400" />
                  <span className="text-xs font-semibold uppercase tracking-wider text-slate-300">
                    Document Text Extraction Result
                  </span>
                  {extractionResult && (
                    <span className="text-[11px] font-mono px-2 py-0.5 rounded bg-slate-900 text-emerald-300 border border-slate-800">
                      {extractionResult.filename}
                    </span>
                  )}
                </div>

                <div className="flex items-center space-x-2">
                  {extractionResult && (
                    <>
                      <button
                        onClick={() => handleOpenFile(extractionResult.relative_path)}
                        title="Open file in default application"
                        className="inline-flex items-center space-x-1 text-xs text-slate-300 hover:text-sky-300 bg-slate-900 hover:bg-slate-800 border border-slate-700 px-2.5 py-1 rounded-md transition-colors cursor-pointer"
                      >
                        <ExternalLink className="w-3 h-3 text-sky-400" />
                        <span>Open</span>
                      </button>
                      <button
                        onClick={() => handleRevealFile(extractionResult.relative_path)}
                        title="Show in Windows File Explorer"
                        className="inline-flex items-center space-x-1 text-xs text-slate-300 hover:text-amber-300 bg-slate-900 hover:bg-slate-800 border border-slate-700 px-2.5 py-1 rounded-md transition-colors cursor-pointer"
                      >
                        <FolderOpen className="w-3 h-3 text-amber-400" />
                        <span>Folder</span>
                      </button>
                      <button
                        onClick={() => handleCopyText(extractionResult.extracted_text)}
                        className="inline-flex items-center space-x-1 text-xs text-slate-300 hover:text-white bg-slate-900 hover:bg-slate-800 border border-slate-700 px-2.5 py-1 rounded-md transition-colors cursor-pointer"
                      >
                        {copiedText ? (
                          <>
                            <Check className="w-3 h-3 text-emerald-400" />
                            <span className="text-emerald-400">Copied</span>
                          </>
                        ) : (
                          <>
                            <Copy className="w-3 h-3" />
                            <span>Copy</span>
                          </>
                        )}
                      </button>
                    </>
                  )}
                  <button
                    onClick={() => {
                      setExtractionResult(null);
                      setExtractionError(null);
                    }}
                    className="text-slate-400 hover:text-slate-200 p-1 hover:bg-slate-800 rounded transition-colors cursor-pointer"
                    title="Close Preview"
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>
              </div>

              {extractingPath && !extractionResult && !extractionError && (
                <div className="py-6 flex items-center justify-center space-x-2 text-xs text-slate-400">
                  <RefreshCw className="w-4 h-4 text-emerald-400 animate-spin" />
                  <span>Extracting text via Rust DocumentExtractor...</span>
                </div>
              )}

              {extractionError && (
                <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/20 text-rose-300 text-xs flex items-start space-x-2">
                  <AlertCircle className="w-4 h-4 shrink-0 mt-0.5 text-rose-400" />
                  <div>
                    <p className="font-semibold text-rose-200">Extraction Error</p>
                    <p className="font-mono mt-0.5 text-[11px]">{extractionError}</p>
                  </div>
                </div>
              )}

              {extractionResult && (
                <div className="space-y-3">
                  <div className="flex flex-wrap items-center gap-2 text-[11px]">
                    <span className="text-slate-400">
                      Length: <strong className="text-slate-200 font-mono">{extractionResult.character_count} chars</strong>
                    </span>
                    {extractionResult.page_count != null && (
                      <>
                        <span className="text-slate-600">&bull;</span>
                        <span className="text-slate-400">
                          Pages: <strong className="text-slate-200 font-mono">{extractionResult.page_count}</strong>
                        </span>
                      </>
                    )}
                    <span className="text-slate-600">&bull;</span>
                    <span className="text-slate-400">
                      Duration: <strong className="text-slate-200 font-mono">{extractionResult.duration_ms} ms</strong>
                    </span>
                    <span className="text-slate-600">&bull;</span>
                    <span className="text-slate-400">
                      Size: <strong className="text-slate-200 font-mono">{formatBytes(extractionResult.file_size_bytes)}</strong>
                    </span>
                    <span className="text-slate-600">&bull;</span>
                    <span className="text-slate-400">
                      Path: <span className="font-mono text-slate-300">{extractionResult.relative_path}</span>
                    </span>
                  </div>

                  {extractionResult.diagnostic_notice && (
                    <div className="p-3 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-200 text-xs flex items-start space-x-2.5">
                      <Info className="w-4 h-4 shrink-0 mt-0.5 text-amber-400" />
                      <div>
                        <p className="font-semibold text-amber-300">Document Layer Notice</p>
                        <p className="mt-0.5 text-[11px] text-amber-200/90 leading-relaxed">
                          {extractionResult.diagnostic_notice}
                        </p>
                      </div>
                    </div>
                  )}

                  {extractionResult.is_empty ? (
                    <div className="p-4 bg-slate-900 border border-dashed border-slate-800 rounded-lg text-center text-xs text-slate-400 italic">
                      {extractionResult.diagnostic_notice
                        ? "File contains 0 extracted digital text characters. See diagnostic notice above."
                        : "File contains 0 extracted text characters."}
                    </div>
                  ) : (
                    <div className="max-h-60 overflow-y-auto p-3.5 bg-slate-900/90 rounded-lg border border-slate-800 text-xs text-slate-200 font-mono whitespace-pre-wrap leading-relaxed select-text">
                      {extractionResult.extracted_text}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </section>

        {/* Backend IPC Verification Card (Preserved from Phase 1) */}
        <div className="border border-slate-800 bg-slate-900/30 rounded-xl p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center space-x-2 text-xs font-semibold uppercase tracking-wider text-slate-400">
              <Cpu className="w-4 h-4 text-emerald-400" />
              <span>Rust Backend IPC Health Status</span>
            </div>
            <span className="text-xs text-slate-500 font-mono">Tauri v2 Command: get_app_status</span>
          </div>

          {statusLoading && (
            <p className="text-xs text-slate-400 italic">Querying backend status over IPC...</p>
          )}

          {statusError && (
            <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/20 text-rose-300 text-xs flex items-start space-x-2">
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
              <div>
                <p className="font-medium">IPC Call Failed</p>
                <p className="font-mono mt-0.5">{statusError}</p>
              </div>
            </div>
          )}

          {status && (
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs">
              <div className="bg-slate-950/60 border border-slate-800/80 rounded-lg p-2.5">
                <span className="text-slate-500 block text-[11px]">Backend Message</span>
                <span className="font-medium text-emerald-300 truncate block mt-0.5">{status.message}</span>
              </div>
              <div className="bg-slate-950/60 border border-slate-800/80 rounded-lg p-2.5">
                <span className="text-slate-500 block text-[11px]">App Version</span>
                <span className="font-mono text-slate-200 block mt-0.5">v{status.version}</span>
              </div>
              <div className="bg-slate-950/60 border border-slate-800/80 rounded-lg p-2.5">
                <span className="text-slate-500 block text-[11px]">Rust Toolchain</span>
                <span className="font-mono text-slate-200 block mt-0.5">{status.rust_version}</span>
              </div>
              <div className="bg-slate-950/60 border border-slate-800/80 rounded-lg p-2.5">
                <span className="text-slate-500 block text-[11px]">Platform</span>
                <span className="font-mono text-slate-200 block mt-0.5">{status.os}</span>
              </div>
            </div>
          )}
        </div>
      </main>

      {/* Footer */}
      <footer className="border-t border-slate-900 bg-slate-950/80 px-6 py-3 text-center text-xs text-slate-500">
        Offline File Finder &bull; Commercial Windows Desktop Application &bull; Privacy-First &amp; Zero Cloud
      </footer>
    </div>
  );
}
