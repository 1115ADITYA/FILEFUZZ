use serde::Serialize;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::time::Instant;

/// Specific category of extraction errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ExtractionErrorKind {
    UnsupportedFormat,
    FileNotFound,
    FileTooLarge,
    SecurityBoundaryViolation,
    IoError,
    ParseError,
    EncryptedDocument,
}

/// Structured error returned when text extraction fails.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractionError {
    pub kind: ExtractionErrorKind,
    pub message: String,
}

impl std::fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ExtractionError {}

/// Standard interface for document text extractors.
pub trait DocumentExtractor: Send + Sync {
    fn supports(&self, extension: &str) -> bool;
    fn extract_text(&self, path: &Path) -> Result<String, ExtractionError>;
}

/// Plain-text extractor for `.txt` files.
pub struct TextExtractor {
    pub max_size_bytes: u64,
}

impl Default for TextExtractor {
    fn default() -> Self {
        Self {
            max_size_bytes: 10 * 1024 * 1024, // 10 MB limit for plain text
        }
    }
}

impl DocumentExtractor for TextExtractor {
    fn supports(&self, extension: &str) -> bool {
        extension.eq_ignore_ascii_case("txt")
    }

    fn extract_text(&self, path: &Path) -> Result<String, ExtractionError> {
        let metadata = fs::metadata(path).map_err(|e| ExtractionError {
            kind: if e.kind() == std::io::ErrorKind::NotFound {
                ExtractionErrorKind::FileNotFound
            } else {
                ExtractionErrorKind::IoError
            },
            message: format!("Cannot read file metadata: {}", e),
        })?;

        if metadata.len() > self.max_size_bytes {
            return Err(ExtractionError {
                kind: ExtractionErrorKind::FileTooLarge,
                message: format!(
                    "File size ({} bytes) exceeds safety limit ({} bytes)",
                    metadata.len(),
                    self.max_size_bytes
                ),
            });
        }

        if metadata.len() == 0 {
            return Ok(String::new());
        }

        let mut file = File::open(path).map_err(|e| ExtractionError {
            kind: ExtractionErrorKind::IoError,
            message: format!("Failed to open file: {}", e),
        })?;

        let mut buffer = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut buffer).map_err(|e| ExtractionError {
            kind: ExtractionErrorKind::IoError,
            message: format!("Failed to read file contents: {}", e),
        })?;

        // Lossy UTF-8 conversion ensures corrupted or non-UTF-8 bytes do not cause a crash
        let text = String::from_utf8_lossy(&buffer).into_owned();
        Ok(text)
    }
}

/// PDF text extractor for text-based PDF documents.
pub struct PdfExtractor {
    pub max_size_bytes: u64,
}

impl Default for PdfExtractor {
    fn default() -> Self {
        Self {
            max_size_bytes: 25 * 1024 * 1024, // 25 MB limit for PDF files
        }
    }
}

impl DocumentExtractor for PdfExtractor {
    fn supports(&self, extension: &str) -> bool {
        extension.eq_ignore_ascii_case("pdf")
    }

    fn extract_text(&self, path: &Path) -> Result<String, ExtractionError> {
        let metadata = fs::metadata(path).map_err(|e| ExtractionError {
            kind: if e.kind() == std::io::ErrorKind::NotFound {
                ExtractionErrorKind::FileNotFound
            } else {
                ExtractionErrorKind::IoError
            },
            message: format!("Cannot read PDF metadata: {}", e),
        })?;

        if metadata.len() > self.max_size_bytes {
            return Err(ExtractionError {
                kind: ExtractionErrorKind::FileTooLarge,
                message: format!(
                    "PDF size ({} bytes) exceeds safety limit ({} bytes)",
                    metadata.len(),
                    self.max_size_bytes
                ),
            });
        }

        if metadata.len() == 0 {
            return Ok(String::new());
        }

        // Call pdf-extract in a safe, non-panicking wrapper
        match pdf_extract::extract_text(path) {
            Ok(extracted) => Ok(extracted.trim().to_string()),
            Err(err) => {
                let err_str = err.to_string();
                let kind = if err_str.to_lowercase().contains("encrypted")
                    || err_str.to_lowercase().contains("password")
                {
                    ExtractionErrorKind::EncryptedDocument
                } else {
                    ExtractionErrorKind::ParseError
                };
                Err(ExtractionError {
                    kind,
                    message: format!("PDF extraction error: {}", err_str),
                })
            }
        }
    }
}

/// Microsoft Word (.docx) text extractor reading `word/document.xml` from the ZIP archive.
pub struct DocxExtractor {
    pub max_size_bytes: u64,
    pub max_decompressed_bytes: u64,
}

impl Default for DocxExtractor {
    fn default() -> Self {
        Self {
            max_size_bytes: 25 * 1024 * 1024, // 25 MB safety limit for archive
            max_decompressed_bytes: 50 * 1024 * 1024, // 50 MB limit for decompressed XML to guard against zip bombs
        }
    }
}

impl DocumentExtractor for DocxExtractor {
    fn supports(&self, extension: &str) -> bool {
        extension.eq_ignore_ascii_case("docx")
    }

    fn extract_text(&self, path: &Path) -> Result<String, ExtractionError> {
        let metadata = fs::metadata(path).map_err(|e| ExtractionError {
            kind: if e.kind() == std::io::ErrorKind::NotFound {
                ExtractionErrorKind::FileNotFound
            } else {
                ExtractionErrorKind::IoError
            },
            message: format!("Cannot read DOCX metadata: {}", e),
        })?;

        if metadata.len() > self.max_size_bytes {
            return Err(ExtractionError {
                kind: ExtractionErrorKind::FileTooLarge,
                message: format!(
                    "DOCX size ({} bytes) exceeds safety limit ({} bytes)",
                    metadata.len(),
                    self.max_size_bytes
                ),
            });
        }

        if metadata.len() == 0 {
            return Ok(String::new());
        }

        let file = File::open(path).map_err(|e| ExtractionError {
            kind: ExtractionErrorKind::IoError,
            message: format!("Failed to open DOCX file: {}", e),
        })?;

        let mut archive = zip::ZipArchive::new(file).map_err(|e| ExtractionError {
            kind: ExtractionErrorKind::ParseError,
            message: format!("Invalid or corrupted DOCX ZIP archive: {}", e),
        })?;

        let xml_entry = match archive.by_name("word/document.xml") {
            Ok(entry) => entry,
            Err(zip::result::ZipError::FileNotFound) => {
                return Err(ExtractionError {
                    kind: ExtractionErrorKind::ParseError,
                    message: "Missing word/document.xml in DOCX archive".to_string(),
                });
            }
            Err(e) => {
                return Err(ExtractionError {
                    kind: ExtractionErrorKind::ParseError,
                    message: format!("Failed to access word/document.xml in archive: {}", e),
                });
            }
        };

        // Guard against excessive decompressed XML content (ZIP bomb defense)
        let bounded_reader = std::io::BufReader::new(xml_entry.take(self.max_decompressed_bytes));
        let mut xml_reader = quick_xml::reader::Reader::from_reader(bounded_reader);
        xml_reader.config_mut().trim_text(false);

        let mut output = String::new();
        let mut in_text_node = false;
        let mut current_paragraph = String::new();
        let mut buf = Vec::new();

        loop {
            match xml_reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"t" {
                        in_text_node = true;
                    }
                }
                Ok(quick_xml::events::Event::End(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"t" {
                        in_text_node = false;
                    } else if local_name.as_ref() == b"p" {
                        let trimmed = current_paragraph.trim();
                        if !trimmed.is_empty() {
                            if !output.is_empty() {
                                output.push_str("\n\n");
                            }
                            output.push_str(trimmed);
                        }
                        current_paragraph.clear();
                    }
                }
                Ok(quick_xml::events::Event::Empty(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"br" || local_name.as_ref() == b"cr" {
                        current_paragraph.push('\n');
                    } else if local_name.as_ref() == b"tab" {
                        current_paragraph.push('\t');
                    }
                }
                Ok(quick_xml::events::Event::Text(ref e)) => {
                    if in_text_node {
                        match e.unescape() {
                            Ok(text) => current_paragraph.push_str(&text),
                            Err(_) => {
                                let raw = String::from_utf8_lossy(e.as_ref());
                                current_paragraph.push_str(&raw);
                            }
                        }
                    }
                }
                Ok(quick_xml::events::Event::CData(ref e)) => {
                    if in_text_node {
                        let text = String::from_utf8_lossy(e.as_ref());
                        current_paragraph.push_str(&text);
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => {
                    return Err(ExtractionError {
                        kind: ExtractionErrorKind::ParseError,
                        message: format!("Malformed XML in DOCX document.xml: {}", e),
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        // Flush trailing paragraph content if any remained unclosed
        let remaining = current_paragraph.trim();
        if !remaining.is_empty() {
            if !output.is_empty() {
                output.push_str("\n\n");
            }
            output.push_str(remaining);
        }

        Ok(output)
    }
}

/// Response returned to the Tauri frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractionResponse {
    pub success: bool,
    pub relative_path: String,
    pub filename: String,
    pub extension: String,
    pub extracted_text: String,
    pub character_count: usize,
    pub file_size_bytes: u64,
    pub page_count: Option<usize>,
    pub has_text_layer: bool,
    pub diagnostic_notice: Option<String>,
    pub duration_ms: u64,
    pub is_empty: bool,
}

/// Unified extraction handler enforcing the canonical `test-files` security sandbox.
pub fn extract_test_file_internal(
    relative_path: &str,
) -> Result<ExtractionResponse, ExtractionError> {
    let start_time = Instant::now();
    let canonical_root = crate::scanner::resolve_test_root().map_err(|e| ExtractionError {
        kind: ExtractionErrorKind::SecurityBoundaryViolation,
        message: e,
    })?;

    // Clean relative path and construct candidate path
    let clean_relative = relative_path.trim_start_matches(['/', '\\']);
    let candidate_path = canonical_root.join(clean_relative);

    // Canonicalize target to resolve any '.' or '..' traversal attempts
    let canonical_target = candidate_path.canonicalize().map_err(|e| ExtractionError {
        kind: if e.kind() == std::io::ErrorKind::NotFound {
            ExtractionErrorKind::FileNotFound
        } else {
            ExtractionErrorKind::IoError
        },
        message: format!("File not found or unreadable {:?}: {}", candidate_path, e),
    })?;

    // Strict boundary enforcement: Fail closed if path escapes canonical sandbox
    if !canonical_target.starts_with(&canonical_root) {
        return Err(ExtractionError {
            kind: ExtractionErrorKind::SecurityBoundaryViolation,
            message: format!(
                "Security violation: Target {:?} escapes approved root {:?}",
                canonical_target, canonical_root
            ),
        });
    }

    let extension = canonical_target
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    let text_extractor = TextExtractor::default();
    let pdf_extractor = PdfExtractor::default();
    let docx_extractor = DocxExtractor::default();

    let (extracted_text, page_count) = if text_extractor.supports(&extension) {
        let text = text_extractor.extract_text(&canonical_target)?;
        (text, None)
    } else if pdf_extractor.supports(&extension) {
        let page_count = lopdf::Document::load(&canonical_target)
            .map(|doc| doc.get_pages().len())
            .ok();
        let text = pdf_extractor.extract_text(&canonical_target)?;
        (text, page_count)
    } else if docx_extractor.supports(&extension) {
        let text = docx_extractor.extract_text(&canonical_target)?;
        (text, None)
    } else {
        return Err(ExtractionError {
            kind: ExtractionErrorKind::UnsupportedFormat,
            message: format!(
                "Unsupported file format '.{}'. Phase 6 supports .txt, .pdf, and .docx text extraction.",
                extension
            ),
        });
    };

    let metadata = fs::metadata(&canonical_target).map_err(|e| ExtractionError {
        kind: ExtractionErrorKind::IoError,
        message: format!("Failed to read metadata: {}", e),
    })?;

    let filename = canonical_target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let character_count = extracted_text.chars().count();
    let is_empty = character_count == 0;
    let has_text_layer = !is_empty;

    let diagnostic_notice = if is_empty && metadata.len() > 0 {
        if extension == "pdf" {
            let pages_msg = page_count
                .map(|p| format!(" across {} page{}", p, if p == 1 { "" } else { "s" }))
                .unwrap_or_default();
            Some(format!(
                "Scanned / image-only document: No extractable digital text layer found{} (scanned or handwritten pages require OCR).",
                pages_msg
            ))
        } else {
            Some("Empty document: File contains 0 readable characters.".to_string())
        }
    } else {
        None
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(ExtractionResponse {
        success: true,
        relative_path: clean_relative.replace('\\', "/"),
        filename,
        extension,
        extracted_text,
        character_count,
        file_size_bytes: metadata.len(),
        page_count,
        has_text_layer,
        diagnostic_notice,
        duration_ms,
        is_empty,
    })
}

/// Helper to construct a 100% standards-compliant text PDF fixture using lopdf.
pub fn create_test_pdf(dest_path: &Path) -> std::io::Result<()> {
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let text_content =
        "Offline File Finder Synthetic PDF Test Fixture. Local PDF text extraction verified.";
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("Td", vec![50.into(), 700.into()]),
            Operation::new("Tj", vec![Object::string_literal(text_content)]),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        },
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(dest_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    Ok(())
}

/// Helper to construct a synthetic DOCX fixture containing known paragraphs using zip.
pub fn create_test_docx(dest_path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let file = fs::File::create(dest_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)?;
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
    zip.write_all(content_types.as_bytes())?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options)?;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    zip.write_all(rels.as_bytes())?;

    // 3. word/document.xml with XML entity &amp;
    zip.start_file("word/document.xml", options)?;
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:t>Offline File Finder DOCX extraction &amp; search fixture.</w:t>
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:t>Search document contents locally.</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;
    zip.write_all(doc_xml.as_bytes())?;

    zip.finish()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_txt_sample() {
        let res =
            extract_test_file_internal("sample.txt").expect("sample.txt extraction should succeed");
        assert!(res.success);
        assert_eq!(res.extension, "txt");
        assert!(res.character_count > 0);
        assert!(res.extracted_text.contains("Offline File Finder"));
    }

    #[test]
    fn test_extract_nested_txt() {
        let res = extract_test_file_internal("nested_docs/nested_readme.txt")
            .expect("nested txt extraction should succeed");
        assert!(res.success);
        assert!(res.extracted_text.contains("Nested directory fixture"));
    }

    #[test]
    fn test_extract_pdf_sample() {
        let canonical_root = crate::scanner::resolve_test_root().expect("Root must resolve");
        let pdf_path = canonical_root.join("sample_document.pdf");
        create_test_pdf(&pdf_path).expect("Create test PDF fixture");

        let res = extract_test_file_internal("sample_document.pdf")
            .expect("sample_document.pdf extraction should succeed");
        assert!(res.success);
        assert_eq!(res.extension, "pdf");
        assert!(res.character_count > 0);
        assert!(res
            .extracted_text
            .contains("Offline File Finder Synthetic PDF Test Fixture"));
    }

    #[test]
    fn test_extract_docx_sample() {
        let canonical_root = crate::scanner::resolve_test_root().expect("Root must resolve");
        let docx_path = canonical_root.join("sample_document.docx");
        if !docx_path.exists() {
            create_test_docx(&docx_path).expect("Create test DOCX fixture");
        }

        let res = extract_test_file_internal("sample_document.docx")
            .expect("sample_document.docx extraction should succeed");
        assert!(res.success);
        assert_eq!(res.extension, "docx");
        assert!(res.character_count > 0);
        assert!(res
            .extracted_text
            .contains("Offline File Finder DOCX extraction & search fixture."));
        assert!(res
            .extracted_text
            .contains("Search document contents locally."));
        // Verify paragraph boundary
        assert!(res.extracted_text.contains("\n\n"));
    }

    #[test]
    fn test_extract_corrupt_docx_zip() {
        let temp_dir = std::env::temp_dir();
        let corrupt_docx_path = temp_dir.join(format!("corrupt_{}.docx", std::process::id()));
        fs::write(&corrupt_docx_path, b"NOT A VALID ZIP FILE HEADER").expect("Write corrupt docx");

        let extractor = DocxExtractor::default();
        let err = extractor.extract_text(&corrupt_docx_path).unwrap_err();
        assert_eq!(err.kind, ExtractionErrorKind::ParseError);
        assert!(err.message.contains("ZIP"));

        let _ = fs::remove_file(corrupt_docx_path);
    }

    #[test]
    fn test_extract_docx_missing_document_xml() {
        let temp_dir = std::env::temp_dir();
        let incomplete_path = temp_dir.join(format!("incomplete_{}.docx", std::process::id()));

        {
            let file = fs::File::create(&incomplete_path).expect("create file");
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            use std::io::Write;
            zip.start_file("some_other_file.txt", options).unwrap();
            zip.write_all(b"hello").unwrap();
            zip.finish().unwrap();
        }

        let extractor = DocxExtractor::default();
        let err = extractor.extract_text(&incomplete_path).unwrap_err();
        assert_eq!(err.kind, ExtractionErrorKind::ParseError);
        assert!(err.message.contains("Missing word/document.xml"));

        let _ = fs::remove_file(incomplete_path);
    }

    #[test]
    fn test_extract_docx_empty_file() {
        let temp_dir = std::env::temp_dir();
        let empty_docx_path = temp_dir.join(format!("empty_{}.docx", std::process::id()));
        fs::write(&empty_docx_path, b"").expect("Write empty docx");

        let extractor = DocxExtractor::default();
        let text = extractor
            .extract_text(&empty_docx_path)
            .expect("empty docx handling");
        assert_eq!(text, "");

        let _ = fs::remove_file(empty_docx_path);
    }

    #[test]
    fn test_extract_empty_file() {
        let res =
            extract_test_file_internal("empty.txt").expect("empty.txt extraction should succeed");
        assert!(res.success);
        assert_eq!(res.character_count, 0);
        assert!(res.is_empty);
    }

    #[test]
    fn test_extract_missing_file_returns_error() {
        let err = extract_test_file_internal("non_existent_file.txt").unwrap_err();
        assert_eq!(err.kind, ExtractionErrorKind::FileNotFound);
    }

    #[test]
    fn test_extract_corrupt_pdf_returns_error_gracefully() {
        let err = extract_test_file_internal("corrupt.pdf").unwrap_err();
        assert_eq!(err.kind, ExtractionErrorKind::ParseError);
        assert!(!err.message.is_empty());
    }

    #[test]
    fn test_unsupported_format_rejected() {
        let canonical_root = crate::scanner::resolve_test_root().expect("Root must resolve");
        let bin_path = canonical_root.join("unsupported.bin");
        fs::write(&bin_path, b"\x00\x01\x02\x03").expect("Write binary fixture");

        let err = extract_test_file_internal("unsupported.bin").unwrap_err();
        assert_eq!(err.kind, ExtractionErrorKind::UnsupportedFormat);

        // Clean up temporary unsupported file
        let _ = fs::remove_file(bin_path);
    }

    #[test]
    fn test_extract_scanned_pdf_diagnostics() {
        let canonical_root = crate::scanner::resolve_test_root().expect("Root must resolve");
        let pdf_path = canonical_root.join("DSAssignment07-25101A0022.pdf");
        if !pdf_path.exists() {
            return;
        }

        let res = extract_test_file_internal("DSAssignment07-25101A0022.pdf")
            .expect("Extraction should succeed and report diagnostics");
        assert!(res.success);
        assert_eq!(res.character_count, 0);
        assert!(res.is_empty);
        assert!(!res.has_text_layer);
        assert_eq!(res.page_count, Some(8));
        assert!(res.diagnostic_notice.is_some());
        assert!(res
            .diagnostic_notice
            .as_ref()
            .unwrap()
            .contains("Scanned / image-only"));
    }

    #[test]
    fn test_path_escaping_rejected() {
        let err =
            extract_test_file_internal("../../Windows/System32/drivers/etc/hosts").unwrap_err();
        assert!(
            err.kind == ExtractionErrorKind::SecurityBoundaryViolation
                || err.kind == ExtractionErrorKind::FileNotFound
        );
    }
}
