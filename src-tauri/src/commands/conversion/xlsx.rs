//! XLSX Conversion Engine Module
//! Handles Excel spreadsheet reconstruction and OpenXML packaging.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

use crate::error::PdfError;
use super::{escape_xml, run_python_engine, PageTextData};

/// Convert zero-indexed column number to Excel column letter (0 -> A, 1 -> B, 26 -> AA, etc.)
pub fn col_letter(mut col: usize) -> String {
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

pub fn convert(
    app: &tauri::AppHandle,
    input_path: Option<&str>,
    output_path: &Path,
    pages: &[PageTextData],
) -> Result<(), PdfError> {
    let out_str = output_path.to_string_lossy();
    if let Some(in_path) = input_path {
        if Path::new(in_path).exists() {
            match run_python_engine(app, in_path, &out_str, "xlsx") {
                Ok(()) => {
                    eprintln!("[XLSX] Yüksek kaliteli Excel tablosu başarıyla oluşturuldu.");
                    return Ok(());
                }
                Err(err) => {
                    eprintln!("[XLSX] Gelişmiş motor hatası: {}, yerel OpenXML deneniyor...", err);
                    if err.contains("iptal edildi") || err.contains("Dosya başka bir programda açık") {
                        return Err(PdfError::Custom(err));
                    }
                }
            }
        }
    }
    generate_xlsx(output_path, pages)
}

/// Builds a 100% valid OpenXML XLSX archive
pub fn generate_xlsx(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    fn test_xlsx_export_generation() {
        let temp_dir = std::env::temp_dir();
        let xlsx_path = temp_dir.join("test_xlsx_module.xlsx");
        let pages = vec![
            PageTextData {
                page: 1,
                text: "Ürün\tMiktar\tFiyat\nKahve Çekirdeği\t2\t150,00 TL".to_string(),
            },
        ];

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
}
