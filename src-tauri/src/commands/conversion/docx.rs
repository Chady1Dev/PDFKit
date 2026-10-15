//! DOCX Conversion Engine Module
//! Handles Word document reconstruction and native OpenXML packaging.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

use crate::error::PdfError;
use super::{escape_xml, run_python_engine, PageTextData};

pub fn convert(
    app: &tauri::AppHandle,
    input_path: Option<&str>,
    output_path: &Path,
    doc_title: &str,
    pages: &[PageTextData],
    language: &str,
) -> Result<(), PdfError> {
    let out_str = output_path.to_string_lossy();
    if let Some(in_path) = input_path {
        if Path::new(in_path).exists() {
            match run_python_engine(app, in_path, &out_str, "docx") {
                Ok(()) => {
                    eprintln!("[DOCX] Yüksek kaliteli Word yeniden inşası başarıyla tamamlandı.");
                    return Ok(());
                }
                Err(err) => {
                    eprintln!("[DOCX] Gelişmiş motor hatası: {}, yerel OpenXML deneniyor...", err);
                    if err.contains("iptal edildi") || err.contains("Dosya başka bir programda açık") {
                        return Err(PdfError::Custom(err));
                    }
                    if pages.is_empty() {
                        return Err(PdfError::Custom(format!("Word dönüştürme hatası: {}", err)));
                    }
                }
            }
        }
    }
    generate_docx(output_path, doc_title, pages, language)
}

/// Builds a 100% valid OpenXML DOCX archive
pub fn generate_docx(
    output_path: &Path,
    doc_title: &str,
    pages: &[PageTextData],
    language: &str,
) -> Result<(), PdfError> {
    let file = File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let lang_tag = if language == "en" { "en-US" } else { "tr-TR" };
    let escaped_title = escape_xml(doc_title);

    // 1. [Content_Types].xml
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>
</Types>"#;
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(content_types.as_bytes())?;

    // 2. _rels/.rels
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    zip.start_file("_rels/.rels", options)?;
    zip.write_all(root_rels.as_bytes())?;

    // 3. word/_rels/document.xml.rels
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>
</Relationships>"#;
    zip.start_file("word/_rels/document.xml.rels", options)?;
    zip.write_all(doc_rels.as_bytes())?;

    // 4. word/settings.xml
    let settings = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:defaultTabStop w:val="720"/>
  <w:characterSpacingControl w:val="doNotCompress"/>
</w:settings>"#;
    zip.start_file("word/settings.xml", options)?;
    zip.write_all(settings.as_bytes())?;

    // 5. word/styles.xml
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/>
        <w:sz w:val="22"/>
        <w:szCs w:val="22"/>
        <w:lang w:val="{}"/>
      </w:rPr>
    </w:rPrDefault>
    <w:pPrDefault>
      <w:pPr>
        <w:spacing w:after="140" w:line="240" w:lineRule="auto"/>
      </w:pPr>
    </w:pPrDefault>
  </w:docDefaults>
</w:styles>"#,
        lang_tag
    );
    zip.start_file("word/styles.xml", options)?;
    zip.write_all(styles.as_bytes())?;

    // 6. word/document.xml
    let mut body = String::new();

    // Document Main Title
    body.push_str(&format!(
        r#"<w:p>
  <w:pPr>
    <w:spacing w:before="120" w:after="240"/>
    <w:jc w:val="center"/>
  </w:pPr>
  <w:r>
    <w:rPr>
      <w:b/>
      <w:sz w:val="36"/>
      <w:color w:val="1E40AF"/>
    </w:rPr>
    <w:t xml:space="preserve">{}</w:t>
  </w:r>
</w:p>"#,
        escaped_title
    ));

    for (page_idx, p) in pages.iter().enumerate() {
        let is_last_page = page_idx == pages.len() - 1;

        // Page Header
        body.push_str(&format!(
            r#"<w:p>
  <w:pPr>
    <w:spacing w:before="240" w:after="120"/>
  </w:pPr>
  <w:r>
    <w:rPr>
      <w:b/>
      <w:sz w:val="26"/>
      <w:color w:val="4F46E5"/>
    </w:rPr>
    <w:t xml:space="preserve">Sayfa {}</w:t>
  </w:r>
</w:p>"#,
            p.page
        ));

        let trimmed_text = p.text.trim();
        if trimmed_text.is_empty() {
            body.push_str(
                r#"<w:p>
  <w:r>
    <w:rPr>
      <w:i/>
      <w:color w:val="6B7280"/>
    </w:rPr>
    <w:t xml:space="preserve">(Bu sayfa görsel içeriklidir veya metin katmanı içermemektedir)</w:t>
  </w:r>
</w:p>"#,
            );
        } else {
            let lines: Vec<&str> = p.text.lines().collect();
            let mut line_idx = 0;
            while line_idx < lines.len() {
                let line = lines[line_idx];
                let trimmed_line = line.trim();
                if trimmed_line.is_empty() {
                    body.push_str("<w:p/>\n");
                    line_idx += 1;
                    continue;
                }

                // Check if this line has tabs indicating a table
                if line.contains('\t') {
                    let mut tbl_rows = Vec::new();
                    while line_idx < lines.len() && lines[line_idx].contains('\t') {
                        let row_cols: Vec<&str> = lines[line_idx].split('\t').map(|s| s.trim()).collect();
                        tbl_rows.push(row_cols);
                        line_idx += 1;
                    }

                    body.push_str(r#"<w:tbl>
  <w:tblPr>
    <w:tblW w:w="0" w:type="auto"/>
    <w:tblBorders>
      <w:top w:val="single" w:sz="4" w:space="0" w:color="CBD5E1"/>
      <w:left w:val="none"/>
      <w:bottom w:val="single" w:sz="6" w:space="0" w:color="94A3B8"/>
      <w:right w:val="none"/>
      <w:insideH w:val="single" w:sz="4" w:space="0" w:color="E2E8F0"/>
      <w:insideV w:val="none"/>
    </w:tblBorders>
  </w:tblPr>
"#);
                    for (r_i, row) in tbl_rows.iter().enumerate() {
                        let is_hdr = r_i == 0;
                        body.push_str("<w:tr>\n");
                        for cell in row {
                            let fill_xml = if is_hdr { r#"<w:shd w:val="clear" w:color="auto" w:fill="F1F5F9"/>"# } else { "" };
                            let b_xml = if is_hdr { "<w:b/>" } else { "" };
                            let color_xml = if is_hdr { r#"<w:color w:val="1E40AF"/>"# } else { "" };
                            body.push_str(&format!(
                                r#"<w:tc>
  <w:tcPr>
    <w:tcMar>
      <w:top w:w="120" w:type="dxa"/>
      <w:bottom w:w="120" w:type="dxa"/>
      <w:left w:w="140" w:type="dxa"/>
      <w:right w:w="140" w:type="dxa"/>
    </w:tcMar>
    {}
  </w:tcPr>
  <w:p>
    <w:r>
      <w:rPr>{}{}</w:rPr>
      <w:t xml:space="preserve">{}</w:t>
    </w:r>
  </w:p>
</w:tc>
"#,
                                fill_xml,
                                b_xml,
                                color_xml,
                                escape_xml(cell)
                            ));
                        }
                        body.push_str("</w:tr>\n");
                    }
                    body.push_str("</w:tbl>\n<w:p/>\n");
                } else {
                    body.push_str(&format!(
                        r#"<w:p>
  <w:r>
    <w:t xml:space="preserve">{}</w:t>
  </w:r>
</w:p>
"#,
                        escape_xml(line)
                    ));
                    line_idx += 1;
                }
            }
        }

        // Insert page break between PDF pages
        if !is_last_page {
            body.push_str(
                r#"<w:p>
  <w:r>
    <w:br w:type="page"/>
  </w:r>
</w:p>"#,
            );
        }
    }

    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    {}
    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#,
        body
    );

    zip.start_file("word/document.xml", options)?;
    zip.write_all(document_xml.as_bytes())?;

    zip.finish().map_err(|e| PdfError::Custom(format!("ZIP finish error: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    fn test_docx_export_generation() {
        let temp_dir = std::env::temp_dir();
        let docx_path = temp_dir.join("test_docx_module.docx");
        let pages = vec![
            PageTextData {
                page: 1,
                text: "Test Başlığı\nÜrün Adı\tFiyat\nKahve\t100 TL".to_string(),
            },
            PageTextData {
                page: 2,
                text: "İkinci sayfa Türkçe: ç, ğ, ı, ö, ş, ü.".to_string(),
            },
        ];

        let result = generate_docx(&docx_path, "Test Dokümanı", &pages, "tr");
        assert!(result.is_ok(), "generate_docx failed: {:?}", result.err());

        let file = File::open(&docx_path).expect("open docx");
        let mut archive = ZipArchive::new(file).expect("read zip archive");
        assert!(archive.by_name("[Content_Types].xml").is_ok());
        assert!(archive.by_name("_rels/.rels").is_ok());
        assert!(archive.by_name("word/document.xml").is_ok());

        let mut doc_xml = String::new();
        archive.by_name("word/document.xml").unwrap().read_to_string(&mut doc_xml).unwrap();
        assert!(doc_xml.contains("Test Dokümanı"));
        assert!(doc_xml.contains("Kahve"));
        assert!(doc_xml.contains("Türkçe: ç, ğ, ı, ö, ş, ü."));

        let _ = std::fs::remove_file(docx_path);
    }
}
