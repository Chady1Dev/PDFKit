use crate::error::PdfError;
use crate::pdfutil::{ensure_helvetica, escape_pdf_text, media_box};
use crate::types::OpResult;
use lopdf::{Dictionary, Document, Object, Stream};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrPageResult {
    pub text: String,
    pub words: Vec<OcrWord>,
    pub image_width: f64,
    pub image_height: f64,
}

fn get_ocr_script_path() -> PathBuf {
    let script_path = PathBuf::from("src-tauri/scripts/ocr_engine.ps1");
    if script_path.exists() {
        return script_path;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidates = [
                dir.join("scripts").join("ocr_engine.ps1"),
                dir.join("ocr_engine.ps1"),
                dir.join("PDFKit").join("src-tauri").join("scripts").join("ocr_engine.ps1"),
                dir.join("PDFKit").join("scripts").join("ocr_engine.ps1"),
            ];
            for c in &candidates {
                if c.exists() {
                    return c.clone();
                }
            }
        }
    }
    script_path
}

/// Executes Windows.Media.Ocr on a given image file.
pub fn run_ocr_on_image(image_path: &Path, language: Option<&str>) -> Result<OcrPageResult, PdfError> {
    let script = get_ocr_script_path();
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        &script.to_string_lossy(),
        "-ImagePath",
        &image_path.to_string_lossy(),
    ]);

    if let Some(lang) = language {
        cmd.args(["-Language", lang]);
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().map_err(|e| PdfError::TaskJoin(format!("OCR çalıştırılamadı: {e}")))?;
    if !output.status.success() {
        let err_text = String::from_utf8_lossy(&output.stderr);
        return Err(PdfError::TaskJoin(format!("OCR hatası: {err_text}")));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let result: OcrPageResult = serde_json::from_str(stdout_str.trim())
        .map_err(|e| PdfError::TaskJoin(format!("OCR çıktı çözümleme hatası: {e}")))?;

    Ok(result)
}

/// Recognizes text from an image file using Windows Native OCR engine.
#[tauri::command]
pub async fn ocr_image(
    image_path: String,
    language: Option<String>,
) -> Result<OcrPageResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        run_ocr_on_image(Path::new(&image_path), language.as_deref())
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Adds an invisible searchable text layer to a specific page of a PDF using OCR word coordinates.
#[tauri::command]
pub async fn add_searchable_ocr_layer(
    input: String,
    output: String,
    page: u32,
    image_path: String,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let ocr_res = run_ocr_on_image(Path::new(&image_path), None)?;
        let mut doc = Document::load(&input)?;
        if doc.is_encrypted() {
            return Err(PdfError::Encrypted);
        }

        let page_map = doc.get_pages();
        let Some(&page_id) = page_map.get(&page) else {
            return Err(PdfError::TaskJoin(format!("Sayfa {page} bulunamadı")));
        };

        ensure_helvetica(&mut doc, page_id)?;

        let [x0, y0, x1, y1] = media_box(&doc, page_id);
        let page_w = x1 - x0;
        let page_h = y1 - y0;

        let img_w = if ocr_res.image_width > 0.0 { ocr_res.image_width } else { page_w };
        let img_h = if ocr_res.image_height > 0.0 { ocr_res.image_height } else { page_h };

        let mut stream_content = String::from("q\n3 Tr\n"); // 3 Tr = Invisible text rendering mode

        for w in ocr_res.words {
            if w.text.trim().is_empty() {
                continue;
            }
            let pdf_x = x0 + (w.x / img_w) * page_w;
            let pdf_y = y0 + page_h - ((w.y + w.height) / img_h) * page_h;
            let font_size = ((w.height / img_h) * page_h).max(6.0);
            let escaped = escape_pdf_text(&w.text);

            stream_content.push_str(&format!(
                "BT /PK_Font {font_size:.2} Tf 1 0 0 1 {pdf_x:.2} {pdf_y:.2} Tm ({escaped}) Tj ET\n"
            ));
        }

        stream_content.push_str("Q\n");

        let mut s_dict = Dictionary::new();
        s_dict.set("PDFKit_OCR", Object::Boolean(true));
        let stream = Stream::new(s_dict, stream_content.into_bytes());
        doc.add_page_contents(page_id, stream.content)?;

        doc.compress();
        doc.save(&output)?;

        let page_count = page_map.len() as u32;
        Ok(OpResult {
            outputs: vec![output],
            total_pages: page_count,
            total_size_bytes: std::fs::metadata(input)?.len(),
        })
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}
