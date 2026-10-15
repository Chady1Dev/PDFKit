//! TXT Conversion Engine Module
//! Handles text extraction and UTF-8 BOM plain text document generation.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::error::PdfError;
use super::{run_python_engine, PageTextData};

pub fn convert(
    app: &tauri::AppHandle,
    input_path: Option<&str>,
    output_path: &Path,
    pages: &[PageTextData],
) -> Result<(), PdfError> {
    let out_str = output_path.to_string_lossy();
    if let Some(in_path) = input_path {
        if Path::new(in_path).exists() {
            match run_python_engine(app, in_path, &out_str, "txt") {
                Ok(()) => {
                    eprintln!("[TXT] Yüksek kaliteli metin çıkarma başarıyla tamamlandı.");
                    return Ok(());
                }
                Err(err) => {
                    eprintln!("[TXT] Gelişmiş motor hatası: {}, yerel çıkarım deneniyor...", err);
                    if err.contains("iptal edildi") || err.contains("Dosya başka bir programda açık") {
                        return Err(PdfError::Custom(err));
                    }
                }
            }
        }
    }
    generate_txt(output_path, pages)
}

/// Builds a clean UTF-8 text file with BOM
pub fn generate_txt(
    output_path: &Path,
    pages: &[PageTextData],
) -> Result<(), PdfError> {
    let mut file = File::create(output_path)?;
    // Write UTF-8 BOM so Windows Notepad and other tools natively display Turkish characters
    file.write_all(&[0xEF, 0xBB, 0xBF])?;

    for (idx, p) in pages.iter().enumerate() {
        let header = format!("--- Sayfa {} ---\r\n", p.page);
        file.write_all(header.as_bytes())?;

        let trimmed = p.text.trim();
        if trimmed.is_empty() {
            file.write_all("(Bu sayfa görsel içeriklidir veya metin katmanı içermemektedir)\r\n\r\n".as_bytes())?;
        } else {
            for line in p.text.lines() {
                file.write_all(line.as_bytes())?;
                file.write_all(b"\r\n")?;
            }
            file.write_all(b"\r\n")?;
        }

        if idx < pages.len() - 1 {
            file.write_all(b"\r\n")?;
        }
    }

    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_txt_export_generation() {
        let temp_dir = std::env::temp_dir();
        let txt_path = temp_dir.join("test_txt_module.txt");
        let pages = vec![
            PageTextData {
                page: 1,
                text: "Kahve Çekirdeği\nTürkçe karakterler: ç, ğ, ı, ö, ş, ü, Ç, Ğ, İ, Ö, Ş, Ü.".to_string(),
            },
        ];

        let result = generate_txt(&txt_path, &pages);
        assert!(result.is_ok(), "generate_txt failed: {:?}", result.err());

        let bytes = std::fs::read(&txt_path).expect("read txt");
        assert_eq!(&bytes[0..3], &[0xEF, 0xBB, 0xBF]);
        let text = String::from_utf8(bytes[3..].to_vec()).expect("valid utf-8");
        assert!(text.contains("Kahve Çekirdeği"));
        assert!(text.contains("Türkçe karakterler: ç, ğ, ı, ö, ş, ü, Ç, Ğ, İ, Ö, Ş, Ü."));

        let _ = std::fs::remove_file(txt_path);
    }
}
