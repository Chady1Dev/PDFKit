use std::fs::File;
use std::io::Write;
use std::path::Path;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

use crate::error::PdfError;

#[derive(Debug, serde::Deserialize)]
pub struct PageTextData {
    pub page: u32,
    pub text: String,
}

/// Helper function to escape XML special characters and strip illegal XML 1.0 control codes.
fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        let code = c as u32;
        // Valid XML 1.0 characters: 0x9 (tab), 0xA (LF), 0xD (CR), 0x20..=0xD7FF, 0xE000..=0xFFFD, 0x10000..=0x10FFFF
        if code != 0x9 && code != 0xA && code != 0xD
            && !(0x20..=0xD7FF).contains(&code)
            && !(0xE000..=0xFFFD).contains(&code)
            && !(0x10000..=0x10FFFF).contains(&code)
        {
            continue;
        }
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Convert zero-indexed column number to Excel column letter (0 -> A, 1 -> B, 26 -> AA, etc.)
fn col_letter(mut col: usize) -> String {
    let mut result = String::new();
    loop {
        let rem = (col % 26) as u8;
        result.insert(0, (b'A' + rem) as char);
        if col < 26 {
            break;
        }
        col = (col / 26) - 1;
    }
    result
}

/// Builds a 100% valid OpenXML DOCX archive
fn generate_docx(
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

/// Builds a 100% valid OpenXML XLSX archive
fn generate_xlsx(
    output_path: &Path,
    pages: &[PageTextData],
) -> Result<(), PdfError> {
    let file = File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // 1. [Content_Types].xml
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
</Types>"#;
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(content_types.as_bytes())?;

    // 2. _rels/.rels
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#;
    zip.start_file("_rels/.rels", options)?;
    zip.write_all(root_rels.as_bytes())?;

    // 3. xl/_rels/workbook.xml.rels
    let wb_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#;
    zip.start_file("xl/_rels/workbook.xml.rels", options)?;
    zip.write_all(wb_rels.as_bytes())?;

    // 4. xl/styles.xml - 100% compliant OpenXML with cellStyles
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="3">
    <font><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/></font>
    <font><b/><sz val="11"/><color rgb="FFFFFFFF"/><name val="Calibri"/><family val="2"/></font>
    <font><b/><sz val="12"/><color rgb="FF1E40AF"/><name val="Calibri"/><family val="2"/></font>
  </fonts>
  <fills count="4">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="FF2563EB"/><bgColor indexed="64"/></patternFill></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="FFF1F5F9"/><bgColor indexed="64"/></patternFill></fill>
  </fills>
  <borders count="2">
    <border><left/><right/><top/><bottom/><diagonal/></border>
    <border>
      <left style="thin"><color rgb="FFE2E8F0"/></left>
      <right style="thin"><color rgb="FFE2E8F0"/></right>
      <top style="thin"><color rgb="FFE2E8F0"/></top>
      <bottom style="thin"><color rgb="FFE2E8F0"/></bottom>
      <diagonal/>
    </border>
  </borders>
  <cellStyleXfs count="1">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
  </cellStyleXfs>
  <cellXfs count="4">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
    <xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1" applyAlignment="1">
      <alignment horizontal="center" vertical="center"/>
    </xf>
    <xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0" applyBorder="1"/>
    <xf numFmtId="0" fontId="2" fillId="3" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1"/>
  </cellXfs>
  <cellStyles count="1">
    <cellStyle name="Normal" xfId="0" builtinId="0"/>
  </cellStyles>
</styleSheet>"#;
    zip.start_file("xl/styles.xml", options)?;
    zip.write_all(styles.as_bytes())?;

    // 5. xl/workbook.xml
    let workbook = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="PDF Verileri" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#;
    zip.start_file("xl/workbook.xml", options)?;
    zip.write_all(workbook.as_bytes())?;

    // 6. xl/worksheets/sheet1.xml
    let mut sheet_data = String::new();
    let mut max_col_used = 1;
    let mut row_idx: usize = 1;

    for (p_idx, p) in pages.iter().enumerate() {
        if pages.len() > 1 {
            sheet_data.push_str(&format!(
                r#"<row r="{row}" ht="22" customHeight="1">
  <c r="A{row}" s="3" t="inlineStr"><is><t>--- Sayfa {} ---</t></is></c>
</row>"#,
                p.page,
                row = row_idx
            ));
            row_idx += 1;
        }

        let lines: Vec<&str> = p.text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        for line in lines {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() > max_col_used {
                max_col_used = cols.len();
            }

            sheet_data.push_str(&format!(r#"<row r="{row}" ht="19" customHeight="1">"#, row = row_idx));
            for (c_idx, col_val) in cols.iter().enumerate() {
                let letter = col_letter(c_idx);
                let trimmed_val = col_val.trim();
                sheet_data.push_str(&format!(
                    r#"<c r="{letter}{row}" s="2" t="inlineStr"><is><t xml:space="preserve">{val}</t></is></c>"#,
                    letter = letter,
                    row = row_idx,
                    val = escape_xml(trimmed_val)
                ));
            }
            sheet_data.push_str("</row>");
            row_idx += 1;
        }

        if p_idx < pages.len() - 1 {
            row_idx += 1;
        }
    }

    if row_idx == 1 {
        sheet_data.push_str(
            r#"<row r="1" ht="20" customHeight="1"><c r="A1" s="2" t="inlineStr"><is><t>(Belgede metin bulunamadı veya salt görsel içeriklidir)</t></is></c></row>"#,
        );
        row_idx = 2;
    }

    let end_col = col_letter(max_col_used.saturating_sub(1));
    let end_row = row_idx.saturating_sub(1).max(1);

    let mut col_tags = String::new();
    let num_cols = max_col_used.max(4);
    for c in 1..=num_cols {
        col_tags.push_str(&format!(r#"<col min="{c}" max="{c}" width="22" customWidth="1"/>"#));
    }

    let worksheet = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:{end_col}{end_row}"/>
  <sheetViews>
    <sheetView tabSelected="1" workbookViewId="0"/>
  </sheetViews>
  <sheetFormatPr defaultRowHeight="16"/>
  <cols>
    {col_tags}
  </cols>
  <sheetData>
    {sheet_data}
  </sheetData>
</worksheet>"#,
        end_col = end_col,
        end_row = end_row,
        col_tags = col_tags,
        sheet_data = sheet_data
    );

    zip.start_file("xl/worksheets/sheet1.xml", options)?;
    zip.write_all(worksheet.as_bytes())?;

    zip.finish().map_err(|e| PdfError::Custom(format!("ZIP finish error: {}", e)))?;
    Ok(())
}

static BLANK_PPTX: &[u8] = include_bytes!("../../templates/blank.pptx");

/// Builds a 100% valid OpenXML PPTX archive using the validated base presentation package
fn generate_pptx(
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

/// Builds a clean UTF-8 text file with BOM
fn generate_txt(
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

use std::sync::atomic::{AtomicU32, Ordering};

static ACTIVE_CONVERSION_PID: AtomicU32 = AtomicU32::new(0);
static CANCELLATION_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversionProgress {
    pub current: u32,
    pub total: u32,
    pub percent: u32,
    pub stage: String,
}

#[tauri::command]
pub fn cancel_conversion() -> Result<(), PdfError> {
    CANCELLATION_REQUESTED.store(true, Ordering::SeqCst);
    let pid = ACTIVE_CONVERSION_PID.swap(0, Ordering::SeqCst);
    if pid != 0 {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .creation_flags(0x08000000)
                .output();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
    }
    Ok(())
}

fn try_advanced_layout_engine(
    app: &tauri::AppHandle,
    input_pdf: &str,
    output_path: &str,
    format: &str,
) -> Result<(), String> {
    use tauri::{Emitter, Manager};
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::path::PathBuf;

    CANCELLATION_REQUESTED.store(false, Ordering::SeqCst);

    if let Err(e) = std::fs::File::open(input_pdf) {
        #[cfg(target_os = "windows")]
        if e.raw_os_error() == Some(32) {
            return Err("Dosya başka bir programda açık (!)".to_string());
        }
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err("Dosya başka bir programda açık (!)".to_string());
        }
    }

    if Path::new(output_path).exists() {
        if let Err(e) = std::fs::OpenOptions::new().write(true).open(output_path) {
            #[cfg(target_os = "windows")]
            if e.raw_os_error() == Some(32) {
                return Err("Dosya başka bir programda açık (!)".to_string());
            }
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return Err("Dosya başka bir programda açık (!)".to_string());
            }
        }
    }

    let mut script_path = None;

    if let Ok(res_dir) = app.path().resource_dir() {
        let p1 = res_dir.join("scripts").join("pdf2docx_engine.py");
        let p2 = res_dir.join("pdf2docx_engine.py");
        if p1.exists() {
            script_path = Some(p1);
        } else if p2.exists() {
            script_path = Some(p2);
        }
    }

    if script_path.is_none() {
        if let Ok(exe_dir) = std::env::current_exe().map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf()) {
            let candidates = [
                exe_dir.join("scripts").join("pdf2docx_engine.py"),
                exe_dir.join("resources").join("pdf2docx_engine.py"),
                exe_dir.join("pdf2docx_engine.py"),
                exe_dir.join("..").join("..").join("src-tauri").join("scripts").join("pdf2docx_engine.py"),
                exe_dir.join("..").join("src-tauri").join("scripts").join("pdf2docx_engine.py"),
            ];
            for c in &candidates {
                if c.exists() {
                    script_path = Some(c.clone());
                    break;
                }
            }
        }
    }

    if script_path.is_none() {
        let possible_script_paths = [
            "src-tauri/scripts/pdf2docx_engine.py",
            "scripts/pdf2docx_engine.py",
            "../src-tauri/scripts/pdf2docx_engine.py",
        ];
        for p in &possible_script_paths {
            if Path::new(p).exists() {
                script_path = Some(PathBuf::from(p));
                break;
            }
        }
    }

    let script = script_path.ok_or_else(|| "Dönüştürme modülü scripti bulunamadı".to_string())?;

    let python_candidates = [
        "python",
        "py",
        "python3",
    ];

    for py in &python_candidates {
        if CANCELLATION_REQUESTED.load(Ordering::SeqCst) {
            if Path::new(output_path).exists() {
                let _ = std::fs::remove_file(output_path);
            }
            return Err("Dönüştürme işlemi iptal edildi.".to_string());
        }

        let mut cmd = std::process::Command::new(py);
        cmd.arg("-u")
            .arg(&script)
            .arg("--input")
            .arg(input_pdf)
            .arg("--output")
            .arg(output_path)
            .arg("--format")
            .arg(format)
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => continue,
        };

        ACTIVE_CONVERSION_PID.store(child.id(), Ordering::SeqCst);

        let app_clone = app.clone();
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                if let Ok(progress) = serde_json::from_str::<ConversionProgress>(&line) {
                    let _ = app_clone.emit("conversion-progress", &progress);
                }
            }
        }

        let status = child.wait();
        ACTIVE_CONVERSION_PID.store(0, Ordering::SeqCst);

        if CANCELLATION_REQUESTED.load(Ordering::SeqCst) {
            if Path::new(output_path).exists() {
                let _ = std::fs::remove_file(output_path);
            }
            return Err("Dönüştürme işlemi iptal edildi.".to_string());
        }

        match status {
            Ok(s) if s.success() => {
                if Path::new(output_path).exists()
                    && std::fs::metadata(output_path).map(|m| m.len() > 0).unwrap_or(false)
                {
                    return Ok(());
                }
            }
            _ => {}
        }
    }

    if CANCELLATION_REQUESTED.load(Ordering::SeqCst) {
        if Path::new(output_path).exists() {
            let _ = std::fs::remove_file(output_path);
        }
        return Err("Dönüştürme işlemi iptal edildi.".to_string());
    }

    Err("Gelişmiş motor tamamlanamadı.".to_string())
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

    match fmt.as_str() {
        "docx" => {
            if let Some(ref in_path) = input_path {
                if Path::new(in_path).exists() {
                    match try_advanced_layout_engine(&app, in_path, &output_path, &fmt) {
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
            generate_docx(path, &doc_title, &pages, lang)
        }
        "pptx" => {
            if let Some(ref in_path) = input_path {
                if Path::new(in_path).exists() {
                    match try_advanced_layout_engine(&app, in_path, &output_path, &fmt) {
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
            generate_pptx(path, &pages, lang)
        }
        "xlsx" => generate_xlsx(path, &pages),
        "txt" => generate_txt(path, &pages),
        other => Err(PdfError::Custom(format!("Desteklenmeyen dosya formatı: {}", other))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let result = generate_docx(&docx_path, "Test Faturası", &pages, "tr");
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
        let result = generate_xlsx(&xlsx_path, &pages);
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
        assert!(slide1_xml.contains("Kahve Çekirdeği"));

        let _ = std::fs::remove_file(pptx_path);
    }

    #[test]
    fn test_txt_export() {
        let temp_dir = std::env::temp_dir();
        let txt_path = temp_dir.join("test_output.txt");
        let pages = sample_pages();
        let result = generate_txt(&txt_path, &pages);
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
