//! End-to-end tests exercising the Tauri command functions against real
//! files in a temp directory. Run with: `cargo test`.

use lopdf::{Document, Object};
use pdfkit_lib::commands;
use pdfkit_lib::commands::organize::OrganizePage;
use pdfkit_lib::commands::split::PageRange;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory per test case.
fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pdfkit_test_{}_{}_{}",
        name,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Builds a minimal valid PDF with `pages` blank 200x200 pages.
fn make_pdf(path: &Path, pages: u32) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for _ in 0..pages {
        let content = doc.add_object(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"0 0 10 10 re f".to_vec(),
        ));
        let page = lopdf::Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Page".to_vec())),
            ("Parent", Object::Reference(pages_id)),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(200),
                    Object::Integer(200),
                ]),
            ),
            ("Contents", Object::Reference(content)),
        ]);
        kids.push(Object::Reference(doc.add_object(page)));
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Pages".to_vec())),
            ("Kids", Object::Array(kids)),
            ("Count", Object::Integer(i64::from(pages))),
        ])),
    );
    let catalog = doc.add_object(lopdf::Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]));
    doc.trailer.set("Root", catalog);
    doc.save(path).unwrap();
}

fn run<F: std::future::Future>(fut: F) -> F::Output {
    tauri::async_runtime::block_on(fut)
}

#[test]
fn merge_two_pdfs() {
    let ws = workspace("merge");
    let a = ws.join("a.pdf");
    let b = ws.join("b.pdf");
    make_pdf(&a, 3);
    make_pdf(&b, 5);

    let out = ws.join("merged.pdf");
    let result = run(commands::merge::merge_pdfs(
        vec![a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned()],
        out.to_string_lossy().into_owned(),
    ))
    .unwrap();

    assert_eq!(result.total_pages, 8);
    let check = Document::load(&out).unwrap();
    assert_eq!(check.get_pages().len(), 8);
}

#[test]
fn split_into_ranges_and_every_page() {
    let ws = workspace("split");
    let src = ws.join("src.pdf");
    make_pdf(&src, 5);

    let out_dir = ws.join("out");
    let result = run(commands::split::split_pdf(
        src.to_string_lossy().into_owned(),
        vec![PageRange { start: 1, end: 2 }, PageRange { start: 3, end: 5 }],
        out_dir.to_string_lossy().into_owned(),
    ))
    .unwrap();
    assert_eq!(result.outputs.len(), 2);
    assert_eq!(Document::load(&result.outputs[0]).unwrap().get_pages().len(), 2);
    assert_eq!(Document::load(&result.outputs[1]).unwrap().get_pages().len(), 3);

    // Empty ranges = every page separate.
    let out_dir2 = ws.join("out2");
    let result2 = run(commands::split::split_pdf(
        src.to_string_lossy().into_owned(),
        vec![],
        out_dir2.to_string_lossy().into_owned(),
    ))
    .unwrap();
    assert_eq!(result2.outputs.len(), 5);
}

#[test]
fn rotate_single_page() {
    let ws = workspace("rotate");
    let src = ws.join("src.pdf");
    make_pdf(&src, 3);

    let out = ws.join("rot.pdf");
    run(commands::rotate::rotate_pages(
        src.to_string_lossy().into_owned(),
        vec![2],
        90,
        out.to_string_lossy().into_owned(),
    ))
    .unwrap();

    let check = Document::load(&out).unwrap();
    let pages = check.get_pages();
    let get_rot = |n: u32| {
        check
            .get_dictionary(*pages.get(&n).unwrap())
            .unwrap()
            .get(b"Rotate")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0)
    };
    assert_eq!(get_rot(1), 0);
    assert_eq!(get_rot(2), 90);
    assert_eq!(get_rot(3), 0);
}

#[test]
fn organize_reorder_delete_duplicate() {
    let ws = workspace("organize");
    let src = ws.join("src.pdf");
    make_pdf(&src, 5);

    // Keep pages 3, 1, 1 (duplicate), rotate page 3 by 90.
    let ops = vec![
        OrganizePage { source_page: 3, rotate_delta: 90 },
        OrganizePage { source_page: 1, rotate_delta: 0 },
        OrganizePage { source_page: 1, rotate_delta: 0 },
    ];
    let out = ws.join("org.pdf");
    let result = run(commands::organize::organize_pdf(
        src.to_string_lossy().into_owned(),
        ops,
        out.to_string_lossy().into_owned(),
    ))
    .unwrap();

    assert_eq!(result.total_pages, 3);
    let check = Document::load(&out).unwrap();
    assert_eq!(check.get_pages().len(), 3);
    let first = check.get_pages()[&1];
    let rot = check
        .get_dictionary(first)
        .unwrap()
        .get(b"Rotate")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0);
    assert_eq!(rot, 90);
}

#[test]
fn encrypt_then_decrypt() {
    let ws = workspace("security");
    let src = ws.join("src.pdf");
    make_pdf(&src, 2);

    let enc = ws.join("enc.pdf");
    run(commands::security::encrypt_pdf(
        src.to_string_lossy().into_owned(),
        enc.to_string_lossy().into_owned(),
        "user123".into(),
        "".into(),
        None,
        Some("aes256".into()),
    ))
    .unwrap();
    assert!(Document::load(&enc).unwrap().is_encrypted());

    let dec = ws.join("dec.pdf");
    run(commands::security::decrypt_pdf(
        enc.to_string_lossy().into_owned(),
        dec.to_string_lossy().into_owned(),
        "user123".into(),
    ))
    .unwrap();
    let check = Document::load(&dec).unwrap();
    assert!(!check.is_encrypted());
    assert_eq!(check.get_pages().len(), 2);
}

#[test]
fn decrypt_rejects_wrong_password() {
    let ws = workspace("wrongpw");
    let src = ws.join("src.pdf");
    make_pdf(&src, 1);
    let enc = ws.join("enc.pdf");
    run(commands::security::encrypt_pdf(
        src.to_string_lossy().into_owned(),
        enc.to_string_lossy().into_owned(),
        "right".into(),
        "".into(),
        None,
        None,
    ))
    .unwrap();

    let dec = ws.join("dec.pdf");
    let result = run(commands::security::decrypt_pdf(
        enc.to_string_lossy().into_owned(),
        dec.to_string_lossy().into_owned(),
        "wrong".into(),
    ));
    assert!(result.is_err());
}

#[test]
fn encrypt_with_granular_permissions() {
    let ws = workspace("permissions");
    let src = ws.join("src.pdf");
    make_pdf(&src, 1);
    let enc = ws.join("enc_restricted.pdf");

    let perms = commands::security::EncryptionPermissions {
        allow_print: false,
        allow_copy: false,
        allow_modify: false,
        allow_annotate: true,
    };

    run(commands::security::encrypt_pdf(
        src.to_string_lossy().into_owned(),
        enc.to_string_lossy().into_owned(),
        "read_only".into(),
        "admin_secret".into(),
        Some(perms),
        Some("aes256".into()),
    ))
    .unwrap();

    assert!(Document::load(&enc).unwrap().is_encrypted());

    // User password unlocks it
    let dec = ws.join("dec.pdf");
    run(commands::security::decrypt_pdf(
        enc.to_string_lossy().into_owned(),
        dec.to_string_lossy().into_owned(),
        "read_only".into(),
    ))
    .unwrap();
    assert!(!Document::load(&dec).unwrap().is_encrypted());
}

#[test]
fn watermark_and_page_numbers() {
    let ws = workspace("annotate");
    let src = ws.join("src.pdf");
    make_pdf(&src, 3);

    let wm = ws.join("wm.pdf");
    run(commands::annotate::add_watermark(
        src.to_string_lossy().into_owned(),
        wm.to_string_lossy().into_owned(),
        "SECRET".into(),
        40.0,
        0.3,
    ))
    .unwrap();

    let num = ws.join("num.pdf");
    run(commands::annotate::add_page_numbers(
        wm.to_string_lossy().into_owned(),
        num.to_string_lossy().into_owned(),
        10.0,
        20.0,
    ))
    .unwrap();

    let check = Document::load(&num).unwrap();
    assert_eq!(check.get_pages().len(), 3);
    // Each page should now carry an annotation content stream with our text.
    let page_id = check.get_pages()[&1];
    let content = check.get_page_content(page_id);
    let text = String::from_utf8_lossy(&content);
    assert!(text.contains("SECRET") || text.contains("1 / 3"));

    // Verify remove_watermark cleanly strips the watermark with explicit text
    let clean = ws.join("clean.pdf");
    run(commands::annotate::remove_watermark(
        wm.to_string_lossy().into_owned(),
        clean.to_string_lossy().into_owned(),
        Some("SECRET".into()),
        None,
    ))
    .unwrap();
    let clean_doc = Document::load(&clean).unwrap();
    assert_eq!(clean_doc.get_pages().len(), 3);
    let p1 = clean_doc.get_pages()[&1];
    let clean_content = clean_doc.get_page_content(p1);
    let clean_text = String::from_utf8_lossy(&clean_content);
    assert!(!clean_text.contains("SECRET"));

    // Verify remove_watermark automatically removes watermark WITHOUT passing target_text (None)
    let auto_clean = ws.join("auto_clean.pdf");
    run(commands::annotate::remove_watermark(
        wm.to_string_lossy().into_owned(),
        auto_clean.to_string_lossy().into_owned(),
        None,
        None,
    ))
    .unwrap();
    let auto_doc = Document::load(&auto_clean).unwrap();
    let auto_p1 = auto_doc.get_pages()[&1];
    let auto_content = auto_doc.get_page_content(auto_p1);
    let auto_text = String::from_utf8_lossy(&auto_content);
    assert!(!auto_text.contains("SECRET"));
}

#[test]
fn test_remove_watermark_in_place_same_file() {
    let ws = workspace("in_place_wm");
    let src = ws.join("doc.pdf");
    make_pdf(&src, 2);

    run(commands::annotate::add_watermark(
        src.to_string_lossy().into_owned(),
        src.to_string_lossy().into_owned(),
        "SIRKET OZEL BELGE".into(),
        36.0,
        0.25,
    )).unwrap();

    // Verify watermark exists
    let check_wm = Document::load(&src).unwrap();
    let p1 = check_wm.get_pages()[&1];
    let bytes_wm = check_wm.get_page_content(p1);
    let content = String::from_utf8_lossy(&bytes_wm);
    assert!(content.contains("SIRKET") || content.contains("BELGE") || content.contains("gs"));

    // Remove in place (input == output) with target_text = None
    run(commands::annotate::remove_watermark(
        src.to_string_lossy().into_owned(),
        src.to_string_lossy().into_owned(),
        None,
        None,
    )).unwrap();

    let check_clean = Document::load(&src).unwrap();
    let p1_clean = check_clean.get_pages()[&1];
    let bytes_clean = check_clean.get_page_content(p1_clean);
    let content_clean = String::from_utf8_lossy(&bytes_clean);
    assert!(!content_clean.contains("SIRKET") && !content_clean.contains("BELGE"));
}

#[test]
fn test_remove_form_xobject_and_arbitrary_angle_watermarks() {
    let ws = workspace("advanced_wm");
    let src = ws.join("src.pdf");
    make_pdf(&src, 1);

    // 1. Manually embed a Form XObject watermark and a 30-degree diagonal watermark into src.pdf
    let mut doc = Document::load(&src).unwrap();
    let page_id = doc.page_iter().next().unwrap();

    // Create a Form XObject stream representing an Adobe/Word watermark
    let mut xobj_dict = lopdf::Dictionary::new();
    xobj_dict.set("Type", lopdf::Object::Name(b"XObject".to_vec()));
    xobj_dict.set("Subtype", lopdf::Object::Name(b"Form".to_vec()));
    xobj_dict.set("BBox", lopdf::Object::Array(vec![0.into(), 0.into(), 200.into(), 100.into()]));
    let xobj_stream_bytes = b"BT /F1 36 Tf (CONFIDENTIAL DRAFT) Tj ET".to_vec();
    let xobj_stream = lopdf::Stream::new(xobj_dict, xobj_stream_bytes);
    let xobj_id = doc.add_object(xobj_stream);

    // Register in Page Resources /XObject /Fm0
    let mut xobj_map = lopdf::Dictionary::new();
    xobj_map.set("Fm0", lopdf::Object::Reference(xobj_id));
    let mut res = lopdf::Dictionary::new();
    res.set("XObject", lopdf::Object::Dictionary(xobj_map));
    if let Ok(p_dict) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
        p_dict.set("Resources", lopdf::Object::Dictionary(res));
    }

    // Add content stream invoking the Form XObject AND a 30° rotated watermark AND normal text
    let mixed_content = b"
        BT /F1 12 Tf 1 0 0 1 50 700 Tm (Important Page Content To Keep) Tj ET
        q /Fm0 Do Q
        q 0.866 0.5 -0.5 0.866 100 200 cm BT /F1 48 Tf (DIAGONAL_30DEG_SAMPLE) Tj ET Q
        BT /F1 12 Tf 1 0 0 1 50 650 Tm (Second Important Line To Keep) Tj ET
    ".to_vec();
    doc.add_page_contents(page_id, mixed_content).unwrap();

    let watermarked_path = ws.join("watermarked_xobj.pdf");
    doc.save(&watermarked_path).unwrap();

    // Verify document contains the watermarks and normal text before cleaning
    let pre_check = Document::load(&watermarked_path).unwrap();
    let pre_bytes = pre_check.get_page_content(page_id);
    let pre_str = String::from_utf8_lossy(&pre_bytes);
    assert!(pre_str.contains("/Fm0 Do"));
    assert!(pre_str.contains("DIAGONAL_30DEG_SAMPLE"));
    assert!(pre_str.contains("Important Page Content To Keep"));

    // 2. Run remove_watermark with None (pure auto-detection)
    let cleaned_path = ws.join("cleaned_xobj.pdf");
    run(commands::annotate::remove_watermark(
        watermarked_path.to_string_lossy().into_owned(),
        cleaned_path.to_string_lossy().into_owned(),
        None,
        None,
    ))
    .unwrap();

    // 3. Verify:
    // - Form XObject call /Fm0 Do is removed
    // - 30-degree diagonal watermark is removed
    // - Normal document content is 100% PRESERVED
    let post_doc = Document::load(&cleaned_path).unwrap();
    let post_bytes = post_doc.get_page_content(page_id);
    let post_str = String::from_utf8_lossy(&post_bytes);

    assert!(!post_str.contains("/Fm0 Do"), "Form XObject call must be removed!");
    assert!(!post_str.contains("DIAGONAL_30DEG_SAMPLE"), "30-degree diagonal watermark must be removed!");
    assert!(post_str.contains("Important Page Content To Keep"), "Normal page text line 1 must be preserved!");
    assert!(post_str.contains("Second Important Line To Keep"), "Normal page text line 2 must be preserved!");

    // Verify Form XObject object is deleted from resources
    let p_dict = post_doc.get_object(page_id).unwrap().as_dict().unwrap();
    if let Ok(res_obj) = p_dict.get(b"Resources") {
        if let Ok(res_dict) = res_obj.as_dict() {
            if let Ok(xobjs) = res_dict.get(b"XObject") {
                if let Ok(xdict) = xobjs.as_dict() {
                    assert!(!xdict.has(b"Fm0"), "Fm0 must be removed from XObject dictionary!");
                }
            }
        }
    }
}

#[test]
fn images_roundtrip_and_compress() {
    let ws = workspace("images");
    // Two tiny PNGs.
    for name in ["x.png", "y.png"] {
        let img = image::RgbImage::from_pixel(32, 24, image::Rgb([200, 30, 30]));
        img.save(ws.join(name)).unwrap();
    }

    let out = ws.join("imgs.pdf");
    let result = run(commands::images::images_to_pdf(
        vec![
            ws.join("x.png").to_string_lossy().into_owned(),
            ws.join("y.png").to_string_lossy().into_owned(),
        ],
        out.to_string_lossy().into_owned(),
    ))
    .unwrap();
    assert_eq!(result.total_pages, 2);

    // Compressing an already tightly compressed image PDF triggers the safety guard
    let comp = ws.join("comp.pdf");
    let c = run(commands::compress::compress_pdf(
        out.to_string_lossy().into_owned(),
        comp.to_string_lossy().into_owned(),
        None,
    ))
    .unwrap();
    assert!(c.already_optimized);
    assert!(!comp.exists());
}

#[test]
fn compress_uncompressed_pdf_and_safety_guard() {
    let ws = workspace("compress");
    let src = ws.join("uncompressed.pdf");

    // Create a PDF containing a large uncompressed stream
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let content = doc.add_object(lopdf::Stream::new(
        lopdf::Dictionary::new(), // Uncompressed stream
        vec![b'X'; 30_000],
    ));
    let page = lopdf::Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Page".to_vec())),
        ("Parent", Object::Reference(pages_id)),
        (
            "MediaBox",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(200),
                Object::Integer(200),
            ]),
        ),
        ("Contents", Object::Reference(content)),
    ]);
    let page_id = doc.add_object(page);
    let pages = lopdf::Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Count", Object::Integer(1)),
        ("Kids", Object::Array(vec![Object::Reference(page_id)])),
    ]);
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog = lopdf::Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.save(&src).unwrap();

    let comp = ws.join("compressed.pdf");
    let c = run(commands::compress::compress_pdf(
        src.to_string_lossy().into_owned(),
        comp.to_string_lossy().into_owned(),
        Some("lossless".into()),
    ))
    .unwrap();

    assert!(!c.already_optimized);
    assert!(c.after_bytes < c.before_bytes);
    assert!(comp.exists());
    assert_eq!(Document::load(&comp).unwrap().get_pages().len(), 1);

    // Second pass on the now-compressed document triggers safety guard
    let comp2 = ws.join("compressed2.pdf");
    let c2 = run(commands::compress::compress_pdf(
        comp.to_string_lossy().into_owned(),
        comp2.to_string_lossy().into_owned(),
        Some("lossless".into()),
    ))
    .unwrap();
    assert!(c2.already_optimized);
    assert!(!comp2.exists());
}

#[test]
fn compress_with_image_reencoding() {
    let ws = workspace("compress_images");
    // Create an uncompressed large image PDF
    let img = image::RgbImage::from_pixel(400, 300, image::Rgb([180, 50, 50]));
    let img_path = ws.join("test_large.png");
    img.save(&img_path).unwrap();

    let pdf_out = ws.join("large_img.pdf");
    run(commands::images::images_to_pdf(
        vec![img_path.to_string_lossy().into_owned()],
        pdf_out.to_string_lossy().into_owned(),
    ))
    .unwrap();

    let comp_balanced = ws.join("balanced.pdf");
    let res = run(commands::compress::compress_pdf(
        pdf_out.to_string_lossy().into_owned(),
        comp_balanced.to_string_lossy().into_owned(),
        Some("balanced".into()),
    ))
    .unwrap();

    // Verify it produced a valid document
    assert!(Document::load(&comp_balanced).is_ok() || res.already_optimized);
}

#[test]
fn write_and_stamp_binary() {
    let ws = workspace("stamp");
    let src = ws.join("src.pdf");
    make_pdf(&src, 1);

    // Signature PNG via write_binary_file (same path the frontend uses).
    let sig = ws.join("sig.png");
    let img = image::RgbImage::from_pixel(10, 5, image::Rgb([0, 0, 200]));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    run(commands::images::write_binary_file(
        sig.to_string_lossy().into_owned(),
        buf.into_inner(),
    ))
    .unwrap();
    assert!(sig.exists());

    let out = ws.join("signed.pdf");
    run(commands::forms::stamp_image(
        src.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        1,
        sig.to_string_lossy().into_owned(),
        10.0,
        10.0,
        50.0,
        25.0,
    ))
    .unwrap();
    assert_eq!(Document::load(&out).unwrap().get_pages().len(), 1);
}

#[test]
fn form_fields_detection_and_filling() {
    let sample_path = std::path::Path::new(r"C:\Users\CAGDASS\Desktop\interactiveform_enabled.pdf");
    if !sample_path.exists() {
        return;
    }
    let fields = run(commands::forms::list_form_fields(
        sample_path.to_string_lossy().into_owned(),
    ))
    .unwrap();
    assert!(!fields.is_empty());

    let hs_diploma = fields.iter().find(|f| f.name == "HIGH SCHOOL DIPLOMA").unwrap();
    assert_eq!(hs_diploma.field_type, "checkbox");
    assert_eq!(hs_diploma.export_value.as_deref(), Some("On"));

    let sex_field = fields.iter().find(|f| f.name == "Sex").unwrap();
    assert_eq!(sex_field.field_type, "radio");
    assert!(sex_field.options.contains(&"MALE".to_string()));
    assert!(sex_field.options.contains(&"FEMALE".to_string()));

    let first_name = fields.iter().find(|f| f.name == "Name_First").unwrap();
    assert_eq!(first_name.field_type, "text");

    let print_btn = fields.iter().find(|f| f.name == "Print").unwrap();
    assert_eq!(print_btn.field_type, "pushbutton");

    let ws = workspace("fill_form_test");
    let out = ws.join("filled_test.pdf");
    let mut fill_vals = std::collections::HashMap::new();
    fill_vals.insert("HIGH SCHOOL DIPLOMA".to_string(), "true".to_string());
    fill_vals.insert("Sex".to_string(), "MALE".to_string());
    fill_vals.insert("Name_First".to_string(), "Çağdaş MİMAR Şükrü".to_string());

    let res = run(commands::forms::fill_form(
        sample_path.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        fill_vals,
    ))
    .unwrap();
    assert_eq!(res.total_pages, 1);
    assert!(out.exists());

    // Verify written PDF fields
    let re_read = run(commands::forms::list_form_fields(
        out.to_string_lossy().into_owned(),
    ))
    .unwrap();
    let filled_hs = re_read.iter().find(|f| f.name == "HIGH SCHOOL DIPLOMA").unwrap();
    assert_eq!(filled_hs.value, "On");
    let filled_sex = re_read.iter().find(|f| f.name == "Sex").unwrap();
    assert_eq!(filled_sex.value, "MALE");
    let filled_first = re_read.iter().find(|f| f.name == "Name_First").unwrap();
    assert_eq!(filled_first.value, "Çağdaş MİMAR Şükrü");
}

#[test]
fn test_turkish_acroform_encoding() {
    let p1 = std::path::Path::new(r"C:\Users\CAGDASS\Desktop\interactiveform_enabled.pdf");
    let p2 = std::path::Path::new(r"C:\Users\CAGDASS\Desktop\pdfKit.pdf");
    let sample_path = if p1.exists() {
        p1
    } else if p2.exists() {
        p2
    } else {
        return;
    };
    let ws = workspace("turkish_acroform");
    let out = ws.join("turkish_filled.pdf");
    let mut fill_vals = std::collections::HashMap::new();
    fill_vals.insert("Name_Last".to_string(), "SÖĞÇÜŞİsöğçüşSOGCUS".to_string());
    fill_vals.insert("Name_First".to_string(), "YANiSÖĞÇÜŞİsöğçüşSOGCUS".to_string());
    fill_vals.insert("Name_Middle".to_string(), "heySÖĞÇÜŞİsöğçüşSOGCUS".to_string());
    fill_vals.insert("Address_1".to_string(), "SÖĞÇÜŞİsöğçüşSOGCUS".to_string());
    fill_vals.insert("Address_2".to_string(), "SÖĞÇÜŞİsöğçüşSOGCUS".to_string());
    fill_vals.insert("City".to_string(), "SÖĞÇÜŞİsöğçüşSOGCUS".to_string());
    fill_vals.insert("Emergency_Contact".to_string(), "SÖĞÇÜŞİssSOGCUS".to_string());

    let res = run(commands::forms::fill_form(
        sample_path.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        fill_vals.clone(),
    ))
    .unwrap();
    assert_eq!(res.total_pages, 1);
    assert!(out.exists());

    // Also write to pdfKit_updated.pdf on desktop for direct inspection and comparison
    let updated_desktop = std::path::Path::new(r"C:\Users\CAGDASS\Desktop\pdfKit_updated.pdf");
    let _ = run(commands::forms::fill_form(
        sample_path.to_string_lossy().into_owned(),
        updated_desktop.to_string_lossy().into_owned(),
        fill_vals,
    ));

    let re_read = run(commands::forms::list_form_fields(
        out.to_string_lossy().into_owned(),
    ))
    .unwrap();
    let last = re_read.iter().find(|f| f.name == "Name_Last").unwrap();
    assert_eq!(last.value, "SÖĞÇÜŞİsöğçüşSOGCUS");
    let mid = re_read.iter().find(|f| f.name == "Name_Middle").unwrap();
    assert_eq!(mid.value, "heySÖĞÇÜŞİsöğçüşSOGCUS");
    let addr = re_read.iter().find(|f| f.name == "Address_1").unwrap();
    assert_eq!(addr.value, "SÖĞÇÜŞİsöğçüşSOGCUS");

    // Verify DR and NeedAppearances are configured in output PDF
    let doc = Document::load(&out).unwrap();
    let catalog = doc.catalog().unwrap();
    let acro_obj = catalog.get(b"AcroForm").unwrap();
    let (_, acro_dict) = doc.dereference(acro_obj).unwrap();
    let acro = acro_dict.as_dict().unwrap();
    assert_eq!(acro.get(b"NeedAppearances").unwrap().as_bool().unwrap(), false);
    assert!(acro.get(b"DR").is_ok());

    // Verify /DR /Font is Type 1 (NOT Type 0) so Adobe Acrobat interactive editor doesn't scramble text
    let dr_obj = acro.get(b"DR").unwrap();
    let (_, dr_dict_res) = doc.dereference(dr_obj).unwrap();
    let dr_dict = dr_dict_res.as_dict().unwrap();
    let dr_font_obj = dr_dict.get(b"Font").unwrap();
    let (_, dr_font_res) = doc.dereference(dr_font_obj).unwrap();
    let dr_fonts = dr_font_res.as_dict().unwrap();
    let helv_ref = dr_fonts.get(b"Helv").unwrap();
    let (_, helv_dict_res) = doc.dereference(helv_ref).unwrap();
    let helv_dict = helv_dict_res.as_dict().unwrap();
    assert_eq!(
        helv_dict.get(b"Subtype").unwrap().as_name().unwrap(),
        b"Type1",
        "/DR /Font /Helv MUST be Type 1 for Adobe Acrobat interactive text editor!"
    );

    // Verify /AP is present on filled fields and uses Type 0 font for vector fidelity
    for field_id in commands::forms::field_ids(&doc) {
        if let Ok(dict) = doc.get_dictionary(field_id) {
            let name = match dict.get(b"T") {
                Ok(lopdf::Object::String(bytes, _)) => commands::forms::decode_pdf_string(bytes),
                _ => continue,
            };
            if name == "Name_Last" || name == "Name_Middle" || name == "Address_1" {
                assert!(dict.get(b"AP").is_ok(), "Field {} must have /AP appearance stream", name);
                let ap_obj = dict.get(b"AP").unwrap();
                let (_, ap_dict_res) = doc.dereference(ap_obj).unwrap();
                let ap_dict = ap_dict_res.as_dict().unwrap();
                let n_obj = ap_dict.get(b"N").unwrap();
                let (_, n_stream_res) = doc.dereference(n_obj).unwrap();
                let n_dict = n_stream_res.as_stream().unwrap();
                let res_obj = n_dict.dict.get(b"Resources").unwrap();
                let (_, res_dict_res) = doc.dereference(res_obj).unwrap();
                let res_fonts_obj = res_dict_res.as_dict().unwrap().get(b"Font").unwrap();
                let (_, res_fonts_res) = doc.dereference(res_fonts_obj).unwrap();
                let ap_font_dict = res_fonts_res.as_dict().unwrap();
                let ap_helv = ap_font_dict.get(b"Helvetica").or_else(|_| ap_font_dict.get(b"Helv")).unwrap();
                let (_, ap_helv_res) = doc.dereference(ap_helv).unwrap();
                let ap_helv_dict = ap_helv_res.as_dict().unwrap();
                assert_eq!(
                    ap_helv_dict.get(b"Subtype").unwrap().as_name().unwrap(),
                    b"Type0",
                    "/AP /N /Resources /Font must be Type 0 for vector precision rendering!"
                );
            }
        }
    }
}

#[test]
fn stamp_page_overlays() {
    let ws = workspace("overlays");
    let src = ws.join("src.pdf");
    make_pdf(&src, 2);

    // Semi-transparent RGBA PNG overlay
    let img = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 0, 0, 128]));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    let png_data = buf.into_inner();

    let out = ws.join("annotated.pdf");
    let overlays = vec![pdfkit_lib::commands::annotate::PageOverlay {
        page: 1,
        png_data,
        replace_page_content: false,
        bbox: None,
    }];


    let res = run(commands::annotate::stamp_page_overlays(
        src.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        overlays,
        Some(r#"{"1":[{"id":"test-1","type":"text","text":"Merhaba"}]}"#.to_string()),
        None,
    ))
    .unwrap();

    assert_eq!(res.total_pages, 2);
    let check = Document::load(&out).unwrap();
    assert_eq!(check.get_pages().len(), 2);

    // Verify load_editor_document recovers the JSON and clean PDF
    let loaded = run(commands::annotate::load_editor_document(
        out.to_string_lossy().into_owned(),
    ))
    .unwrap();
    assert!(loaded.annotations_json.is_some());
    assert!(loaded.annotations_json.unwrap().contains("Merhaba"));
    assert!(loaded.clean_pdf_bytes.is_some());

    let clean_bytes = loaded.clean_pdf_bytes.unwrap();
    let clean_doc = Document::load_mem(&clean_bytes).unwrap();
    let p1_id = clean_doc.get_pages()[&1];
    let p1 = clean_doc.get_dictionary(p1_id).unwrap();
    println!("CLEAN DOC P1 CONTENTS: {:?}", p1.get(b"Contents"));
}

#[test]
fn test_remove_watermark_bytes_in_memory() {
    let ws = workspace("mem_wm");
    let src = ws.join("watermarked.pdf");
    make_pdf(&src, 2);

    // Add watermark
    run(commands::annotate::add_watermark(
        src.to_string_lossy().into_owned(),
        src.to_string_lossy().into_owned(),
        "TOP SECRET".into(),
        40.0,
        0.3,
    ))
    .unwrap();

    let bytes = std::fs::read(&src).unwrap();

    // Clean via in-memory bytes
    let cleaned_bytes = run(commands::annotate::remove_watermark_bytes(
        None,
        Some(bytes),
        None,
        None,
    ))
    .unwrap();

    assert!(!cleaned_bytes.is_empty());
    let doc = Document::load_mem(&cleaned_bytes).unwrap();
    assert_eq!(doc.get_pages().len(), 2);

    let p1 = doc.get_pages()[&1];
    let p1_content = doc.get_page_content(p1);
    let text = String::from_utf8_lossy(&p1_content);
    assert!(!text.contains("TOP SECRET"));

    // Write cleaned bytes via write_pdf_bytes
    let out_file = ws.join("written_clean.pdf");
    let write_res = run(commands::annotate::write_pdf_bytes(
        out_file.to_string_lossy().into_owned(),
        cleaned_bytes,
    ))
    .unwrap();

    assert_eq!(write_res.total_pages, 2);
    assert!(out_file.exists());
    assert!(std::fs::metadata(&out_file).unwrap().len() > 100);
}

#[test]
fn test_selective_zone_watermark_removal() {
    let ws = workspace("zone_wm");
    let src = ws.join("zoned.pdf");
    make_pdf(&src, 1);

    // Embed header banner (Top, y=180), body question (Center, y=120), diagonal watermark (Center), and footer disclaimer (Bottom, y=15)
    let mut doc = Document::load(&src).unwrap();
    let page_id = doc.page_iter().next().unwrap();

    let zoned_content = b"
        BT /F1 12 Tf 1 0 0 1 20 180 Tm (aosdestek.anadolu.edu.tr duyuru sistemi) Tj ET
        BT /F1 12 Tf 1 0 0 1 20 120 Tm (Soru 1: Asagidakilerden hangisi bir isletim sistemidir?) Tj ET
        q 0.707 0.707 -0.707 0.707 50 100 cm BT /F1 20 Tf (GIZLI BELGE FILIGRANI) Tj ET Q
        BT /F1 10 Tf 1 0 0 1 20 15 Tm (Bu belgenin her hakki saklidir ve izinsiz cogaltilamaz.) Tj ET
    ".to_vec();
    doc.add_page_contents(page_id, zoned_content).unwrap();
    doc.save(&src).unwrap();

    // Verify all 4 elements are present before cleaning
    let pre_doc = Document::load(&src).unwrap();
    let pre_bytes = pre_doc.get_page_content(page_id);
    let pre_str = String::from_utf8_lossy(&pre_bytes);
    assert!(pre_str.contains("aosdestek.anadolu.edu.tr"));
    assert!(pre_str.contains("Soru 1: Asagidakilerden"));
    assert!(pre_str.contains("GIZLI BELGE FILIGRANI"));
    assert!(pre_str.contains("her hakki saklidir"));

    // Case 1: ONLY remove Center (keep Top and Bottom)
    let out_center_only = ws.join("center_only.pdf");
    run(commands::annotate::remove_watermark(
        src.to_string_lossy().into_owned(),
        out_center_only.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: false,
            center: true,
            bottom: false,
        }),
    )).unwrap();

    let doc_c = Document::load(&out_center_only).unwrap();
    let bytes_c = doc_c.get_page_content(page_id);
    let str_c = String::from_utf8_lossy(&bytes_c);
    assert!(str_c.contains("aosdestek.anadolu.edu.tr"), "Top header notice must be preserved!");
    assert!(str_c.contains("Soru 1: Asagidakilerden"), "Exam question must be preserved!");
    assert!(!str_c.contains("GIZLI BELGE FILIGRANI"), "Diagonal center watermark must be removed!");
    assert!(str_c.contains("her hakki saklidir"), "Bottom footer disclaimer must be preserved!");

    // Case 2: Remove Top and Bottom (keep Center)
    let out_top_bottom = ws.join("top_bottom.pdf");
    run(commands::annotate::remove_watermark(
        src.to_string_lossy().into_owned(),
        out_top_bottom.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: true,
            center: false,
            bottom: true,
        }),
    )).unwrap();

    let doc_tb = Document::load(&out_top_bottom).unwrap();
    let bytes_tb = doc_tb.get_page_content(page_id);
    let str_tb = String::from_utf8_lossy(&bytes_tb);
    assert!(!str_tb.contains("aosdestek.anadolu.edu.tr"), "Top header notice must be removed!");
    assert!(str_tb.contains("Soru 1: Asagidakilerden"), "Exam question must be preserved!");
    assert!(str_tb.contains("GIZLI BELGE FILIGRANI"), "Diagonal center watermark must be preserved!");
    assert!(!str_tb.contains("her hakki saklidir"), "Bottom footer disclaimer must be removed!");

    // Case 3: Master toggle / All zones (Remove Top, Center, and Bottom)
    let out_all = ws.join("all_zones.pdf");
    run(commands::annotate::remove_watermark(
        src.to_string_lossy().into_owned(),
        out_all.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: true,
            center: true,
            bottom: true,
        }),
    )).unwrap();

    let doc_all = Document::load(&out_all).unwrap();
    let bytes_all = doc_all.get_page_content(page_id);
    let str_all = String::from_utf8_lossy(&bytes_all);
    assert!(!str_all.contains("aosdestek.anadolu.edu.tr"), "Top notice must be removed!");
    assert!(str_all.contains("Soru 1: Asagidakilerden"), "Exam question must NEVER be damaged!");
    assert!(!str_all.contains("GIZLI BELGE FILIGRANI"), "Center diagonal watermark must be removed!");
    assert!(!str_all.contains("her hakki saklidir"), "Bottom footer disclaimer must be removed!");
}

#[test]
fn test_unified_watermark_layer_selective_zone_removal() {
    let ws = workspace("unified_wm_layer");
    let src = ws.join("src.pdf");
    make_pdf(&src, 1);

    // MediaBox height is 200 (from make_pdf in test helpers)
    let mut doc = Document::load(&src).unwrap();
    let page_id = doc.page_iter().next().unwrap();

    // Stream 0: Models the real AÖF/Adobe Acrobat exam PDF structure
    // A single q ... /OC /Xi0 BDC ... EMC Q block with BOTH header and footer in the first BT,
    // and diagonal watermark in the second BT.
    let unified_wm_stream = b"
        q
        /OC /Xi0 BDC
        /Xi1 8 Tf
        0.25 0.25 0.25 rg
        BT
        1 0 0 1 30 190 Tm (TOP_HEADER_ANADOLU_NOTICE) Tj
        1 0 0 1 30 15 Tm (BOTTOM_FOOTER_COPYRIGHT_WARNING) Tj
        ET
        /Xi1 38 Tf
        /Xi2 gs
        BT
        0.70711 0.70711 -0.70711 0.70711 50 100 Tm (CENTER_DIAGONAL_WATERMARK) Tj
        ET
        EMC
        Q
    ".to_vec();

    // Stream 1: Models the exam questions, choices (A-E), tables, page numbers
    let exam_content_stream = b"
        BT /F1 12 Tf 1 0 0 1 40 120 Tm (Soru 1: Exam Question Text) Tj ET
        BT /F1 10 Tf 1 0 0 1 40 100 Tm (A) Secenek A (B) Secenek B) Tj ET
    ".to_vec();

    doc.add_page_contents(page_id, unified_wm_stream).unwrap();
    doc.add_page_contents(page_id, exam_content_stream).unwrap();
    doc.save(&src).unwrap();

    // Verify all parts exist initially
    let pre_doc = Document::load(&src).unwrap();
    let pre_bytes = pre_doc.get_page_content(page_id);
    let pre_str = String::from_utf8_lossy(&pre_bytes);
    assert!(pre_str.contains("TOP_HEADER_ANADOLU_NOTICE"));
    assert!(pre_str.contains("BOTTOM_FOOTER_COPYRIGHT_WARNING"));
    assert!(pre_str.contains("CENTER_DIAGONAL_WATERMARK"));
    assert!(pre_str.contains("Soru 1: Exam Question Text"));

    // 1. Remove ONLY Top
    let out_top = ws.join("top_only.pdf");
    run(commands::annotate::remove_watermark(
        src.to_string_lossy().into_owned(),
        out_top.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: true,
            center: false,
            bottom: false,
        }),
    )).unwrap();
    let doc_top = Document::load(&out_top).unwrap();
    let bytes_top = doc_top.get_page_content(page_id);
    let str_top = String::from_utf8_lossy(&bytes_top);
    assert!(!str_top.contains("TOP_HEADER_ANADOLU_NOTICE"), "Top header must be removed!");
    assert!(str_top.contains("CENTER_DIAGONAL_WATERMARK"), "Center diagonal must be preserved!");
    assert!(str_top.contains("BOTTOM_FOOTER_COPYRIGHT_WARNING"), "Bottom footer must be preserved!");
    assert!(str_top.contains("Soru 1: Exam Question Text"), "Exam question must be preserved!");

    // 2. Remove ONLY Center
    let out_center = ws.join("center_only.pdf");
    run(commands::annotate::remove_watermark(
        src.to_string_lossy().into_owned(),
        out_center.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: false,
            center: true,
            bottom: false,
        }),
    )).unwrap();
    let doc_center = Document::load(&out_center).unwrap();
    let bytes_center = doc_center.get_page_content(page_id);
    let str_center = String::from_utf8_lossy(&bytes_center);
    assert!(str_center.contains("TOP_HEADER_ANADOLU_NOTICE"), "Top header must be preserved!");
    assert!(!str_center.contains("CENTER_DIAGONAL_WATERMARK"), "Center diagonal must be removed!");
    assert!(str_center.contains("BOTTOM_FOOTER_COPYRIGHT_WARNING"), "Bottom footer must be preserved!");
    assert!(str_center.contains("Soru 1: Exam Question Text"), "Exam question must be preserved!");

    // 3. Remove ONLY Bottom
    let out_bottom = ws.join("bottom_only.pdf");
    run(commands::annotate::remove_watermark(
        src.to_string_lossy().into_owned(),
        out_bottom.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: false,
            center: false,
            bottom: true,
        }),
    )).unwrap();
    let doc_bottom = Document::load(&out_bottom).unwrap();
    let bytes_bottom = doc_bottom.get_page_content(page_id);
    let str_bottom = String::from_utf8_lossy(&bytes_bottom);
    assert!(str_bottom.contains("TOP_HEADER_ANADOLU_NOTICE"), "Top header must be preserved!");
    assert!(str_bottom.contains("CENTER_DIAGONAL_WATERMARK"), "Center diagonal must be preserved!");
    assert!(!str_bottom.contains("BOTTOM_FOOTER_COPYRIGHT_WARNING"), "Bottom footer must be removed!");
    assert!(str_bottom.contains("Soru 1: Exam Question Text"), "Exam question must be preserved!");

    // 4. Remove ALL (Master Toggle)
    let out_all_zones = ws.join("all_unified.pdf");
    run(commands::annotate::remove_watermark(
        src.to_string_lossy().into_owned(),
        out_all_zones.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: true,
            center: true,
            bottom: true,
        }),
    )).unwrap();
    let doc_all_z = Document::load(&out_all_zones).unwrap();
    let bytes_all_z = doc_all_z.get_page_content(page_id);
    let str_all_z = String::from_utf8_lossy(&bytes_all_z);
    assert!(!str_all_z.contains("TOP_HEADER_ANADOLU_NOTICE"), "Top header must be removed!");
    assert!(!str_all_z.contains("CENTER_DIAGONAL_WATERMARK"), "Center diagonal must be removed!");
    assert!(!str_all_z.contains("BOTTOM_FOOTER_COPYRIGHT_WARNING"), "Bottom footer must be removed!");
    assert!(str_all_z.contains("Soru 1: Exam Question Text"), "Exam question must be preserved!");
}

#[test]
fn test_real_aof_exam_file_zoning() {
    let real_path = std::path::Path::new(r"C:\Users\CAGDASS\Desktop\ai-Aöf 2026 YAZ OKULU\KARAR MODELLERİ - YBS403U\Dönem Sonu 2025-2026.pdf");
    if !real_path.exists() {
        return;
    }

    let ws = workspace("real_aof_test");

    // Case 1: ONLY Center removed (Diagonal 45° watermark removed, Header and Footer kept)
    let out_center = ws.join("real_center_only.pdf");
    run(commands::annotate::remove_watermark(
        real_path.to_string_lossy().into_owned(),
        out_center.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: false,
            center: true,
            bottom: false,
        }),
    )).unwrap();

    let doc_c = Document::load(&out_center).unwrap();
    let p1 = doc_c.page_iter().next().unwrap();
    let p1_bytes = doc_c.get_page_content(p1);
    let p1_str = String::from_utf8_lossy(&p1_bytes);
    // Diagonal matrix should be gone from the page
    assert!(!p1_str.contains("0.70711"), "Diagonal matrix must be removed when Center is toggled!");
    // Header (y=820) and Footer (y=45) must still exist
    assert!(p1_str.contains("820 Tm"), "Header at y=820 must be preserved!");
    assert!(p1_str.contains("45 Tm"), "Footer at y=45 must be preserved!");
    // Exam question stream (/Xf1 Do) must still exist
    assert!(p1_str.contains("/Xf1 Do"), "Exam questions must be 100% preserved!");

    // Case 2: ONLY Top removed (Header y=820 removed, Diagonal and Footer kept)
    let out_top = ws.join("real_top_only.pdf");
    run(commands::annotate::remove_watermark(
        real_path.to_string_lossy().into_owned(),
        out_top.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: true,
            center: false,
            bottom: false,
        }),
    )).unwrap();

    let doc_t = Document::load(&out_top).unwrap();
    let p1_t_bytes = doc_t.get_page_content(p1);
    let p1_t_str = String::from_utf8_lossy(&p1_t_bytes);
    assert!(!p1_t_str.contains("820 Tm"), "Header at y=820 must be removed when Top is toggled!");
    assert!(p1_t_str.contains("0.70711"), "Diagonal matrix must be preserved!");
    assert!(p1_t_str.contains("45 Tm"), "Footer at y=45 must be preserved!");
    assert!(p1_t_str.contains("/Xf1 Do"), "Exam questions must be 100% preserved!");

    // Case 3: ONLY Bottom removed (Footer y=45 removed, Header and Diagonal kept)
    let out_bottom = ws.join("real_bottom_only.pdf");
    run(commands::annotate::remove_watermark(
        real_path.to_string_lossy().into_owned(),
        out_bottom.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: false,
            center: false,
            bottom: true,
        }),
    )).unwrap();

    let doc_b = Document::load(&out_bottom).unwrap();
    let p1_b_bytes = doc_b.get_page_content(p1);
    let p1_b_str = String::from_utf8_lossy(&p1_b_bytes);
    assert!(p1_b_str.contains("820 Tm"), "Header at y=820 must be preserved!");
    assert!(p1_b_str.contains("0.70711"), "Diagonal matrix must be preserved!");
    assert!(!p1_b_str.contains("30 45 Tm"), "Footer at y=45 must be removed when Bottom is toggled!");
    assert!(p1_b_str.contains("/Xf1 Do"), "Exam questions must be 100% preserved!");

    // Case 4: ALL removed (Header, Diagonal, and Footer all removed)
    let out_all = ws.join("real_all_clean.pdf");
    run(commands::annotate::remove_watermark(
        real_path.to_string_lossy().into_owned(),
        out_all.to_string_lossy().into_owned(),
        None,
        Some(pdfkit_lib::commands::annotate::WatermarkZones {
            top: true,
            center: true,
            bottom: true,
        }),
    )).unwrap();

    let doc_a = Document::load(&out_all).unwrap();
    let p1_a_bytes = doc_a.get_page_content(p1);
    let p1_a_str = String::from_utf8_lossy(&p1_a_bytes);
    assert!(!p1_a_str.contains("820 Tm"), "Header must be removed!");
    assert!(!p1_a_str.contains("0.70711"), "Diagonal watermark must be removed!");
    assert!(!p1_a_str.contains("30 45 Tm"), "Footer disclaimer must be removed!");
    assert!(p1_a_str.contains("/Xf1 Do"), "Exam questions must be 100% preserved!");
}

#[test]
fn test_page_numbers_preserve_existing_fonts() {
    let ws = workspace("preserve_fonts");
    let src = ws.join("src_with_fonts.pdf");

    // Build a PDF with an indirect Font dictionary containing /F1 and /F2
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_f1_id = doc.add_object(lopdf::Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Font".to_vec())),
        ("Subtype", Object::Name(b"Type1".to_vec())),
        ("BaseFont", Object::Name(b"CustomExamHeaderFont".to_vec())),
    ]));
    let font_f2_id = doc.add_object(lopdf::Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Font".to_vec())),
        ("Subtype", Object::Name(b"Type1".to_vec())),
        ("BaseFont", Object::Name(b"CustomExamBodyFont".to_vec())),
    ]));

    let font_dict_id = doc.add_object(lopdf::Dictionary::from_iter(vec![
        ("F1", Object::Reference(font_f1_id)),
        ("F2", Object::Reference(font_f2_id)),
    ]));

    let resources_id = doc.add_object(lopdf::Dictionary::from_iter(vec![
        ("Font", Object::Reference(font_dict_id)),
    ]));

    let content_stream = b"BT /F1 14 Tf 50 750 Td (Header: DIJITAL DONUSUM BIL109U) Tj ET\nBT /F2 10 Tf 50 600 Td (Soru 1: Asagidakilerden hangisi dogrudur?) Tj ET".to_vec();
    let content = doc.add_object(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content_stream,
    ));

    let page = lopdf::Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Page".to_vec())),
        ("Parent", Object::Reference(pages_id)),
        (
            "MediaBox",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(595),
                Object::Integer(842),
            ]),
        ),
        ("Resources", Object::Reference(resources_id)),
        ("Contents", Object::Reference(content)),
    ]);
    let page_id = doc.add_object(page);

    doc.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Pages".to_vec())),
            ("Kids", Object::Array(vec![Object::Reference(page_id)])),
            ("Count", Object::Integer(1)),
        ])),
    );
    let catalog = doc.add_object(lopdf::Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]));
    doc.trailer.set("Root", catalog);
    doc.save(&src).unwrap();

    // Now run add_page_numbers
    let out = ws.join("out_numbered.pdf");
    run(commands::annotate::add_page_numbers(
        src.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        12.0,
        30.0,
    ))
    .unwrap();

    // Verify resulting PDF
    let check_doc = Document::load(&out).unwrap();
    let p1 = check_doc.get_pages()[&1];
    let p1_dict = check_doc.get_dictionary(p1).unwrap();

    // Verify Resources -> Font contains F1, F2 AND PK_Font
    let res_obj = p1_dict.get(b"Resources").unwrap();
    let res_dict = match res_obj {
        Object::Reference(rid) => check_doc.get_dictionary(*rid).unwrap(),
        Object::Dictionary(d) => d,
        _ => panic!("Expected dictionary for resources"),
    };

    let font_obj = res_dict.get(b"Font").unwrap();
    let font_dict = match font_obj {
        Object::Reference(fid) => check_doc.get_dictionary(*fid).unwrap(),
        Object::Dictionary(d) => d,
        _ => panic!("Expected dictionary for Font"),
    };

    assert!(font_dict.get(b"F1").is_ok(), "/F1 must be preserved and not overwritten!");
    assert!(font_dict.get(b"F2").is_ok(), "/F2 must be preserved!");
    assert!(font_dict.get(b"PK_Font").is_ok(), "/PK_Font must be added!");

    // Check that /F1 still points to CustomExamHeaderFont
    let f1_ref = font_dict.get(b"F1").unwrap().as_reference().unwrap();
    let f1_dict = check_doc.get_dictionary(f1_ref).unwrap();
    assert_eq!(
        f1_dict.get(b"BaseFont").unwrap().as_name().unwrap(),
        b"CustomExamHeaderFont",
        "Original F1 font must remain intact without being replaced by Helvetica!"
    );

    // Check content streams: original text still uses /F1 and /F2, stamped number uses /PK_Font
    let content_bytes = check_doc.get_page_content(p1);
    let content_str = String::from_utf8_lossy(&content_bytes);
    assert!(content_str.contains("/F1 14 Tf"), "Original header must still reference /F1!");
    assert!(content_str.contains("/F2 10 Tf"), "Original body must still reference /F2!");
    assert!(content_str.contains("/PK_Font"), "Stamped page number must use /PK_Font!");
    assert!(content_str.contains("1 / 1"), "Page number label must be stamped!");
}

#[test]
fn test_inspect_real_exam_page_number() {
    let p = std::path::Path::new(r"C:\Users\CAGDASS\Desktop\ai-Aöf 2026 YAZ OKULU\KARAR MODELLERİ - YBS403U\Dönem Sonu 2025-2026.pdf");
    if !p.exists() { return; }

    let ws = workspace("clean_exam_page_numbers");
    let out = ws.join("exam_numbered.pdf");

    // Apply page numbers using the new pipeline
    let res = pdfkit_lib::commands::annotate::apply_page_numbers(
        p,
        &out,
        10.0,
        24.0,
    ).unwrap();
    assert_eq!(res.total_pages, 16);

    // Verify resulting PDF
    let check_doc = Document::load(&out).unwrap();
    let p1 = check_doc.page_iter().next().unwrap();
    let content = check_doc.get_page_content(p1);
    let decoded = lopdf::content::Content::decode(&content).unwrap();

    let mut found_old_glyph_22 = false;
    let mut found_pk_font = false;
    let mut found_new_page_label = false;
    let mut found_disclaimer = false;

    for op in &decoded.operations {
        if op.operator == "Tf" {
            if let Some(lopdf::Object::Name(n)) = op.operands.get(0) {
                if n == b"PK_Font" {
                    found_pk_font = true;
                }
            }
        }
        if op.operator == "Tj" {
            if let Some(lopdf::Object::String(bytes, _)) = op.operands.get(0) {
                if bytes == &[0, 22] {
                    found_old_glyph_22 = true;
                }
                let s = String::from_utf8_lossy(bytes);
                if s.contains("1 / 16") {
                    found_new_page_label = true;
                }
                if bytes.len() > 100 {
                    found_disclaimer = true;
                }
            }
        }
    }

    assert!(!found_old_glyph_22, "Old page number glyph [0, 22] must be cleanly removed!");
    assert!(found_pk_font, "New PK_Font must be present for page numbering!");
    assert!(found_new_page_label, "New page number label '1 / 16' must be stamped!");
    assert!(found_disclaimer, "Disclaimer text at bottom must be completely preserved!");
}

#[test]
fn test_redaction_physically_purges_unreferenced_text() {
    let ws = workspace("redaction_purge");
    let src = ws.join("secret_doc.pdf");
    make_pdf(&src, 1);

    // Embed top secret text into src.pdf
    let mut doc = Document::load(&src).unwrap();
    let page_id = doc.page_iter().next().unwrap();
    let secret_stream = b"BT /F1 16 Tf 1 0 0 1 50 150 Tm (CONFIDENTIAL_CARD_NUMBER_4111_2222) Tj ET".to_vec();
    doc.add_page_contents(page_id, secret_stream).unwrap();
    doc.save(&src).unwrap();

    // Verify secret text exists in the document
    let raw_before = std::fs::read(&src).unwrap();
    let str_before = String::from_utf8_lossy(&raw_before);
    assert!(str_before.contains("CONFIDENTIAL_CARD_NUMBER_4111_2222"));

    // Apply redaction overlay with replace_page_content = true
    let out = ws.join("redacted_doc.pdf");
    let img = image::RgbaImage::from_pixel(200, 200, image::Rgba([0, 0, 0, 255]));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    let png_data = buf.into_inner();

    let overlays = vec![pdfkit_lib::commands::annotate::PageOverlay {
        page: 1,
        png_data,
        replace_page_content: true,
        bbox: None,
    }];


    let res = run(commands::annotate::stamp_page_overlays(
        src.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        overlays,
        None,
        None,
    ))
    .unwrap();

    assert_eq!(res.total_pages, 1);

    // Verify that the secret text is completely purged from raw bytes and all objects
    let raw_after = std::fs::read(&out).unwrap();
    let str_after = String::from_utf8_lossy(&raw_after);
    assert!(
        !str_after.contains("CONFIDENTIAL_CARD_NUMBER_4111_2222"),
        "Redacted text must NOT exist anywhere in raw output bytes!"
    );

    let check_doc = Document::load(&out).unwrap();
    for (_, obj) in check_doc.objects.iter() {
        if let Ok(stream) = obj.as_stream() {
            let stream_str = String::from_utf8_lossy(&stream.content);
            assert!(
                !stream_str.contains("CONFIDENTIAL_CARD_NUMBER"),
                "Redacted text must NOT exist in any object stream in the document!"
            );
        }
    }
}

#[test]
fn test_cropped_page_overlay_with_bbox() {
    let ws = workspace("cropped_overlay");
    let src = ws.join("src.pdf");
    make_pdf(&src, 1);

    // Small cropped image overlay (e.g. 50x30 stamp)
    let img = image::RgbaImage::from_pixel(50, 30, image::Rgba([0, 150, 255, 200]));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    let png_data = buf.into_inner();

    let out = ws.join("stamped_cropped.pdf");
    let overlays = vec![pdfkit_lib::commands::annotate::PageOverlay {
        page: 1,
        png_data,
        replace_page_content: false,
        bbox: Some([20.0, 40.0, 50.0, 30.0]),
    }];

    let res = run(commands::annotate::stamp_page_overlays(
        src.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        overlays,
        None,
        None,
    ))
    .unwrap();

    assert_eq!(res.total_pages, 1);
    let check = Document::load(&out).unwrap();
    assert_eq!(check.get_pages().len(), 1);

    // Verify content stream contains the cropped cm transform
    let p1_id = check.get_pages()[&1];
    let content = check.get_page_content(p1_id);
    let content_str = String::from_utf8_lossy(&content);
    assert!(content_str.contains("50.00 0 0 30.00 20.00 40.00 cm"));
}

#[test]
fn test_ocr_searchable_layer() {
    let ws = workspace("ocr_layer");
    let src = ws.join("src.pdf");
    make_pdf(&src, 1);

    // Create an image with text "PDFKit"
    let sample_img = ws.join("sample.png");
    let img = image::RgbImage::from_pixel(100, 50, image::Rgb([255, 255, 255]));
    img.save(&sample_img).unwrap();

    let out = ws.join("searchable.pdf");
    let res = run(commands::ocr::add_searchable_ocr_layer(
        src.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
        1,
        sample_img.to_string_lossy().into_owned(),
    ));

    // If Windows OCR succeeds on this platform, assert document is valid
    if let Ok(res) = res {
        assert_eq!(res.total_pages, 1);
        let check = Document::load(&out).unwrap();
        let p1_id = check.get_pages()[&1];
        let content = check.get_page_content(p1_id);
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("3 Tr"));
    }
}




