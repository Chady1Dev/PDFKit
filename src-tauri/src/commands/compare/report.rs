use crate::commands::compare::types::ComparisonResult;
use crate::error::PdfError;
use crate::types::OpResult;
use std::path::Path;

pub fn write_comparison_report(
    result: &ComparisonResult,
    output_path: &Path,
    format: &str,
) -> Result<OpResult, PdfError> {
    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let fmt = format.trim().to_ascii_lowercase();
    let bytes: Vec<u8> = match fmt.as_str() {
        "json" => {
            let s = serde_json::to_string_pretty(result)
                .map_err(|e| PdfError::Custom(format!("JSON serileştirme hatası: {}", e)))?;
            s.into_bytes()
        }
        "csv" => build_csv_report(result),
        "html" => build_html_report(result).into_bytes(),
        _ => {
            return Err(PdfError::Custom(format!(
                "Desteklenmeyen rapor formatı: {} (izin verilen: json, csv, html)",
                format
            )));
        }
    };

    std::fs::write(output_path, &bytes)?;

    Ok(OpResult {
        outputs: vec![output_path.to_string_lossy().into_owned()],
        total_pages: result.left_page_count.max(result.right_page_count),
        total_size_bytes: bytes.len() as u64,
    })
}

fn csv_escape(s: &str) -> String {
    let cleaned = s.replace('\r', " ").replace('\n', " ");
    if cleaned.contains(',') || cleaned.contains('"') || cleaned.contains(';') {
        format!("\"{}\"", cleaned.replace('"', "\"\""))
    } else {
        cleaned
    }
}

fn build_csv_report(result: &ComparisonResult) -> Vec<u8> {
    // Prefix with UTF-8 BOM so Microsoft Excel renders Turkish characters cleanly
    let mut out = vec![0xEF, 0xBB, 0xBF];
    let mut csv = String::new();
    csv.push_str("ID,Katman,Kategori,Tur,Sol_Sayfa,Sag_Sayfa,Baslik,Aciklama,Onceki_Deger,Yeni_Deger,Sezgisel\r\n");

    for c in &result.changes {
        let line = format!(
            "{},{:?},{:?},{:?},{},{},{},{},{},{},{}\r\n",
            csv_escape(&c.id),
            c.layer,
            c.category,
            c.kind,
            c.left_page.map(|p| p.to_string()).unwrap_or_default(),
            c.right_page.map(|p| p.to_string()).unwrap_or_default(),
            csv_escape(&c.title),
            csv_escape(&c.detail),
            csv_escape(c.before_text.as_deref().unwrap_or("")),
            csv_escape(c.after_text.as_deref().unwrap_or("")),
            if c.is_heuristic { "Evet" } else { "Hayir" }
        );
        csv.push_str(&line);
    }

    out.extend_from_slice(csv.as_bytes());
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn build_html_report(result: &ComparisonResult) -> String {
    let mut rows = String::new();
    for (idx, c) in result.changes.iter().enumerate() {
        let layer_badge = match c.layer {
            crate::commands::compare::types::ChangeLayer::Text => "Metin",
            crate::commands::compare::types::ChangeLayer::Visual => "Görsel",
            crate::commands::compare::types::ChangeLayer::Structure => "Yapısal",
        };
        rows.push_str(&format!(
            "<tr>
                <td>{}</td>
                <td><span class=\"badge\">{}</span></td>
                <td>{:?}</td>
                <td>{}</td>
                <td>{}</td>
                <td><strong>{}</strong><br/><small>{}</small></td>
                <td class=\"del\">{}</td>
                <td class=\"add\">{}</td>
            </tr>",
            idx + 1,
            layer_badge,
            c.category,
            c.left_page.map(|p| p.to_string()).unwrap_or_else(|| "—".into()),
            c.right_page.map(|p| p.to_string()).unwrap_or_else(|| "—".into()),
            html_escape(&c.title),
            html_escape(&c.detail),
            html_escape(c.before_text.as_deref().unwrap_or("—")),
            html_escape(c.after_text.as_deref().unwrap_or("—")),
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="tr">
<head>
<meta charset="UTF-8" />
<title>PDFKit Karşılaştırma Raporu</title>
<style>
  body {{ font-family: system-ui, -apple-system, sans-serif; margin: 24px; color: #18181b; background: #fafafa; }}
  h1 {{ font-size: 20px; margin-bottom: 4px; }}
  .meta {{ font-size: 13px; color: #52525b; margin-bottom: 16px; }}
  .cards {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px; margin-bottom: 20px; }}
  .card {{ background: #fff; border: 1px solid #e4e4e7; border-radius: 10px; padding: 12px; }}
  .card .num {{ font-size: 22px; font-weight: 700; color: #4f46e5; }}
  .card .lbl {{ font-size: 12px; color: #71717a; }}
  table {{ width: 100%; border-collapse: collapse; background: #fff; border-radius: 10px; overflow: hidden; border: 1px solid #e4e4e7; font-size: 13px; }}
  th, td {{ padding: 10px 12px; border-bottom: 1px solid #f4f4f5; text-align: left; vertical-align: top; }}
  th {{ background: #f4f4f5; font-weight: 600; }}
  .del {{ color: #be123c; background: #fff1f2; }}
  .add {{ color: #047857; background: #ecfdf5; }}
  .badge {{ display: inline-block; padding: 2px 8px; border-radius: 999px; background: #e0e7ff; color: #3730a3; font-size: 11px; font-weight: 600; }}
</style>
</head>
<body>
  <h1>PDFKit — Belge Karşılaştırma Raporu</h1>
  <div class="meta">
    <div><strong>Referans (Sol) PDF:</strong> {} ({} sayfa)</div>
    <div><strong>Karşılaştırılan (Sağ) PDF:</strong> {} ({} sayfa)</div>
    <div><strong>Mod:</strong> {:?} · <strong>Süre:</strong> {} ms</div>
  </div>
  <div class="cards">
    <div class="card"><div class="num">{}</div><div class="lbl">Toplam Değişiklik</div></div>
    <div class="card"><div class="num">{}</div><div class="lbl">Metin Ekleme / Silme / Değişim</div></div>
    <div class="card"><div class="num">{}</div><div class="lbl">Biçim & Görsel Fark</div></div>
    <div class="card"><div class="num">{}</div><div class="lbl">Sayfa Yapısı Farkı</div></div>
  </div>
  <table>
    <thead>
      <tr>
        <th>#</th>
        <th>Katman</th>
        <th>Kategori</th>
        <th>Sol Sayfa</th>
        <th>Sağ Sayfa</th>
        <th>Değişiklik</th>
        <th>Önceki Değer</th>
        <th>Yeni Değer</th>
      </tr>
    </thead>
    <tbody>
      {}
    </tbody>
  </table>
</body>
</html>"#,
        html_escape(&result.left_path),
        result.left_page_count,
        html_escape(&result.right_path),
        result.right_page_count,
        result.mode,
        result.duration_ms,
        result.summary.total_changes,
        result.summary.text_added_count
            + result.summary.text_deleted_count
            + result.summary.text_modified_count
            + result.summary.text_moved_count,
        result.summary.formatting_changes_count
            + result.summary.visual_changes_count
            + result.summary.image_graphic_changes_count,
        result.summary.pages_added_count
            + result.summary.pages_deleted_count
            + result.summary.pages_moved_count,
        rows
    )
}
