//! Office Document Conversion Command Facade
//! Routes export requests to format-specific conversion modules (DOCX, PPTX, XLSX, TXT).

use std::path::Path;
use std::sync::atomic::Ordering;

use crate::error::PdfError;
use super::conversion::{self, CANCELLATION_REQUESTED};

pub use super::conversion::{ConversionProgress, PageTextData};

#[tauri::command]
pub fn cancel_conversion() -> Result<(), PdfError> {
    conversion::cancel_conversion()
}

#[tauri::command]
pub async fn export_office_document(
    app: tauri::AppHandle,
    input_path: Option<String>,
    output_path: String,
    format: String,
    doc_title: String,
    language: Option<String>,
    pages: Vec<PageTextData>,
) -> Result<(), PdfError> {
    CANCELLATION_REQUESTED.store(false, Ordering::SeqCst);
    let path = Path::new(&output_path);
    let lang = language.as_deref().unwrap_or("tr");
    let fmt = format.to_lowercase();
    let in_path_ref = input_path.as_deref();

    match fmt.as_str() {
        "docx" => conversion::docx::convert(&app, in_path_ref, path, &doc_title, &pages, lang),
        "pptx" => conversion::pptx::convert(&app, in_path_ref, path, &pages, lang),
        "xlsx" => conversion::xlsx::convert(&app, in_path_ref, path, &pages),
        "txt" => conversion::txt::convert(&app, in_path_ref, path, &pages),
        other => Err(PdfError::Custom(format!("Desteklenmeyen dosya formatı: {}", other))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Read;
    use zip::ZipArchive;

    fn sample_pages() -> Vec<PageTextData> {
        vec![
            PageTextData {
                page: 1,
                text: "T.C. Hazine ve Maliye Bakanlığı\nFatura No: 2026-001\nÜrün Adı\tMiktar\tFiyat\nKahve Çekirdeği\t2\t150,00 TL\nŞeker\t1\t45,50 TL".to_string(),
            },
            PageTextData {
                page: 2,
                text: "Özel Şartlar ve Hükümler:\n1. İşbu belge Türkçe karakterler içerir: ç, ğ, ı, ö, ş, ü, Ç, Ğ, İ, Ö, Ş, Ü.\n2. Güvenli OpenXML paketidir.".to_string(),
            },
        ]
    }

    #[test]
    fn test_docx_export() {
        let temp_dir = std::env::temp_dir();
        let docx_path = temp_dir.join("test_output.docx");
        let pages = sample_pages();
        let result = conversion::docx::generate_docx(&docx_path, "Test Faturası", &pages, "tr");
        assert!(result.is_ok(), "generate_docx failed: {:?}", result.err());

        // Verify valid zip and contents
        let file = File::open(&docx_path).expect("open docx");
        let mut archive = ZipArchive::new(file).expect("read zip archive");
        assert!(archive.by_name("[Content_Types].xml").is_ok());
        assert!(archive.by_name("_rels/.rels").is_ok());
        assert!(archive.by_name("word/document.xml").is_ok());
        assert!(archive.by_name("word/styles.xml").is_ok());

        let mut doc_xml = String::new();
        archive.by_name("word/document.xml").unwrap().read_to_string(&mut doc_xml).unwrap();
        assert!(doc_xml.contains("Test Faturası"));
        assert!(doc_xml.contains("Kahve Çekirdeği"));
        assert!(doc_xml.contains("Türkçe karakterler içerir: ç, ğ, ı, ö, ş, ü"));

        let _ = std::fs::remove_file(docx_path);
    }

    #[test]
    fn test_xlsx_export() {
        let temp_dir = std::env::temp_dir();
        let xlsx_path = temp_dir.join("test_output.xlsx");
        let pages = sample_pages();
        let result = conversion::xlsx::generate_xlsx(&xlsx_path, &pages);
        assert!(result.is_ok(), "generate_xlsx failed: {:?}", result.err());

        let file = File::open(&xlsx_path).expect("open xlsx");
        let mut archive = ZipArchive::new(file).expect("read zip archive");
        assert!(archive.by_name("[Content_Types].xml").is_ok());
        assert!(archive.by_name("xl/workbook.xml").is_ok());
        assert!(archive.by_name("xl/worksheets/sheet1.xml").is_ok());

        let mut sheet_xml = String::new();
        archive.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut sheet_xml).unwrap();
        assert!(sheet_xml.contains("Kahve Çekirdeği"));
        assert!(sheet_xml.contains("150,00 TL"));

        let _ = std::fs::remove_file(xlsx_path);
    }

    #[test]
    fn test_pptx_export() {
        let temp_dir = std::env::temp_dir();
        let pptx_path = temp_dir.join("test_output.pptx");
        let pages = sample_pages();
        let result = conversion::pptx::generate_pptx(&pptx_path, &pages, "tr");
        assert!(result.is_ok(), "generate_pptx failed: {:?}", result.err());

        let file = File::open(&pptx_path).expect("open pptx");
        let mut archive = ZipArchive::new(file).expect("read zip archive");
        assert!(archive.by_name("[Content_Types].xml").is_ok());
        assert!(archive.by_name("ppt/presentation.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide2.xml").is_ok());

        let mut slide1_xml = String::new();
        archive.by_name("ppt/slides/slide1.xml").unwrap().read_to_string(&mut slide1_xml).unwrap();
        assert!(slide1_xml.contains("Sayfa 1"));
        assert!(slide1_xml.contains("Kahve Çekirdeği"));

        let _ = std::fs::remove_file(pptx_path);
    }

    #[test]
    fn test_txt_export() {
        let temp_dir = std::env::temp_dir();
        let txt_path = temp_dir.join("test_output.txt");
        let pages = sample_pages();
        let result = conversion::txt::generate_txt(&txt_path, &pages);
        assert!(result.is_ok(), "generate_txt failed: {:?}", result.err());

        let bytes = std::fs::read(&txt_path).expect("read txt");
        // Verify UTF-8 BOM
        assert_eq!(&bytes[0..3], &[0xEF, 0xBB, 0xBF]);
        let text = String::from_utf8(bytes[3..].to_vec()).expect("valid utf-8");
        assert!(text.contains("Kahve Çekirdeği"));
        assert!(text.contains("Türkçe karakterler içerir: ç, ğ, ı, ö, ş, ü, Ç, Ğ, İ, Ö, Ş, Ü."));

        let _ = std::fs::remove_file(txt_path);
    }
}
