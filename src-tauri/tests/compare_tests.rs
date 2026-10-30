//! Comprehensive automated integration tests for the PDFKit Compare PDFs engine.
//! Covers all 20 mandatory scenarios specified in the engineering test plan.

use image::{ImageFormat, Rgba, RgbaImage};
use lopdf::{Dictionary, Document, Object, Stream};
use pdfkit_lib::commands::compare::{
    self, cancel_comparison, compare_page_rasters, compare_pdf_files,
    compare_pdf_files_with_progress, export_comparison_report, request_cancel_comparison,
    reset_cancel_flag, ChangeCategory, ChangeKind, ChangeLayer, CompareMode, CompareOptions,
    EnrichedPageData, EnrichedTextSpan, NormalizationOptions, PageMatchStatus,
    RasterCompareRequest,
};
use pdfkit_lib::PdfError;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn test_workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pdfkit_compare_test_{}_{}_{}",
        name,
        std::process::id(),
        TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tauri::async_runtime::block_on(fut)
}

#[derive(Clone)]
struct PageSpec {
    width: i64,
    height: i64,
    crop_box: Option<[i64; 4]>,
    rotate: i64,
    stream_ops: Vec<u8>,
    images: Vec<(String, u32, u32, Vec<u8>)>,
    annotations: Vec<(String, [i64; 4], String)>,
}

impl Default for PageSpec {
    fn default() -> Self {
        Self {
            width: 595,
            height: 842,
            crop_box: None,
            rotate: 0,
            stream_ops: Vec::new(),
            images: Vec::new(),
            annotations: Vec::new(),
        }
    }
}

/// Builds a synthetic PDF document with custom pages, fonts, images, annotations, and optional AcroForm fields.
fn build_synthetic_pdf(
    path: &Path,
    pages: &[PageSpec],
    form_fields: &[(&str, &str, &str, [i64; 4])],
) {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();

    let font_f1 = doc.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Font".to_vec())),
        ("Subtype", Object::Name(b"Type1".to_vec())),
        ("BaseFont", Object::Name(b"Helvetica".to_vec())),
        ("Encoding", Object::Name(b"WinAnsiEncoding".to_vec())),
    ]));
    let font_f2 = doc.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Font".to_vec())),
        ("Subtype", Object::Name(b"Type1".to_vec())),
        ("BaseFont", Object::Name(b"Times-Bold".to_vec())),
        ("Encoding", Object::Name(b"WinAnsiEncoding".to_vec())),
    ]));

    let mut kids = Vec::new();
    let mut first_page_id = None;

    for p in pages {
        let mut fonts_dict = Dictionary::new();
        fonts_dict.set("F1", Object::Reference(font_f1));
        fonts_dict.set("F2", Object::Reference(font_f2));

        let mut res_dict = Dictionary::new();
        res_dict.set("Font", Object::Dictionary(fonts_dict));

        if !p.images.is_empty() {
            let mut xobj_dict = Dictionary::new();
            for (name, w, h, raw_bytes) in &p.images {
                let img_dict = Dictionary::from_iter(vec![
                    ("Type", Object::Name(b"XObject".to_vec())),
                    ("Subtype", Object::Name(b"Image".to_vec())),
                    ("Width", Object::Integer(*w as i64)),
                    ("Height", Object::Integer(*h as i64)),
                    ("ColorSpace", Object::Name(b"DeviceRGB".to_vec())),
                    ("BitsPerComponent", Object::Integer(8)),
                ]);
                let img_id = doc.add_object(Stream::new(img_dict, raw_bytes.clone()));
                xobj_dict.set(name.as_bytes().to_vec(), Object::Reference(img_id));
            }
            res_dict.set("XObject", Object::Dictionary(xobj_dict));
        }

        let content_id = doc.add_object(Stream::new(Dictionary::new(), p.stream_ops.clone()));

        let mut page_dict = Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Page".to_vec())),
            ("Parent", Object::Reference(pages_id)),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(p.width),
                    Object::Integer(p.height),
                ]),
            ),
            ("Resources", Object::Dictionary(res_dict)),
            ("Contents", Object::Reference(content_id)),
        ]);

        if let Some(cb) = p.crop_box {
            page_dict.set(
                "CropBox",
                Object::Array(vec![
                    Object::Integer(cb[0]),
                    Object::Integer(cb[1]),
                    Object::Integer(cb[2]),
                    Object::Integer(cb[3]),
                ]),
            );
        }

        if p.rotate != 0 {
            page_dict.set("Rotate", Object::Integer(p.rotate));
        }

        if !p.annotations.is_empty() {
            let mut annots_arr = Vec::new();
            for (subtype, rect, contents) in &p.annotations {
                let annot_dict = Dictionary::from_iter(vec![
                    ("Type", Object::Name(b"Annot".to_vec())),
                    ("Subtype", Object::Name(subtype.as_bytes().to_vec())),
                    (
                        "Rect",
                        Object::Array(vec![
                            Object::Integer(rect[0]),
                            Object::Integer(rect[1]),
                            Object::Integer(rect[2]),
                            Object::Integer(rect[3]),
                        ]),
                    ),
                    (
                        "Contents",
                        pdfkit_lib::commands::forms::encode_pdf_string(contents),
                    ),
                ]);
                annots_arr.push(Object::Reference(doc.add_object(annot_dict)));
            }
            page_dict.set("Annots", Object::Array(annots_arr));
        }

        let pid = doc.add_object(page_dict);
        if first_page_id.is_none() {
            first_page_id = Some(pid);
        }
        kids.push(Object::Reference(pid));
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Pages".to_vec())),
            ("Kids", Object::Array(kids)),
            ("Count", Object::Integer(pages.len() as i64)),
        ])),
    );

    let mut catalog_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]);

    if !form_fields.is_empty() {
        let mut fields_arr = Vec::new();
        for &(name, ftype, val, rect) in form_fields {
            let mut fdict = Dictionary::from_iter(vec![
                ("FT", Object::Name(ftype.as_bytes().to_vec())),
                ("T", pdfkit_lib::commands::forms::encode_pdf_string(name)),
                ("V", pdfkit_lib::commands::forms::encode_pdf_string(val)),
                (
                    "Rect",
                    Object::Array(vec![
                        Object::Integer(rect[0]),
                        Object::Integer(rect[1]),
                        Object::Integer(rect[2]),
                        Object::Integer(rect[3]),
                    ]),
                ),
            ]);
            if let Some(pid) = first_page_id {
                fdict.set("P", Object::Reference(pid));
            }
            fields_arr.push(Object::Reference(doc.add_object(fdict)));
        }
        let acro_id = doc.add_object(Dictionary::from_iter(vec![(
            "Fields",
            Object::Array(fields_arr),
        )]));
        catalog_dict.set("AcroForm", Object::Reference(acro_id));
    }

    let catalog_id = doc.add_object(catalog_dict);
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.save(path).unwrap();
}

fn png_bytes_from_rgba(img: &RgbaImage) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).unwrap();
    buf.into_inner()
}

// ---------------------------------------------------------------------------
// 1. Aynı belge (Identical documents -> zero meaningful differences)
// ---------------------------------------------------------------------------
#[test]
fn scenario_01_identical_documents_zero_differences() {
    reset_cancel_flag();
    let ws = test_workspace("s01_identical");
    let a = ws.join("a.pdf");
    let b = ws.join("b.pdf");

    let page = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 720 Tm (PDFKit yerel belge karsilastirma motoru testi.) Tj ET".to_vec(),
        ..Default::default()
    };
    build_synthetic_pdf(&a, &[page.clone()], &[]);
    build_synthetic_pdf(&b, &[page], &[]);

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert_eq!(res.summary.total_changes, 0, "Identical PDFs must report 0 changes");
    assert_eq!(res.alignments.len(), 1);
    assert_eq!(res.alignments[0].status, PageMatchStatus::Identical);
}

// ---------------------------------------------------------------------------
// 2. Metin ekleme (Text insertion on right page & accurate coordinates)
// ---------------------------------------------------------------------------
#[test]
fn scenario_02_text_addition_detected_with_coordinates() {
    reset_cancel_flag();
    let ws = test_workspace("s02_text_add");
    let a = ws.join("old.pdf");
    let b = ws.join("new.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 60 700 Tm (Sozlesme maddeleri onaylandi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 60 700 Tm (Sozlesme maddeleri bugun resmen onaylandi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert_eq!(res.summary.text_added_count, 1);
    let added = res.changes.iter().find(|c| c.kind == ChangeKind::TextAdded).unwrap();
    assert_eq!(added.right_page, Some(1));
    assert_eq!(added.after_text.as_deref(), Some("bugun resmen"));
    let rect = added.right_rect.expect("Must have bounding box on right page");
    assert!(rect.x > 60.0 && rect.y > 100.0 && rect.y < 160.0);
}

// ---------------------------------------------------------------------------
// 3. Metin silme (Text deletion on left page & accurate coordinates)
// ---------------------------------------------------------------------------
#[test]
fn scenario_03_text_deletion_detected_with_coordinates() {
    reset_cancel_flag();
    let ws = test_workspace("s03_text_del");
    let a = ws.join("old.pdf");
    let b = ws.join("new.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 680 Tm (Gecici taslak ibaresi kaldirildi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 680 Tm (Gecici kaldirildi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert_eq!(res.summary.text_deleted_count, 1);
    let del = res.changes.iter().find(|c| c.kind == ChangeKind::TextDeleted).unwrap();
    assert_eq!(del.left_page, Some(1));
    assert_eq!(del.before_text.as_deref(), Some("taslak ibaresi"));
    assert!(del.left_rect.is_some());
}

// ---------------------------------------------------------------------------
// 4. Metin değiştirme (Granular word replacement, not full paragraph!)
// ---------------------------------------------------------------------------
#[test]
fn scenario_04_text_replacement_is_word_granular_not_whole_paragraph() {
    reset_cancel_flag();
    let ws = test_workspace("s04_text_mod");
    let a = ws.join("old.pdf");
    let b = ws.join("new.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Bu projede teslim suresi otuz is gunu olarak belirlenmistir) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Bu projede teslim suresi kirkbes is gunu olarak belirlenmistir) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert_eq!(res.summary.text_modified_count, 1);
    let m = res.changes.iter().find(|c| c.kind == ChangeKind::TextModified).unwrap();
    assert_eq!(m.before_text.as_deref(), Some("otuz"));
    assert_eq!(m.after_text.as_deref(), Some("kirkbes"));
}

// ---------------------------------------------------------------------------
// 5. Biçim değişikliği (Same text, different font/size/color -> Formatting)
// ---------------------------------------------------------------------------
#[test]
fn scenario_05_formatting_change_detected_without_false_text_diff() {
    reset_cancel_flag();
    let ws = test_workspace("s05_formatting");
    let a = ws.join("old.pdf");
    let b = ws.join("new.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 0 0 0 rg 1 0 0 1 50 700 Tm (Kurumsal Guvenlik Politikasi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"BT /F2 18 Tf 0.8 0.1 0.1 rg 1 0 0 1 50 700 Tm (Kurumsal Guvenlik Politikasi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert_eq!(res.summary.text_added_count, 0);
    assert_eq!(res.summary.text_deleted_count, 0);
    assert_eq!(res.summary.text_modified_count, 0);
    assert!(res.summary.formatting_changes_count >= 1);
    let fmt_item = res
        .changes
        .iter()
        .find(|c| c.category == ChangeCategory::Formatting && c.kind == ChangeKind::FormattingChanged)
        .unwrap();
    assert!(fmt_item.detail.contains("Font") && fmt_item.detail.contains("Boyut") && fmt_item.detail.contains("Renk"));
}

// ---------------------------------------------------------------------------
// 6. Görsel değişikliği (Image replacement and image movement)
// ---------------------------------------------------------------------------
#[test]
fn scenario_06_image_modification_and_movement_detected() {
    reset_cancel_flag();
    let ws = test_workspace("s06_image_diff");
    let a = ws.join("old.pdf");
    let b = ws.join("new_modified.pdf");
    let c = ws.join("new_moved.pdf");

    let img_red = vec![220u8, 20, 20, 220, 20, 20, 220, 20, 20, 220, 20, 20];
    let img_blue = vec![20u8, 40, 230, 20, 40, 230, 20, 40, 230, 20, 40, 230];

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 750 Tm (Rapor Basligi) Tj ET q 100 0 0 80 60 500 cm /Im1 Do Q".to_vec(),
            images: vec![("Im1".to_string(), 2, 2, img_red.clone())],
            ..Default::default()
        }],
        &[],
    );

    // Same position, different image bytes
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 750 Tm (Rapor Basligi) Tj ET q 100 0 0 80 60 500 cm /Im1 Do Q".to_vec(),
            images: vec![("Im1".to_string(), 2, 2, img_blue)],
            ..Default::default()
        }],
        &[],
    );

    // Same image bytes, shifted position
    build_synthetic_pdf(
        &c,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 750 Tm (Rapor Basligi) Tj ET q 100 0 0 80 220 350 cm /Im1 Do Q".to_vec(),
            images: vec![("Im1".to_string(), 2, 2, img_red)],
            ..Default::default()
        }],
        &[],
    );

    let res_mod = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert!(res_mod.changes.iter().any(|ch| ch.kind == ChangeKind::ImageModified));

    let res_mov = compare_pdf_files(&a, &c, CompareOptions::default()).unwrap();
    assert!(res_mov.changes.iter().any(|ch| ch.kind == ChangeKind::ImageMoved));
}

// ---------------------------------------------------------------------------
// 7. Vektörel değişiklik (Vector graphic / shape / table changes)
// ---------------------------------------------------------------------------
#[test]
fn scenario_07_vector_and_shape_changes_detected() {
    reset_cancel_flag();
    let ws = test_workspace("s07_vector");
    let a = ws.join("old.pdf");
    let b = ws.join("new.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"0 0 0 RG 1 w 50 500 120 60 re S".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"0.9 0.1 0.1 RG 3 w 50 500 200 90 re S 50 400 m 250 400 l S".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert!(res.changes.iter().any(|c| c.kind == ChangeKind::VectorChanged));
}

// ---------------------------------------------------------------------------
// 8. Anotasyon ve Form alanı değişikliği (Annotations & AcroForm values)
// ---------------------------------------------------------------------------
#[test]
fn scenario_08_annotations_and_form_field_changes_detected() {
    reset_cancel_flag();
    let ws = test_workspace("s08_annot_forms");
    let a = ws.join("old.pdf");
    let b = ws.join("new.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: "BT /F1 12 Tf 1 0 0 1 50 700 Tm (Başvuru Formu) Tj ET".as_bytes().to_vec(),
            annotations: vec![("Text".to_string(), [100, 600, 140, 640], "Eski inceleme notu".to_string())],
            ..Default::default()
        }],
        &[("AdSoyad", "Tx", "Ahmet Yılmaz", [100, 500, 250, 520])],
    );

    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: "BT /F1 12 Tf 1 0 0 1 50 700 Tm (Başvuru Formu) Tj ET".as_bytes().to_vec(),
            annotations: vec![
                ("Text".to_string(), [100, 600, 140, 640], "Güncellenmiş inceleme notu".to_string()),
                ("Highlight".to_string(), [50, 695, 160, 715], "Önemli başlık".to_string()),
            ],
            ..Default::default()
        }],
        &[
            ("AdSoyad", "Tx", "Çağdaş Polat", [100, 500, 250, 520]),
            ("Departman", "Tx", "Altyapı Yönetimi", [100, 460, 250, 480]),
        ],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert!(res.changes.iter().any(|c| c.kind == ChangeKind::AnnotationModified));
    assert!(res.changes.iter().any(|c| c.kind == ChangeKind::AnnotationAdded));
    assert!(res.changes.iter().any(|c| c.kind == ChangeKind::FormFieldModified));
    assert!(res.changes.iter().any(|c| c.kind == ChangeKind::FormFieldAdded));
}

// ---------------------------------------------------------------------------
// 9. Sayfa ekleme ve silme (Subsequent pages NOT marked as false positives)
// ---------------------------------------------------------------------------
#[test]
fn scenario_09_page_insertion_and_deletion_without_subsequent_false_positives() {
    reset_cancel_flag();
    let ws = test_workspace("s09_page_add_del");
    let a = ws.join("three_pages.pdf");
    let b = ws.join("four_pages_inserted_middle.pdf");

    let p1 = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Birinci sayfa sabit metin icerigi burada yer alir) Tj ET".to_vec(),
        ..Default::default()
    };
    let p_inserted = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Araya yeni eklenen tamamen farkli ara rapor sayfasi) Tj ET".to_vec(),
        ..Default::default()
    };
    let p2 = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Ikincil bolum teknik mimari detaylari ve protokoller) Tj ET".to_vec(),
        ..Default::default()
    };
    let p3 = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Sonuc ve degerlendirme bolumu kapanis maddeleri) Tj ET".to_vec(),
        ..Default::default()
    };

    build_synthetic_pdf(&a, &[p1.clone(), p2.clone(), p3.clone()], &[]);
    build_synthetic_pdf(&b, &[p1, p_inserted, p2, p3], &[]);

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert_eq!(res.summary.pages_added_count, 1, "Exactly 1 added page");
    assert_eq!(res.summary.pages_identical_count, 3, "Remaining 3 pages must match as Identical!");
    assert_eq!(res.summary.text_added_count, 0, "Subsequent pages must NOT produce false text diffs");
    assert_eq!(res.summary.text_modified_count, 0);
}

// ---------------------------------------------------------------------------
// 10. Sayfa taşıma ve sıralama (Reordered pages matched accurately)
// ---------------------------------------------------------------------------
#[test]
fn scenario_10_page_reordering_and_moving_detected() {
    reset_cancel_flag();
    let ws = test_workspace("s10_page_move");
    let a = ws.join("order_123.pdf");
    let b = ws.join("order_132.pdf");

    let p1 = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Bolum Bir Giris ve Genel Kapsam Tanimlari) Tj ET".to_vec(),
        ..Default::default()
    };
    let p2 = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Bolum Iki Finansal Tablolar ve Butce Planlamasi) Tj ET".to_vec(),
        ..Default::default()
    };
    let p3 = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Bolum Uc Guvenlik Denetimi ve Kriptografik Mimari) Tj ET".to_vec(),
        ..Default::default()
    };

    build_synthetic_pdf(&a, &[p1.clone(), p2.clone(), p3.clone()], &[]);
    build_synthetic_pdf(&b, &[p1, p3, p2], &[]);

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert!(res.summary.pages_moved_count >= 1, "Moved page must be detected");
    assert!(res.changes.iter().any(|c| c.kind == ChangeKind::PageMoved));
    assert_eq!(res.summary.text_modified_count, 0, "Moved pages should not generate false text modifications");
}

// ---------------------------------------------------------------------------
// 11. Tarama PDF'si (Scanned / image-only PDF without text layer)
// ---------------------------------------------------------------------------
#[test]
fn scenario_11_scanned_pdf_visual_comparison_and_text_limitation_warning() {
    reset_cancel_flag();
    let ws = test_workspace("s11_scanned");
    let a = ws.join("scan1.pdf");
    let b = ws.join("scan2.pdf");

    let scan_img_1 = vec![240u8; 300];
    let mut scan_img_2 = vec![240u8; 300];
    scan_img_2[50] = 10;

    // Full-page image XObject with zero text operators
    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"q 550 0 0 780 20 30 cm /ScanIm Do Q".to_vec(),
            images: vec![("ScanIm".to_string(), 10, 10, scan_img_1)],
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"q 550 0 0 780 20 30 cm /ScanIm Do Q".to_vec(),
            images: vec![("ScanIm".to_string(), 10, 10, scan_img_2)],
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert!(res.alignments[0].is_scanned, "Page must be flagged as scanned");
    assert!(
        res.uncertainties.iter().any(|u| u.contains("taranmış") || u.contains("metin katmanı")),
        "Must explicitly warn that text comparison is limited on scanned pages"
    );
    assert!(res.changes.iter().any(|c| c.layer == ChangeLayer::Visual));
}

// ---------------------------------------------------------------------------
// 12. Türkçe ve Unicode (İ, ı, Ş, ş, Ğ, ğ, Ç, ç, Ö, ö, Ü, ü)
// ---------------------------------------------------------------------------
#[test]
fn scenario_12_turkish_unicode_and_dotted_dotless_i_accuracy() {
    reset_cancel_flag();
    let ws = test_workspace("s12_turkish");
    let a = ws.join("tr_a.pdf");
    let b = ws.join("tr_b.pdf");
    let c = ws.join("tr_case.pdf");

    // UTF-8 stream with full Turkish characters
    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: "BT /F1 12 Tf 1 0 0 1 50 700 Tm (İSTANBUL IŞIK Şanlıurfa Ğümüşhane Çanakkale Ödemiş Ürgüp ıspanak) Tj ET"
                .as_bytes()
                .to_vec(),
            ..Default::default()
        }],
        &[],
    );

    // Change only "IŞIK" (dotless I) to "İŞİK" (dotted İ) -> must be detected as a change when case-sensitive!
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: "BT /F1 12 Tf 1 0 0 1 50 700 Tm (İSTANBUL İŞİK Şanlıurfa Ğümüşhane Çanakkale Ödemiş Ürgüp ıspanak) Tj ET"
                .as_bytes()
                .to_vec(),
            ..Default::default()
        }],
        &[],
    );

    // Lowercase Turkish equivalents: "İSTANBUL" -> "istanbul", "IŞIK" -> "ışık"
    build_synthetic_pdf(
        &c,
        &[PageSpec {
            stream_ops: "BT /F1 12 Tf 1 0 0 1 50 700 Tm (istanbul ışık şanlıurfa ğümüşhane çanakkale ödemiş ürgüp ISPANAK) Tj ET"
                .as_bytes()
                .to_vec(),
            ..Default::default()
        }],
        &[],
    );

    // 1. "IŞIK" vs "İŞİK" must NEVER be treated as equal even with ignore_case: true!
    let mut opts_ci = CompareOptions::default();
    opts_ci.normalization.ignore_case = true;
    let res_dot_diff = compare_pdf_files(&a, &b, opts_ci.clone()).unwrap();
    assert_eq!(
        res_dot_diff.summary.text_modified_count, 1,
        "Turkish IŞIK (ı) vs İŞİK (i) must be distinguished even in case-insensitive mode!"
    );

    // 2. "İSTANBUL IŞIK ... ıspanak" vs "istanbul ışık ... ISPANAK" MUST match with 0 diffs when ignore_case: true!
    let res_tr_case = compare_pdf_files(&a, &c, opts_ci).unwrap();
    assert_eq!(
        res_tr_case.summary.total_changes, 0,
        "Turkish İ->i and I->ı case folding must match with 0 differences when ignore_case is enabled"
    );
}

// ---------------------------------------------------------------------------
// 13. Satır sonları ve ligatürler (Hyphenation, ligatures, line breaks)
// ---------------------------------------------------------------------------
#[test]
fn scenario_13_hyphenation_ligatures_and_line_breaks_normalization() {
    reset_cancel_flag();
    let ws = test_workspace("s13_normalization");
    let a = ws.join("norm_a.pdf");
    let b = ws.join("norm_b.pdf");

    // Page A has hyphenated line break "karşı-\nlaştırma" and ligature "ﬁlters"
    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: "BT /F1 12 Tf 1 0 0 1 50 700 Tm (Belge karşı-) Tj 0 -16 Td (laştırma \u{FB01}lters aktif) Tj ET"
                .as_bytes()
                .to_vec(),
            ..Default::default()
        }],
        &[],
    );
    // Page B has unhyphenated "karşılaştırma" and expanded "filters"
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: "BT /F1 12 Tf 1 0 0 1 50 700 Tm (Belge karşılaştırma filters aktif) Tj ET"
                .as_bytes()
                .to_vec(),
            ..Default::default()
        }],
        &[],
    );

    // With default normalization (normalize_hyphenation=true, normalize_ligatures=true, ignore_whitespace=true) -> 0 diffs
    let res_norm = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert_eq!(res_norm.summary.total_changes, 0);

    // With normalization disabled -> differences must be surfaced
    let opts_strict = CompareOptions {
        normalization: NormalizationOptions {
            ignore_case: false,
            ignore_whitespace: false,
            normalize_hyphenation: false,
            unicode_normalization: true,
            normalize_ligatures: false,
        },
        ..Default::default()
    };
    let res_strict = compare_pdf_files(&a, &b, opts_strict).unwrap();
    assert!(res_strict.summary.total_changes >= 1);
}

// ---------------------------------------------------------------------------
// 14. Geometri (Rotation, CropBox, different page sizes & unstretched visual diff)
// ---------------------------------------------------------------------------
#[test]
fn scenario_14_geometry_rotation_cropbox_and_size_differences() {
    reset_cancel_flag();
    let ws = test_workspace("s14_geometry");
    let a = ws.join("portrait.pdf");
    let b = ws.join("rotated_cropped.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            width: 595,
            height: 842,
            rotate: 0,
            crop_box: None,
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Geometri Testi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            width: 595,
            height: 842,
            rotate: 90,
            crop_box: Some([10, 10, 580, 820]),
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Geometri Testi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert!(res.alignments[0].geometry_differs);
    assert!(res.changes.iter().any(|c| c.kind == ChangeKind::PageGeometryChanged));

    // Also verify visual raster comparison flags geometry incompatibility without crashing
    let img1 = RgbaImage::from_pixel(100, 140, Rgba([255, 255, 255, 255]));
    let img2 = RgbaImage::from_pixel(140, 100, Rgba([255, 255, 255, 255]));
    let raster_res = block_on(compare_page_rasters(RasterCompareRequest {
        left_page: 1,
        right_page: 1,
        left_png: png_bytes_from_rgba(&img1),
        right_png: png_bytes_from_rgba(&img2),
        left_dims_pt: [595.0, 842.0],
        right_dims_pt: [842.0, 595.0],
        tolerance: 24,
        ignore_antialiasing: true,
        exclude_rects_pt: vec![],
    }))
    .unwrap();
    assert!(!raster_res.geometry_compatible);
    assert!(raster_res.geometry_warning.is_some());
}

// ---------------------------------------------------------------------------
// 15. Bozuk ve parola korumalı PDF (Controlled error, no panic)
// ---------------------------------------------------------------------------
#[test]
fn scenario_15_corrupted_and_encrypted_pdf_controlled_errors() {
    reset_cancel_flag();
    let ws = test_workspace("s15_errors");
    let valid = ws.join("valid.pdf");
    let corrupt = ws.join("corrupt.pdf");
    let encrypted = ws.join("encrypted.pdf");

    build_synthetic_pdf(
        &valid,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Valid) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    std::fs::write(&corrupt, b"NOT_A_VALID_PDF_FILE_CONTENT").unwrap();

    // Create encrypted PDF using PDFKit's security module
    block_on(pdfkit_lib::commands::security::encrypt_pdf(
        valid.to_string_lossy().into_owned(),
        encrypted.to_string_lossy().into_owned(),
        "secret123".into(),
        "".into(),
        None,
        Some("aes256".into()),
    ))
    .unwrap();

    let err_corrupt = compare_pdf_files(&valid, &corrupt, CompareOptions::default());
    assert!(err_corrupt.is_err(), "Corrupt PDF must return controlled Err");

    let err_enc = compare_pdf_files(&valid, &encrypted, CompareOptions::default());
    assert!(
        matches!(err_enc, Err(PdfError::Encrypted)),
        "Encrypted PDF must return PdfError::Encrypted"
    );
}

// ---------------------------------------------------------------------------
// 16. Büyük belgeler ve iptal mekanizması (Large multi-page doc & cancellation)
// ---------------------------------------------------------------------------
#[test]
fn scenario_16_large_document_performance_and_cancellation() {
    reset_cancel_flag();
    let ws = test_workspace("s16_large");
    let a = ws.join("large_a.pdf");
    let b = ws.join("large_b.pdf");

    let mut pages_a = Vec::new();
    let mut pages_b = Vec::new();
    for p in 1..=40 {
        pages_a.push(PageSpec {
            stream_ops: format!(
                "BT /F1 12 Tf 1 0 0 1 50 700 Tm (Buyuk belge sayfa numarasi {} standart metin satiri) Tj ET",
                p
            )
            .into_bytes(),
            ..Default::default()
        });
        let text_b = if p == 25 {
            format!(
                "BT /F1 12 Tf 1 0 0 1 50 700 Tm (Buyuk belge sayfa numarasi {} degistirilmis metin satiri) Tj ET",
                p
            )
        } else {
            format!(
                "BT /F1 12 Tf 1 0 0 1 50 700 Tm (Buyuk belge sayfa numarasi {} standart metin satiri) Tj ET",
                p
            )
        };
        pages_b.push(PageSpec {
            stream_ops: text_b.into_bytes(),
            ..Default::default()
        });
    }

    build_synthetic_pdf(&a, &pages_a, &[]);
    build_synthetic_pdf(&b, &pages_b, &[]);

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert_eq!(res.left_page_count, 40);
    assert_eq!(res.summary.total_changes, 1);
    assert_eq!(res.summary.changed_left_pages, vec![25]);

    // Test cancellation mechanism
    request_cancel_comparison();
    let cancelled = compare_pdf_files(&a, &b, CompareOptions::default());
    assert!(cancelled.is_err(), "Cancelled comparison must abort immediately");
    cancel_comparison();
    reset_cancel_flag();
}

// ---------------------------------------------------------------------------
// 17. Senkronize kaydırma ve sayfa eşleştirme takibi
// ---------------------------------------------------------------------------
#[test]
fn scenario_17_synchronized_scrolling_alignment_mapping() {
    reset_cancel_flag();
    let ws = test_workspace("s17_sync_scroll");
    let a = ws.join("a.pdf");
    let b = ws.join("b.pdf");

    let p1 = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Birinci sayfa ortak metin) Tj ET".to_vec(),
        ..Default::default()
    };
    let p_added = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Yeni eklenen ara sayfa bambaska icerik) Tj ET".to_vec(),
        ..Default::default()
    };
    let p2 = PageSpec {
        stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Ikinci sayfa ortak sonuc metni) Tj ET".to_vec(),
        ..Default::default()
    };

    build_synthetic_pdf(&a, &[p1.clone(), p2.clone()], &[]);
    build_synthetic_pdf(&b, &[p1, p_added, p2], &[]);

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    // Verify Left Page 2 is aligned to Right Page 3 (not blindly bound to Page 2!)
    let align_p2 = res
        .alignments
        .iter()
        .find(|al| al.left_page == Some(2))
        .expect("Left page 2 must be in alignments");
    assert_eq!(
        align_p2.right_page,
        Some(3),
        "Synchronized scrolling model must map Left Page 2 to Right Page 3 when a page was inserted at index 2"
    );
}

// ---------------------------------------------------------------------------
// 18. Bağımsız kaydırma ve mod izolasyonu
// ---------------------------------------------------------------------------
#[test]
fn scenario_18_independent_mode_and_text_moved_detection() {
    reset_cancel_flag();
    let ws = test_workspace("s18_moved_text");
    let a = ws.join("a.pdf");
    let b = ws.join("b.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 720 Tm (Tasinacak onemli uyari blogu) Tj 1 0 0 1 50 500 Tm (Sabit ana paragraf metni burada yer aliyor) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 500 Tm (Sabit ana paragraf metni burada yer aliyor) Tj 1 0 0 1 50 280 Tm (Tasinacak onemli uyari blogu) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert!(
        res.changes.iter().any(|c| c.kind == ChangeKind::TextMoved),
        "Relocated text block must be classified as TextMoved"
    );
}

// ---------------------------------------------------------------------------
// 19. Değişiklik navigasyonu, filtreler ve özet tutarlılığı
// ---------------------------------------------------------------------------
#[test]
fn scenario_19_filters_summary_and_change_navigation_consistency() {
    reset_cancel_flag();
    let ws = test_workspace("s19_filters");
    let a = ws.join("a.pdf");
    let b = ws.join("b.pdf");

    build_synthetic_pdf(
        &a,
        &[
            PageSpec {
                stream_ops: b"BT /F1 10 Tf 1 0 0 1 50 810 Tm (Kurumsal Rapor Ustbilgi) Tj /F1 12 Tf 1 0 0 1 50 650 Tm (Govde metni eski kelime) Tj ET".to_vec(),
                ..Default::default()
            },
            PageSpec {
                stream_ops: b"BT /F1 10 Tf 1 0 0 1 50 810 Tm (Kurumsal Rapor Ustbilgi) Tj /F1 12 Tf 1 0 0 1 50 650 Tm (Ikinci sayfa sabit metin) Tj ET".to_vec(),
                ..Default::default()
            },
        ],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[
            PageSpec {
                stream_ops: b"BT /F1 10 Tf 1 0 0 1 50 810 Tm (Kurumsal Rapor Yeni Ustbilgi) Tj /F2 16 Tf 1 0 0 1 50 650 Tm (Govde metni yeni kelime) Tj ET".to_vec(),
                ..Default::default()
            },
            PageSpec {
                stream_ops: b"BT /F1 10 Tf 1 0 0 1 50 810 Tm (Kurumsal Rapor Yeni Ustbilgi) Tj /F1 12 Tf 1 0 0 1 50 650 Tm (Ikinci sayfa sabit metin) Tj ET".to_vec(),
                ..Default::default()
            },
        ],
        &[],
    );

    let res_auto = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();
    assert!(res_auto.changes.iter().any(|c| c.category == ChangeCategory::Text));
    assert!(res_auto.changes.iter().any(|c| c.category == ChangeCategory::Formatting));
    assert!(res_auto.changes.iter().any(|c| c.category == ChangeCategory::HeaderFooter));
    for c in &res_auto.changes {
        assert!(c.left_page.is_some() || c.right_page.is_some());
        assert!(c.left_rect.is_some() || c.right_rect.is_some());
    }

    // Verify TextOnly mode filters out Formatting and HeaderFooter
    let res_text_only = compare_pdf_files(
        &a,
        &b,
        CompareOptions {
            mode: CompareMode::TextOnly,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(res_text_only.changes.iter().all(|c| c.category == ChangeCategory::Text));
}

// ---------------------------------------------------------------------------
// 20. Çevrimdışı Rapor Dışa Aktarma & Görsel Anti-Aliasing Regresyon Testi
// ---------------------------------------------------------------------------
#[test]
fn scenario_20_offline_report_export_and_visual_antialiasing_filter() {
    reset_cancel_flag();
    let ws = test_workspace("s20_export_and_aa");
    let a = ws.join("a.pdf");
    let b = ws.join("b.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Rapor Testi Eski) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );
    build_synthetic_pdf(
        &b,
        &[PageSpec {
            stream_ops: b"BT /F1 12 Tf 1 0 0 1 50 700 Tm (Rapor Testi Yeni) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let res = compare_pdf_files(&a, &b, CompareOptions::default()).unwrap();

    // Export JSON, CSV, and HTML reports locally
    let json_path = ws.join("report.json");
    let csv_path = ws.join("report.csv");
    let html_path = ws.join("report.html");

    block_on(export_comparison_report(
        res.clone(),
        json_path.to_string_lossy().into_owned(),
        "json".into(),
    ))
    .unwrap();
    block_on(export_comparison_report(
        res.clone(),
        csv_path.to_string_lossy().into_owned(),
        "csv".into(),
    ))
    .unwrap();
    block_on(export_comparison_report(
        res,
        html_path.to_string_lossy().into_owned(),
        "html".into(),
    ))
    .unwrap();

    assert!(json_path.exists() && std::fs::metadata(&json_path).unwrap().len() > 50);
    assert!(csv_path.exists() && std::fs::metadata(&csv_path).unwrap().len() > 20);
    assert!(html_path.exists() && std::fs::metadata(&html_path).unwrap().len() > 100);

    // Visual regression test:
    // 1) Two identical renders with 1px anti-aliasing edge jitter must produce 0 visual diff regions
    let mut img_base = RgbaImage::from_pixel(120, 120, Rgba([255, 255, 255, 255]));
    for y in 20..80 {
        for x in 40..60 {
            img_base.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let mut img_aa = img_base.clone();
    // Simulate anti-aliased edge pixel along the vertical edge x=40
    img_aa.put_pixel(40, 35, Rgba([40, 40, 40, 255]));

    let aa_res = compare::visual_diff::compare_rgba_images(
        1,
        1,
        &img_base,
        &img_aa,
        [120.0, 120.0],
        [120.0, 120.0],
        24,
        true,
        &[],
        false,
    );
    assert_eq!(
        aa_res.regions_pt.len(),
        0,
        "1px anti-aliased edge jitter must NOT produce false-positive visual diff regions"
    );

    // 2) Real visual block change MUST produce a visual diff region and heatmap
    let mut img_changed = img_base.clone();
    for y in 85..105 {
        for x in 85..105 {
            img_changed.put_pixel(x, y, Rgba([220, 20, 60, 255]));
        }
    }
    let real_res = compare::visual_diff::compare_rgba_images(
        1,
        1,
        &img_base,
        &img_changed,
        [120.0, 120.0],
        [120.0, 120.0],
        24,
        true,
        &[],
        true,
    );
    assert_eq!(real_res.regions_pt.len(), 1, "Real visual change must be detected as 1 merged region");
    assert!(real_res.heatmap_png.is_some(), "Heatmap PNG must be generated");
}

// ---------------------------------------------------------------------------
// 21. Aynı PDF + pdf.js g_d<N>_f<M> font önekleri ile 0 sahte biçim farkı
// ---------------------------------------------------------------------------
#[test]
fn scenario_21_identical_pdf_with_pdfjs_font_prefixes_zero_false_positives() {
    reset_cancel_flag();
    let ws = test_workspace("s21_pdfjs_font_prefix_regression");
    let a = ws.join("same_doc.pdf");

    build_synthetic_pdf(
        &a,
        &[PageSpec {
            stream_ops: b"BT /F1 23 Tf 1 0 0 1 50 700 Tm (AYT Biyoloji Soru Kitabi) Tj ET".to_vec(),
            ..Default::default()
        }],
        &[],
    );

    let left_enriched = vec![EnrichedPageData {
        page: 1,
        spans: vec![EnrichedTextSpan {
            text: "AYT Biyoloji Soru Kitabi".into(),
            x: 50.0,
            y: 120.0,
            width: 240.0,
            height: 23.0,
            font_name: "g_d16_f1_sans-serif".into(),
            font_size: 23.0,
        }],
    }];

    let right_enriched = vec![EnrichedPageData {
        page: 1,
        spans: vec![EnrichedTextSpan {
            text: "AYT Biyoloji Soru Kitabi".into(),
            x: 50.0,
            y: 120.0,
            width: 240.0,
            height: 23.0,
            font_name: "g_d17_f1_sans-serif".into(),
            font_size: 23.0,
        }],
    }];

    let res = compare_pdf_files_with_progress(
        &a,
        &a,
        CompareOptions::default(),
        Some(&left_enriched),
        Some(&right_enriched),
        |_, _, _, _| {},
    )
    .unwrap();

    assert_eq!(
        res.summary.formatting_changes_count, 0,
        "pdf.js g_d16_f1_sans-serif vs g_d17_f1_sans-serif must NOT produce false-positive formatting changes"
    );
    assert_eq!(res.summary.total_changes, 0);
}
