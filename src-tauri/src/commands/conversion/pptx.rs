//! PPTX Conversion Engine Module
//! Handles PowerPoint presentation reconstruction and OpenXML packaging.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

use crate::error::PdfError;
use super::{escape_xml, run_python_engine, PageTextData};

static BLANK_PPTX: &[u8] = include_bytes!("../../../templates/blank.pptx");

pub fn convert(
    app: &tauri::AppHandle,
    input_path: Option<&str>,
    output_path: &Path,
    pages: &[PageTextData],
    language: &str,
) -> Result<(), PdfError> {
    let out_str = output_path.to_string_lossy();
    if let Some(in_path) = input_path {
        if Path::new(in_path).exists() {
            match run_python_engine(app, in_path, &out_str, "pptx") {
                Ok(()) => {
                    eprintln!("[PPTX] Yüksek kaliteli sunum inşası başarıyla tamamlandı.");
                    return Ok(());
                }
                Err(err) => {
                    eprintln!("[PPTX] Gelişmiş motor hatası: {}, yerel OpenXML deneniyor...", err);
                    if err.contains("iptal edildi") || err.contains("Dosya başka bir programda açık") {
                        return Err(PdfError::Custom(err));
                    }
                    if pages.is_empty() {
                        return Err(PdfError::Custom(format!("PowerPoint dönüştürme hatası: {}", err)));
                    }
                }
            }
        }
    }
    generate_pptx(output_path, pages, language)
}

/// Builds a 100% valid OpenXML PPTX archive using the validated base presentation package
pub fn generate_pptx(
    output_path: &Path,
    pages: &[PageTextData],
    language: &str,
) -> Result<(), PdfError> {
    use std::io::{Cursor, Read};
    use zip::ZipArchive;

    let lang_tag = if language == "en" { "en-US" } else { "tr-TR" };
    let slide_count = if pages.is_empty() { 1 } else { pages.len() };

    let template_cursor = Cursor::new(BLANK_PPTX);
    let mut template_archive = ZipArchive::new(template_cursor)
        .map_err(|e| PdfError::Custom(format!("PPTX template error: {}", e)))?;

    let out_file = File::create(output_path)?;
    let mut zip = ZipWriter::new(out_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut content_types = String::new();
    let mut pres_rels = String::new();
    let mut presentation_xml = String::new();

    for i in 0..template_archive.len() {
        let mut file = template_archive.by_index(i)
            .map_err(|e| PdfError::Custom(format!("PPTX template read error: {}", e)))?;
        let name = file.name().to_string();

        if name == "[Content_Types].xml" {
            file.read_to_string(&mut content_types)?;
        } else if name == "ppt/_rels/presentation.xml.rels" {
            file.read_to_string(&mut pres_rels)?;
        } else if name == "ppt/presentation.xml" {
            file.read_to_string(&mut presentation_xml)?;
        } else {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            zip.start_file(&name, options)?;
            zip.write_all(&buf)?;
        }
    }

    // 1. Modify [Content_Types].xml to register slides
    let mut overrides = String::new();
    for i in 1..=slide_count {
        overrides.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
    }
    content_types = content_types.replace("</Types>", &format!("{overrides}</Types>"));
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(content_types.as_bytes())?;

    // 2. Modify ppt/_rels/presentation.xml.rels to link slides
    let mut max_rid = 7;
    for cap in pres_rels.split("rId").skip(1) {
        let num_str: String = cap.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(num) = num_str.parse::<u32>() {
            if num > max_rid {
                max_rid = num;
            }
        }
    }

    let mut slide_rids = Vec::new();
    let mut slide_rel_tags = String::new();
    for i in 1..=slide_count {
        let rid_val = max_rid + i as u32;
        let rid_str = format!("rId{}", rid_val);
        slide_rel_tags.push_str(&format!(
            r#"<Relationship Id="{rid_str}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>"#
        ));
        slide_rids.push(rid_str);
    }
    pres_rels = pres_rels.replace("</Relationships>", &format!("{slide_rel_tags}</Relationships>"));
    zip.start_file("ppt/_rels/presentation.xml.rels", options)?;
    zip.write_all(pres_rels.as_bytes())?;

    // 3. Modify ppt/presentation.xml to add <p:sldIdLst>
    let mut sld_id_items = String::new();
    for (idx, rid_str) in slide_rids.iter().enumerate() {
        sld_id_items.push_str(&format!(
            r#"<p:sldId id="{}" r:id="{}"/>"#,
            256 + idx + 1,
            rid_str
        ));
    }
    let sld_id_lst = format!("<p:sldIdLst>{}</p:sldIdLst>", sld_id_items);
    if presentation_xml.contains("<p:sldIdLst>") {
        if let Some(start) = presentation_xml.find("<p:sldIdLst>") {
            if let Some(end) = presentation_xml[start..].find("</p:sldIdLst>") {
                presentation_xml.replace_range(start..start + end + 13, &sld_id_lst);
            }
        }
    } else {
        presentation_xml = presentation_xml.replace("</p:sldMasterIdLst>", &format!("</p:sldMasterIdLst>{}", sld_id_lst));
    }
    zip.start_file("ppt/presentation.xml", options)?;
    zip.write_all(presentation_xml.as_bytes())?;

    // 4. Create slides and slide relationships
    let slide_layout_rel = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout7.xml"/>
</Relationships>"#;

    let default_page = PageTextData {
        page: 1,
        text: String::new(),
    };
    let pages_slice = if pages.is_empty() {
        std::slice::from_ref(&default_page)
    } else {
        pages
    };

    for (idx, p) in pages_slice.iter().enumerate() {
        let slide_num = idx + 1;
        let rel_path = format!("ppt/slides/_rels/slide{}.xml.rels", slide_num);
        zip.start_file(&rel_path, options)?;
        zip.write_all(slide_layout_rel.as_bytes())?;

        let mut body_paragraphs = String::new();
        let trimmed = p.text.trim();
        if trimmed.is_empty() {
            body_paragraphs.push_str(&format!(
                r#"<a:p><a:r><a:rPr lang="{lang_tag}" sz="1400" i="1"><a:solidFill><a:srgbClr val="6B7280"/></a:solidFill></a:rPr><a:t>(Bu sayfa görsel içeriklidir veya metin katmanı içermemektedir)</a:t></a:r></a:p>"#
            ));
        } else {
            for line in p.text.lines() {
                let trimmed_l = line.trim();
                if trimmed_l.is_empty() {
                    body_paragraphs.push_str("<a:p/>");
                } else {
                    body_paragraphs.push_str(&format!(
                        r#"<a:p><a:r><a:rPr lang="{lang_tag}" sz="1400"><a:solidFill><a:srgbClr val="1F2937"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p>"#,
                        escape_xml(trimmed_l)
                    ));
                }
            }
        }

        let slide_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="457200" y="457200"/><a:ext cx="8229600" cy="914400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="none"><a:spAutoFit/></a:bodyPr><a:lstStyle/><a:p><a:r><a:rPr lang="{lang_tag}" sz="2800" b="1"><a:solidFill><a:srgbClr val="1E40AF"/></a:solidFill></a:rPr><a:t>Sayfa {}</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="457200" y="1600000"/><a:ext cx="8229600" cy="4800000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="square"><a:spAutoFit/></a:bodyPr><a:lstStyle/>{}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
            p.page,
            body_paragraphs
        );

        let slide_path = format!("ppt/slides/slide{}.xml", slide_num);
        zip.start_file(&slide_path, options)?;
        zip.write_all(slide_xml.as_bytes())?;
    }

    zip.finish().map_err(|e| PdfError::Custom(format!("ZIP finish error: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    fn test_pptx_export_generation() {
        let temp_dir = std::env::temp_dir();
        let pptx_path = temp_dir.join("test_pptx_module.pptx");
        let pages = vec![
            PageTextData {
                page: 1,
                text: "Sunum Sayfa 1\nKahve Çekirdeği 150 TL".to_string(),
            },
            PageTextData {
                page: 2,
                text: "Sunum Sayfa 2\nTürkçe: ç, ğ, ı, ö, ş, ü.".to_string(),
            },
        ];

        let result = generate_pptx(&pptx_path, &pages, "tr");
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
        assert!(slide1_xml.contains("Kahve"));

        let _ = std::fs::remove_file(pptx_path);
    }
}
