use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
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
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Valid XML 1.0 characters: 0x9 (tab), 0xA (LF), 0xD (CR), or >= 0x20
            c if (c as u32) < 32 && c != '\t' && c != '\n' && c != '\r' => {
                // strip forbidden ASCII control characters that break Office XML parsers
            }
            c => out.push(c),
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
            for line in p.text.lines() {
                let trimmed_line = line.trim();
                if trimmed_line.is_empty() {
                    body.push_str("<w:p/>\n");
                } else {
                    body.push_str(&format!(
                        r#"<w:p>
  <w:r>
    <w:t xml:space="preserve">{}</w:t>
  </w:r>
</w:p>"#,
                        escape_xml(line)
                    ));
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

    // 4. xl/styles.xml
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="2">
    <font>
      <sz val="11"/>
      <name val="Calibri"/>
    </font>
    <font>
      <b/>
      <sz val="11"/>
      <color rgb="FFFFFFFF"/>
      <name val="Calibri"/>
    </font>
  </fonts>
  <fills count="3">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="FF1E40AF"/><bgColor indexed="64"/></patternFill></fill>
  </fills>
  <borders count="2">
    <border><left/><right/><top/><bottom/><diagonal/></border>
    <border>
      <left style="thin"><color rgb="FFD1D5DB"/></left>
      <right style="thin"><color rgb="FFD1D5DB"/></right>
      <top style="thin"><color rgb="FFD1D5DB"/></top>
      <bottom style="thin"><color rgb="FFD1D5DB"/></bottom>
      <diagonal/>
    </border>
  </borders>
  <cellStyleXfs count="1">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
  </cellStyleXfs>
  <cellXfs count="3">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
    <xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1" applyAlignment="1">
      <alignment horizontal="center" vertical="center"/>
    </xf>
    <xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0" applyBorder="1"/>
  </cellXfs>
</styleSheet>"#;
    zip.start_file("xl/styles.xml", options)?;
    zip.write_all(styles.as_bytes())?;

    // 5. xl/workbook.xml
    let workbook = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sayfa 1" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#;
    zip.start_file("xl/workbook.xml", options)?;
    zip.write_all(workbook.as_bytes())?;

    // 6. xl/worksheets/sheet1.xml
    let mut sheet_data = String::new();

    // Header Row (Row 1)
    sheet_data.push_str(
        r#"<row r="1" ht="24" customHeight="1">
  <c r="A1" s="1" t="inlineStr"><is><t>Sayfa</t></is></c>
  <c r="B1" s="1" t="inlineStr"><is><t>Satır No</t></is></c>
  <c r="C1" s="1" t="inlineStr"><is><t>Metin İçeriği</t></is></c>
</row>"#,
    );

    let mut row_idx: usize = 2;
    for p in pages {
        let lines: Vec<&str> = p.text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            sheet_data.push_str(&format!(
                r#"<row r="{row}">
  <c r="A{row}" s="2"><v>{page}</v></c>
  <c r="B{row}" s="2"><v>1</v></c>
  <c r="C{row}" s="2" t="inlineStr"><is><t>(Boş veya Salt Görsel Sayfa)</t></is></c>
</row>"#,
                row = row_idx,
                page = p.page
            ));
            row_idx += 1;
        } else {
            for (line_num, line) in lines.iter().enumerate() {
                // If line contains tab characters, split into multiple columns
                let cols: Vec<&str> = line.split('\t').collect();
                if cols.len() > 1 {
                    sheet_data.push_str(&format!(
                        r#"<row r="{row}">
  <c r="A{row}" s="2"><v>{page}</v></c>
  <c r="B{row}" s="2"><v>{line_num}</v></c>"#,
                        row = row_idx,
                        page = p.page,
                        line_num = line_num + 1
                    ));
                    for (c_idx, col_val) in cols.iter().enumerate() {
                        let c_letter = col_letter(2 + c_idx);
                        sheet_data.push_str(&format!(
                            r#"<c r="{letter}{row}" s="2" t="inlineStr"><is><t xml:space="preserve">{val}</t></is></c>"#,
                            letter = c_letter,
                            row = row_idx,
                            val = escape_xml(col_val.trim())
                        ));
                    }
                    sheet_data.push_str("</row>");
                } else {
                    sheet_data.push_str(&format!(
                        r#"<row r="{row}">
  <c r="A{row}" s="2"><v>{page}</v></c>
  <c r="B{row}" s="2"><v>{line_num}</v></c>
  <c r="C{row}" s="2" t="inlineStr"><is><t xml:space="preserve">{val}</t></is></c>
</row>"#,
                        row = row_idx,
                        page = p.page,
                        line_num = line_num + 1,
                        val = escape_xml(line)
                    ));
                }
                row_idx += 1;
            }
        }
    }

    let worksheet = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cols>
    <col min="1" max="1" width="10" customWidth="1"/>
    <col min="2" max="2" width="10" customWidth="1"/>
    <col min="3" max="3" width="90" customWidth="1"/>
  </cols>
  <sheetData>
    {}
  </sheetData>
</worksheet>"#,
        sheet_data
    );

    zip.start_file("xl/worksheets/sheet1.xml", options)?;
    zip.write_all(worksheet.as_bytes())?;

    zip.finish().map_err(|e| PdfError::Custom(format!("ZIP finish error: {}", e)))?;
    Ok(())
}

/// Builds a 100% valid OpenXML PPTX archive
fn generate_pptx(
    output_path: &Path,
    pages: &[PageTextData],
    language: &str,
) -> Result<(), PdfError> {
    let file = File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let lang_tag = if language == "en" { "en-US" } else { "tr-TR" };
    let slide_count = if pages.is_empty() { 1 } else { pages.len() };

    // 1. [Content_Types].xml
    let mut slide_overrides = String::new();
    for i in 1..=slide_count {
        slide_overrides.push_str(&format!(
            r#"  <Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
"#
        ));
    }

    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
{}</Types>"#,
        slide_overrides
    );
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(content_types.as_bytes())?;

    // 2. _rels/.rels
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#;
    zip.start_file("_rels/.rels", options)?;
    zip.write_all(root_rels.as_bytes())?;

    // 3. ppt/_rels/presentation.xml.rels
    let mut pres_rels = String::new();
    pres_rels.push_str(
        r#"  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
"#,
    );
    for i in 1..=slide_count {
        pres_rels.push_str(&format!(
            r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>
"#,
            i + 1
        ));
    }
    let pres_rels_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{}</Relationships>"#,
        pres_rels
    );
    zip.start_file("ppt/_rels/presentation.xml.rels", options)?;
    zip.write_all(pres_rels_xml.as_bytes())?;

    // 4. ppt/presentation.xml
    let mut sld_id_lst = String::new();
    for i in 1..=slide_count {
        sld_id_lst.push_str(&format!(
            r#"    <p:sldId id="{}" r:id="rId{}"/>
"#,
            255 + i,
            i + 1
        ));
    }
    let presentation_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rId1"/>
  </p:sldMasterIdLst>
  <p:sldIdLst>
{}  </p:sldIdLst>
  <p:sldSz cx="12192000" cy="6858000" type="screen16x9"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#,
        sld_id_lst
    );
    zip.start_file("ppt/presentation.xml", options)?;
    zip.write_all(presentation_xml.as_bytes())?;

    // 5. ppt/theme/theme1.xml
    let theme_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme">
  <a:themeElements>
    <a:clrScheme name="Office">
      <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="1F497D"/></a:dk2>
      <a:lt2><a:srgbClr val="EEECE1"/></a:lt2>
      <a:accent1><a:srgbClr val="4F81BD"/></a:accent1>
      <a:accent2><a:srgbClr val="C0504D"/></a:accent2>
      <a:accent3><a:srgbClr val="9BBB59"/></a:accent3>
      <a:accent4><a:srgbClr val="8064A2"/></a:accent4>
      <a:accent5><a:srgbClr val="4BACC6"/></a:accent5>
      <a:accent6><a:srgbClr val="F79646"/></a:accent6>
      <a:hlink><a:srgbClr val="0000FF"/></a:hlink>
      <a:folHlink><a:srgbClr val="800080"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Office">
      <a:majorFont><a:latin typeface="Calibri"/></a:majorFont>
      <a:minorFont><a:latin typeface="Calibri"/></a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="Office">
      <a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>
      <a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>
      <a:effectStyleLst><a:effectLst/></a:effectStyleLst>
      <a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
  <a:objectDefaults/>
  <a:extraClrSchemeLst/>
</a:theme>"#;
    zip.start_file("ppt/theme/theme1.xml", options)?;
    zip.write_all(theme_xml.as_bytes())?;

    // 6. ppt/slideMasters/_rels/slideMaster1.xml.rels
    let sm_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#;
    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", options)?;
    zip.write_all(sm_rels.as_bytes())?;

    // 7. ppt/slideMasters/slideMaster1.xml
    let slide_master = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm>
          <a:off x="0" y="0"/>
          <a:ext cx="0" cy="0"/>
          <a:chOff x="0" y="0"/>
          <a:chExt cx="0" cy="0"/>
        </a:xfrm>
      </p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst>
    <p:sldLayoutId id="2147483649" r:id="rId1"/>
  </p:sldLayoutIdLst>
  <p:txStyles>
    <p:titleStyle/>
    <p:bodyStyle/>
    <p:otherStyle/>
  </p:txStyles>
</p:sldMaster>"#;
    zip.start_file("ppt/slideMasters/slideMaster1.xml", options)?;
    zip.write_all(slide_master.as_bytes())?;

    // 7. ppt/slideLayouts/_rels/slideLayout1.xml.rels
    let sl_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#;
    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", options)?;
    zip.write_all(sl_rels.as_bytes())?;

    // 8. ppt/slideLayouts/slideLayout1.xml
    let slide_layout = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             type="blank" preserve="1">
  <p:cSld name="Blank">
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm>
          <a:off x="0" y="0"/>
          <a:ext cx="0" cy="0"/>
          <a:chOff x="0" y="0"/>
          <a:chExt cx="0" cy="0"/>
        </a:xfrm>
      </p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr>
    <a:masterClrMapping/>
  </p:clrMapOvr>
</p:sldLayout>"#;
    zip.start_file("ppt/slideLayouts/slideLayout1.xml", options)?;
    zip.write_all(slide_layout.as_bytes())?;

    // 9. Slides & Slide Rels
    let default_page = PageTextData {
        page: 1,
        text: String::new(),
    };
    let pages_slice = if pages.is_empty() {
        std::slice::from_ref(&default_page)
    } else {
        pages
    };

    let slide_rel_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#;

    for (idx, p) in pages_slice.iter().enumerate() {
        let slide_num = idx + 1;

        // Slide rels
        let rel_path = format!("ppt/slides/_rels/slide{}.xml.rels", slide_num);
        zip.start_file(&rel_path, options)?;
        zip.write_all(slide_rel_xml.as_bytes())?;

        // Slide content
        let mut paragraphs = String::new();
        let trimmed = p.text.trim();
        if trimmed.is_empty() {
            paragraphs.push_str(&format!(
                r#"          <a:p>
            <a:r>
              <a:rPr lang="{lang_tag}" sz="1400" i="1">
                <a:solidFill><a:srgbClr val="6B7280"/></a:solidFill>
              </a:rPr>
              <a:t>(Bu sayfa görsel içeriklidir veya metin katmanı içermemektedir)</a:t>
            </a:r>
          </a:p>"#
            ));
        } else {
            for line in p.text.lines() {
                let trimmed_l = line.trim();
                if trimmed_l.is_empty() {
                    paragraphs.push_str("          <a:p/>\n");
                } else {
                    paragraphs.push_str(&format!(
                        r#"          <a:p>
            <a:r>
              <a:rPr lang="{lang_tag}" sz="1400">
                <a:solidFill><a:srgbClr val="1E293B"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
"#,
                        escape_xml(line)
                    ));
                }
            }
        }

        let slide_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm>
          <a:off x="0" y="0"/>
          <a:ext cx="0" cy="0"/>
          <a:chOff x="0" y="0"/>
          <a:chExt cx="0" cy="0"/>
        </a:xfrm>
      </p:grpSpPr>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="685800" y="457200"/>
            <a:ext cx="10820400" cy="685800"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        </p:spPr>
        <p:txBody>
          <a:bodyPr vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0"/>
          <a:lstStyle/>
          <a:p>
            <a:r>
              <a:rPr lang="{lang_tag}" sz="2800" b="1">
                <a:solidFill><a:srgbClr val="1E40AF"/></a:solidFill>
              </a:rPr>
              <a:t>Sayfa {}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Content"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="685800" y="1300000"/>
            <a:ext cx="10820400" cy="5000000"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        </p:spPr>
        <p:txBody>
          <a:bodyPr vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0"/>
          <a:lstStyle/>
{}
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr>
    <a:masterClrMapping/>
  </p:clrMapOvr>
</p:sld>"#,
            p.page,
            paragraphs
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

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::Emitter;

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

/// Attempts high-fidelity layout analysis & reconstruction via the local Python engine.
/// Supports real-time JSON progress streaming and graceful fallback.
fn try_advanced_layout_engine(
    app: &tauri::AppHandle,
    input_pdf: &str,
    output_path: &str,
    format: &str,
) -> Result<(), String> {
    use tauri::Manager;
    CANCELLATION_REQUESTED.store(false, Ordering::SeqCst);

    // 0. Pre-checks for file lock / sharing violation (e.g. file open in another program)
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

    // 1. Check Tauri resource directory
    if let Ok(res_dir) = app.path().resource_dir() {
        let p1 = res_dir.join("scripts").join("pdf2docx_engine.py");
        let p2 = res_dir.join("pdf2docx_engine.py");
        if p1.exists() {
            script_path = Some(p1);
        } else if p2.exists() {
            script_path = Some(p2);
        }
    }

    // 2. Check exe-relative directories
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

    // 3. Check relative and known workspace paths
    if script_path.is_none() {
        let possible_script_paths = [
            "src-tauri/scripts/pdf2docx_engine.py",
            "scripts/pdf2docx_engine.py",
            "../src-tauri/scripts/pdf2docx_engine.py",
            "C:\\Users\\CAGDASS\\Desktop\\PDFKit\\src-tauri\\scripts\\pdf2docx_engine.py",
        ];
        for p in &possible_script_paths {
            if Path::new(p).exists() {
                script_path = Some(PathBuf::from(p));
                break;
            }
        }
    }

    let script = script_path.ok_or_else(|| "Dönüştürme modülü bileşeni bulunamadı".to_string())?;

    // 4. Try python candidates:
    let python_candidates = [
        "python",
        "C:\\Program Files\\Python314\\python.exe",
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
        cmd.arg(&script)
            .arg("--input")
            .arg(input_pdf)
            .arg("--output")
            .arg(output_path)
            .arg("--format")
            .arg(format)
            .stdout(Stdio::piped())
            .stderr(Stdio::null()); // IMPORTANT: Do not pipe stderr without reading it, to prevent Windows OS pipe buffer deadlock!

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: prevent CMD flash
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => continue,
        };

        let pid = child.id();
        ACTIVE_CONVERSION_PID.store(pid, Ordering::SeqCst);

        let mut child_error: Option<String> = None;

        // Read stdout line by line for real-time progress and messages
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let app_clone = app.clone();
            for line in reader.lines().flatten() {
                if CANCELLATION_REQUESTED.load(Ordering::SeqCst) {
                    let _ = child.kill();
                    break;
                }
                if line.starts_with('{') {
                    if let Ok(progress) = serde_json::from_str::<ConversionProgress>(&line) {
                        let _ = app_clone.emit("conversion-progress", &progress);
                    } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        if val.get("type").and_then(|t| t.as_str()) == Some("error") {
                            if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                                child_error = Some(msg.to_string());
                            }
                        }
                    }
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
            _ => {
                if let Some(err_msg) = child_error {
                    if err_msg.contains("Permission denied") || err_msg.contains("Errno 13") || err_msg.contains("Sharing violation") {
                        return Err("Dosya başka bir programda açık (!)".to_string());
                    }
                }
            }
        }
    }

    if CANCELLATION_REQUESTED.load(Ordering::SeqCst) {
        if Path::new(output_path).exists() {
            let _ = std::fs::remove_file(output_path);
        }
        return Err("Dönüştürme işlemi iptal edildi.".to_string());
    }

    Err("Dönüştürme işlemi tamamlanamadı. Lütfen belgenizin açık olmadığından ve geçerli bir dosya olduğundan emin olun.".to_string())
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
    let path = Path::new(&output_path);
    let lang = language.as_deref().unwrap_or("tr");
    let fmt = format.to_lowercase();

    match fmt.as_str() {
        "docx" | "pptx" => {
            // Attempt advanced layout analysis & reconstruction via backend engine
            if let Some(ref in_path) = input_path {
                if Path::new(in_path).exists() {
                    match try_advanced_layout_engine(&app, in_path, &output_path, &fmt) {
                        Ok(()) => {
                            eprintln!("[{}] Gelişmiş ofis yeniden inşası başarıyla tamamlandı.", fmt.to_uppercase());
                            return Ok(());
                        }
                        Err(err) => {
                            return Err(PdfError::Custom(err));
                        }
                    }
                }
            }

            // Fallback to built-in clean OpenXML generator for tests or direct text payloads
            if fmt == "docx" {
                generate_docx(path, &doc_title, &pages, lang)
            } else {
                generate_pptx(path, &pages, lang)
            }
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
