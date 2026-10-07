use crate::error::PdfError;
use crate::types::OpResult;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::{Dictionary, Document, Object, Stream};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Converts images (JPEG / PNG / WebP / BMP …) into a PDF with one image
/// per page, sized 1:1 in points.
///
/// JPEG bytes are embedded directly with DCTDecode (no re-encode, zero
/// quality loss); everything else is decoded to RGB8 and Flate-compressed.
#[tauri::command]
pub async fn images_to_pdf(inputs: Vec<String>, output: String) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let inputs: Vec<PathBuf> = inputs.into_iter().map(PathBuf::from).collect();
        images_document(&inputs, Path::new(&output))
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Writes raw bytes to disk. Used by the PDF→image flow, where PDF.js
/// renders pages to PNG/JPG in a web worker and streams them here per page.
#[tauri::command]
pub async fn write_binary_file(path: String, data: Vec<u8>) -> Result<(), PdfError> {
    let p = Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    const ALLOWED_EXTS: &[&str] = &[
        // Visual / Image formats
        "png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tif",
        "psd", "heic", "raw", "cr2", "nef", "arw", "dng",
        // Document / Office formats
        "docx", "xlsx", "pptx", "txt", "csv", "rtf", "html", "pdf",
    ];

    if !ALLOWED_EXTS.contains(&ext.as_str()) {
        return Err(PdfError::Image(format!(
            "Güvenlik kısıtlaması: Yalnızca izin verilen görsel ve ofis formatlarına ({}) yazma izni vardır.",
            ALLOWED_EXTS.join(", ")
        )));
    }

    tauri::async_runtime::spawn_blocking(move || {
        if let Some(parent) = Path::new(&path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &data)?;
        drop(data);
        Ok(())
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Decoded-or-passthrough image payload ready to become an XObject.
pub(crate) struct EmbeddedImage {
    pub stream: Stream,
    pub width: u32,
    pub height: u32,
}

/// Builds an image XObject stream from raw file bytes.
/// JPEG is passed through untouched; other formats are re-encoded as
/// FlateDecode RGB8.
pub(crate) fn build_image_xobject(bytes: &[u8]) -> Result<EmbeddedImage, PdfError> {
    let format = image::guess_format(bytes).map_err(|e| PdfError::Image(e.to_string()))?;

    if format == image::ImageFormat::Jpeg {
        let reader = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(bytes))
            .map_err(|e| PdfError::Image(e.to_string()))?;
        let (width, height) = image::ImageDecoder::dimensions(&reader);
        let dict = Dictionary::from_iter(vec![
            ("Type", Object::Name(b"XObject".to_vec())),
            ("Subtype", Object::Name(b"Image".to_vec())),
            ("Width", Object::Integer(i64::from(width))),
            ("Height", Object::Integer(i64::from(height))),
            ("ColorSpace", Object::Name(b"DeviceRGB".to_vec())),
            ("BitsPerComponent", Object::Integer(8)),
            ("Filter", Object::Name(b"DCTDecode".to_vec())),
        ]);
        return Ok(EmbeddedImage {
            stream: Stream::new(dict, bytes.to_vec()),
            width,
            height,
        });
    }

    let dyn_img = image::load_from_memory(bytes)
        .map_err(|e| PdfError::Image(e.to_string()))?;

    let rgb = if dyn_img.color().has_alpha() {
        let rgba = dyn_img.to_rgba8();
        let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
        for (x, y, pixel) in rgba.enumerate_pixels() {
            let [r, g, b, a] = pixel.0;
            let alpha = a as f32 / 255.0;
            let nr = (r as f32 * alpha + 255.0 * (1.0 - alpha)).round() as u8;
            let ng = (g as f32 * alpha + 255.0 * (1.0 - alpha)).round() as u8;
            let nb = (b as f32 * alpha + 255.0 * (1.0 - alpha)).round() as u8;
            rgb.put_pixel(x, y, image::Rgb([nr, ng, nb]));
        }
        rgb
    } else {
        dyn_img.to_rgb8()
    };
    let (width, height) = rgb.dimensions();

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(rgb.as_raw())
        .and_then(|_| encoder.finish())
        .map(|compressed| {
            let dict = Dictionary::from_iter(vec![
                ("Type", Object::Name(b"XObject".to_vec())),
                ("Subtype", Object::Name(b"Image".to_vec())),
                ("Width", Object::Integer(i64::from(width))),
                ("Height", Object::Integer(i64::from(height))),
                ("ColorSpace", Object::Name(b"DeviceRGB".to_vec())),
                ("BitsPerComponent", Object::Integer(8)),
                ("Filter", Object::Name(b"FlateDecode".to_vec())),
            ]);
            EmbeddedImage {
                stream: Stream::new(dict, compressed),
                width,
                height,
            }
        })
        .map_err(PdfError::Io)
}

fn images_document(inputs: &[PathBuf], output: &Path) -> Result<OpResult, PdfError> {
    if inputs.is_empty() {
        return Err(PdfError::NotEnoughInputs(1));
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids: Vec<Object> = Vec::with_capacity(inputs.len());

    for input in inputs {
        let bytes = std::fs::read(input)?;
        let img = build_image_xobject(&bytes)?;
        drop(bytes);

        let img_id = doc.add_object(img.stream);
        let content = format!(
            "q\n{w} 0 0 {h} 0 0 cm\n/Im0 Do\nQ\n",
            w = img.width,
            h = img.height
        );
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

        let resources = Dictionary::from_iter(vec![(
            "XObject",
            Object::Dictionary(Dictionary::from_iter(vec![(
                "Im0",
                Object::Reference(img_id),
            )])),
        )]);
        let page = Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Page".to_vec())),
            ("Parent", Object::Reference(pages_id)),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(i64::from(img.width)),
                    Object::Integer(i64::from(img.height)),
                ]),
            ),
            ("Resources", Object::Dictionary(resources)),
            ("Contents", Object::Reference(content_id)),
        ]);
        kids.push(Object::Reference(doc.add_object(page)));
    }

    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Pages".to_vec())),
            ("Kids", Object::Array(kids)),
            ("Count", Object::Integer(count)),
        ])),
    );
    let catalog_id = doc.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]));
    doc.trailer.set("Root", Object::Reference(catalog_id));
    // NOTE: no doc.compress() here — JPEG streams must stay byte-identical.
    doc.save(output)?;

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: count as u32,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(doc);
    Ok(result)
}
