use crate::error::PdfError;
use crate::types::CompressResult;
use lopdf::Document;
use std::path::Path;

/// Compression with level support:
/// - "lossless": deflates uncompressed streams, removes dead objects & metadata (zero quality loss)
/// - "balanced": lossless + downsamples images to max 1600px with 75% JPEG quality (ideal for screen & email)
/// - "maximum": lossless + downsamples images to max 1000px with 50% JPEG quality (maximum size savings)
#[tauri::command]
pub async fn compress_pdf(
    input: String,
    output: String,
    level: Option<String>,
) -> Result<CompressResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        compress_document(
            Path::new(&input),
            Path::new(&output),
            level.as_deref().unwrap_or("lossless"),
        )
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

fn compress_document(input: &Path, output: &Path, level: &str) -> Result<CompressResult, PdfError> {
    let before_bytes = std::fs::metadata(input)?.len();

    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    // Strip metadata: document Info dict and XMP Metadata stream reference.
    doc.trailer.remove(b"Info");
    if let Ok(catalog) = doc.catalog_mut() {
        catalog.remove(b"Metadata");
    }

    doc.delete_zero_length_streams();
    doc.prune_objects();

    // If level is "balanced" or "maximum", compress/downscale embedded images
    if level == "balanced" || level == "maximum" {
        let (max_dim, quality) = if level == "maximum" {
            (1000u32, 50u8)
        } else {
            (1600u32, 75u8)
        };
        optimize_images(&mut doc, max_dim, quality);
    }

    doc.compress();

    // Unique temporary path to check resulting file size before committing to destination
    let temp_path = std::env::temp_dir().join(format!(
        "pdfkit_cmp_{}_{}.pdf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    doc.save(&temp_path)?;
    drop(doc);

    let after_bytes = std::fs::metadata(&temp_path)?.len();

    // Safety Guard:
    // If the re-encoded document is not strictly smaller than the original,
    // do not create/overwrite the destination file. Clean up temp and return already_optimized.
    if after_bytes >= before_bytes {
        let _ = std::fs::remove_file(&temp_path);
        return Ok(CompressResult {
            output: String::new(),
            before_bytes,
            after_bytes,
            already_optimized: true,
        });
    }

    // Move or copy temp file to final destination
    if let Err(_) = std::fs::rename(&temp_path, output) {
        std::fs::copy(&temp_path, output)?;
        let _ = std::fs::remove_file(&temp_path);
    }

    let result = CompressResult {
        output: output.to_string_lossy().into_owned(),
        before_bytes,
        after_bytes,
        already_optimized: false,
    };
    Ok(result)
}

fn optimize_images(doc: &mut Document, max_dim: u32, quality: u8) {
    for (_id, object) in doc.objects.iter_mut() {
        if let lopdf::Object::Stream(ref mut stream) = object {
            // Check if stream is an image XObject
            let is_image = stream
                .dict
                .get(b"Subtype")
                .map(|s| s.as_name().map(|n| n == b"Image").unwrap_or(false))
                .unwrap_or(false);
            if !is_image {
                continue;
            }

            let width = stream.dict.get(b"Width").and_then(|w| w.as_i64()).unwrap_or(0) as u32;
            let height = stream.dict.get(b"Height").and_then(|h| h.as_i64()).unwrap_or(0) as u32;
            if width == 0 || height == 0 {
                continue;
            }

            // Attempt to decode image from raw stream content or decompressed content
            let dyn_img = if let Ok(img) = image::load_from_memory(&stream.content) {
                Some(img)
            } else if let Ok(decompressed) = stream.decompressed_content() {
                if let Ok(img) = image::load_from_memory(&decompressed) {
                    Some(img)
                } else if decompressed.len() == (width * height * 3) as usize {
                    image::RgbImage::from_raw(width, height, decompressed)
                        .map(image::DynamicImage::ImageRgb8)
                } else if decompressed.len() == (width * height) as usize {
                    image::GrayImage::from_raw(width, height, decompressed)
                        .map(image::DynamicImage::ImageLuma8)
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(img) = dyn_img {
                let (w, h) = (img.width(), img.height());
                let should_resize = w > max_dim || h > max_dim;
                let target_img = if should_resize {
                    img.resize(max_dim, max_dim, image::imageops::FilterType::Triangle)
                } else {
                    img
                };

                let mut buf = std::io::Cursor::new(Vec::new());
                let mut encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
                if encoder.encode_image(&target_img).is_ok() {
                    let new_bytes = buf.into_inner();
                    // Only replace if smaller than the original stream content
                    if new_bytes.len() < stream.content.len() {
                        stream.content = new_bytes;
                        stream.dict.set("Filter", lopdf::Object::Name(b"DCTDecode".to_vec()));
                        stream.dict.set("Width", lopdf::Object::Integer(target_img.width() as i64));
                        stream.dict.set("Height", lopdf::Object::Integer(target_img.height() as i64));
                        stream.dict.set("ColorSpace", lopdf::Object::Name(b"DeviceRGB".to_vec()));
                        stream.dict.set("BitsPerComponent", lopdf::Object::Integer(8));
                        stream.dict.remove(b"DecodeParms");
                    }
                }
            }
        }
    }
}

