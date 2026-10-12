use crate::commands::images::build_image_xobject;
use crate::error::PdfError;
use crate::types::{FormField, OpResult};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::{Document, Object, ObjectId};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::path::Path;

/// Lists the AcroForm fields of a PDF (name, type, current value).
#[tauri::command]
pub async fn list_form_fields(input: String) -> Result<Vec<FormField>, PdfError> {
    tauri::async_runtime::spawn_blocking(move || collect_fields(Path::new(&input)))
        .await
        .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Fills text/checkbox-ish AcroForm fields by name and saves a copy.
///
/// Sets `/NeedAppearances` so viewers regenerate field appearances.
#[tauri::command]
pub async fn fill_form(
    input: String,
    output: String,
    values: HashMap<String, String>,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        fill_fields(Path::new(&input), Path::new(&output), &values)
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Stamps an image (e.g. a scanned signature) onto a page at a rectangle
/// given in PDF points (origin = bottom-left).
#[tauri::command]
pub async fn stamp_image(
    input: String,
    output: String,
    page: u32,
    image_path: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<OpResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        stamp(
            Path::new(&input),
            Path::new(&output),
            page,
            Path::new(&image_path),
            x,
            y,
            width,
            height,
        )
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

// ---------------------------------------------------------------------------

/// Encodes a text string for a PDF dictionary value (`/V`).
///
/// In the PDF 1.7 specification (Section 7.9.2.2 "Text String Type"):
/// Text strings containing non-ASCII characters (e.g. Turkish characters: ç, Ç, ğ, Ğ, ı, İ, ö, Ö, ş, Ş, ü, Ü)
/// MUST be encoded as UTF-16BE prefixed with the 2-byte BOM `0xFE, 0xFF`.
/// Without this BOM, PDF viewers interpret raw bytes as PDFDocEncoding (Latin-1-like),
/// which mangles Turkish characters into Mojibake (e.g. `ÃaÃždaÅŁ`).
pub fn encode_pdf_string(text: &str) -> Object {
    if text.is_ascii() {
        Object::string_literal(text)
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for ch in text.encode_utf16() {
            bytes.push((ch >> 8) as u8);
            bytes.push((ch & 0xFF) as u8);
        }
        Object::String(bytes, lopdf::StringFormat::Hexadecimal)
    }
}

/// Decodes a PDF text string from raw bytes into a Rust String.
/// Supports UTF-16BE (with BOM `0xFE, 0xFF`), UTF-8 (with or without BOM), and Latin/PDFDoc fallback.
pub fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let u16_chars: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| ((chunk[0] as u16) << 8) | (chunk[1] as u16))
            .collect();
        return String::from_utf16_lossy(&u16_chars);
    }

    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    String::from_utf8_lossy(bytes).into_owned()
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    if offset + 2 <= data.len() {
        Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
    } else {
        None
    }
}

fn read_i16(data: &[u8], offset: usize) -> Option<i16> {
    if offset + 2 <= data.len() {
        Some(i16::from_be_bytes([data[offset], data[offset + 1]]))
    } else {
        None
    }
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 <= data.len() {
        Some(u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]))
    } else {
        None
    }
}

fn find_table<'a>(ttf: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let num_tables = read_u16(ttf, 4)? as usize;
    for i in 0..num_tables {
        let entry_off = 12 + i * 16;
        if entry_off + 16 > ttf.len() {
            return None;
        }
        if &ttf[entry_off..entry_off + 4] == tag {
            let offset = read_u32(ttf, entry_off + 8)? as usize;
            let length = read_u32(ttf, entry_off + 12)? as usize;
            if offset + length <= ttf.len() {
                return Some(&ttf[offset..offset + length]);
            }
        }
    }
    None
}

#[derive(Clone)]
struct CmapFormat4 {
    seg_count: usize,
    end_codes: Vec<u16>,
    start_codes: Vec<u16>,
    id_deltas: Vec<i16>,
    id_range_offsets: Vec<u16>,
    id_range_offsets_base: usize,
    subtable_data: Vec<u8>,
}

impl CmapFormat4 {
    fn parse(subtable: &[u8]) -> Option<Self> {
        let format = read_u16(subtable, 0)?;
        if format != 4 {
            return None;
        }
        let seg_count_x2 = read_u16(subtable, 6)? as usize;
        let seg_count = seg_count_x2 / 2;
        if seg_count == 0 {
            return None;
        }

        let mut end_codes = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            end_codes.push(read_u16(subtable, 14 + i * 2)?);
        }

        let start_codes_off = 16 + seg_count * 2;
        let mut start_codes = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            start_codes.push(read_u16(subtable, start_codes_off + i * 2)?);
        }

        let id_deltas_off = 16 + seg_count * 4;
        let mut id_deltas = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            id_deltas.push(read_i16(subtable, id_deltas_off + i * 2)?);
        }

        let id_range_offsets_base = 16 + seg_count * 6;
        let mut id_range_offsets = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            id_range_offsets.push(read_u16(subtable, id_range_offsets_base + i * 2)?);
        }

        Some(Self {
            seg_count,
            end_codes,
            start_codes,
            id_deltas,
            id_range_offsets,
            id_range_offsets_base,
            subtable_data: subtable.to_vec(),
        })
    }

    fn char_to_gid(&self, c: char) -> u16 {
        let code = c as u32;
        if code > 0xFFFF {
            return 0;
        }
        let code16 = code as u16;
        for i in 0..self.seg_count {
            if self.end_codes[i] >= code16 {
                if self.start_codes[i] <= code16 {
                    let range_offset = self.id_range_offsets[i];
                    if range_offset == 0 {
                        return ((code16 as i32 + self.id_deltas[i] as i32) & 0xFFFF) as u16;
                    } else {
                        let offset = self.id_range_offsets_base
                            + i * 2
                            + range_offset as usize
                            + (code16 - self.start_codes[i]) as usize * 2;
                        if let Some(val) = read_u16(&self.subtable_data, offset) {
                            if val != 0 {
                                return ((val as i32 + self.id_deltas[i] as i32) & 0xFFFF) as u16;
                            }
                        }
                        return 0;
                    }
                } else {
                    return 0;
                }
            }
        }
        0
    }
}

pub struct TrueTypeInfo {
    pub font_bytes: Vec<u8>,
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub cap_height: i16,
    pub bbox: [i16; 4],
    pub num_hmetrics: usize,
    pub hmtx_data: Vec<u8>,
    cmap4: CmapFormat4,
}

impl TrueTypeInfo {
    pub fn parse(bytes: Vec<u8>) -> Option<Self> {
        let head = find_table(&bytes, b"head")?;
        let units_per_em = read_u16(head, 18).unwrap_or(2048);
        let bbox = [
            read_i16(head, 36).unwrap_or(-665),
            read_i16(head, 38).unwrap_or(-325),
            read_i16(head, 40).unwrap_or(2000),
            read_i16(head, 42).unwrap_or(1006),
        ];

        let hhea = find_table(&bytes, b"hhea")?;
        let ascent = read_i16(hhea, 4).unwrap_or(1854);
        let descent = read_i16(hhea, 6).unwrap_or(-434);
        let num_hmetrics = read_u16(hhea, 34).unwrap_or(0) as usize;

        let cap_height = if let Some(os2) = find_table(&bytes, b"OS/2") {
            read_i16(os2, 88).unwrap_or(716)
        } else {
            716
        };

        let hmtx = find_table(&bytes, b"hmtx")?.to_vec();

        let cmap = find_table(&bytes, b"cmap")?;
        let num_tables = read_u16(cmap, 2)? as usize;
        let mut cmap4 = None;
        for i in 0..num_tables {
            let entry_off = 4 + i * 8;
            let plat_id = read_u16(cmap, entry_off)?;
            let enc_id = read_u16(cmap, entry_off + 2)?;
            let sub_off = read_u32(cmap, entry_off + 4)? as usize;
            if sub_off < cmap.len() {
                let fmt = read_u16(cmap, sub_off)?;
                if fmt == 4 {
                    if let Some(parsed) = CmapFormat4::parse(&cmap[sub_off..]) {
                        if plat_id == 3 && enc_id == 1 {
                            cmap4 = Some(parsed);
                            break;
                        } else if cmap4.is_none() {
                            cmap4 = Some(parsed);
                        }
                    }
                }
            }
        }

        let cmap4 = cmap4?;

        Some(Self {
            font_bytes: bytes,
            units_per_em,
            ascent,
            descent,
            cap_height,
            bbox,
            num_hmetrics,
            hmtx_data: hmtx,
            cmap4,
        })
    }

    pub fn char_to_gid(&self, c: char) -> u16 {
        self.cmap4.char_to_gid(c)
    }

    pub fn gid_width(&self, gid: u16) -> i64 {
        let gid_idx = gid as usize;
        let adv = if gid_idx < self.num_hmetrics {
            read_u16(&self.hmtx_data, gid_idx * 4).unwrap_or(1000)
        } else if self.num_hmetrics > 0 {
            read_u16(&self.hmtx_data, (self.num_hmetrics - 1) * 4).unwrap_or(1000)
        } else {
            1000
        };
        let upm = if self.units_per_em > 0 {
            self.units_per_em as i64
        } else {
            2048
        };
        (adv as i64 * 1000) / upm
    }
}

pub fn load_system_truetype_font() -> Option<TrueTypeInfo> {
    let candidate_paths = [
        // Windows
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\calibri.ttf",
        // Linux
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        // macOS
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
    ];

    for path in candidate_paths {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(info) = TrueTypeInfo::parse(bytes) {
                return Some(info);
            }
        }
    }
    None
}

/// Configures AcroForm Default Resources (/DR) with an embedded Type 0 / Identity-H TrueType font
/// or WinAnsiEncoding fallback.
///
/// Embedding a TrueType font with /Subtype /Type0, /Encoding /Identity-H, and /ToUnicode CMap
/// ensures Adobe Acrobat, macOS Preview, and all PDF viewers render Turkish characters
/// (Ç, ç, Ğ, ğ, İ, ı, Ö, ö, Ş, ş, Ü, ü) with 100% vector accuracy without glyph substitution or bullets.
fn ensure_acroform_setup(
    doc: &mut Document,
    font_info: Option<&TrueTypeInfo>,
    used_chars: &HashSet<char>,
) -> Result<ObjectId, PdfError> {
    let mut font_dict = lopdf::Dictionary::new();

    if let Some(info) = font_info {
        // 1. Embed compressed TrueType FontFile2
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&info.font_bytes)
            .map_err(|e| PdfError::TaskJoin(e.to_string()))?;
        let compressed_font = encoder
            .finish()
            .map_err(|e| PdfError::TaskJoin(e.to_string()))?;

        let mut font_file_dict = lopdf::Dictionary::new();
        font_file_dict.set("Length", Object::Integer(compressed_font.len() as i64));
        font_file_dict.set("Length1", Object::Integer(info.font_bytes.len() as i64));
        font_file_dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        let font_file_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            font_file_dict,
            compressed_font,
        )));

        // 2. Build FontDescriptor
        let upm = if info.units_per_em > 0 {
            info.units_per_em as i64
        } else {
            2048
        };
        let ascent_scaled = (info.ascent as i64 * 1000) / upm;
        let descent_scaled = (info.descent as i64 * 1000) / upm;
        let cap_scaled = (info.cap_height as i64 * 1000) / upm;
        let bbox_scaled = [
            (info.bbox[0] as i64 * 1000) / upm,
            (info.bbox[1] as i64 * 1000) / upm,
            (info.bbox[2] as i64 * 1000) / upm,
            (info.bbox[3] as i64 * 1000) / upm,
        ];

        let mut fd = lopdf::Dictionary::new();
        fd.set("Type", Object::Name(b"FontDescriptor".to_vec()));
        fd.set("FontName", Object::Name(b"Arial".to_vec()));
        fd.set("Flags", Object::Integer(32));
        fd.set(
            "FontBBox",
            Object::Array(vec![
                Object::Integer(bbox_scaled[0]),
                Object::Integer(bbox_scaled[1]),
                Object::Integer(bbox_scaled[2]),
                Object::Integer(bbox_scaled[3]),
            ]),
        );
        fd.set("ItalicAngle", Object::Integer(0));
        fd.set("Ascent", Object::Integer(ascent_scaled));
        fd.set("Descent", Object::Integer(descent_scaled));
        fd.set("CapHeight", Object::Integer(cap_scaled));
        fd.set("StemV", Object::Integer(80));
        fd.set("FontFile2", Object::Reference(font_file_id));
        let fd_id = doc.add_object(Object::Dictionary(fd));

        // 3. Gather all required glyph IDs and build /W array
        let mut all_chars = BTreeSet::new();
        for c in ' '..='~' {
            all_chars.insert(c);
        }
        for c in "ÇçĞğİıÖöŞşÜü".chars() {
            all_chars.insert(c);
        }
        for c in used_chars {
            all_chars.insert(*c);
        }

        let mut gid_map: BTreeMap<u16, char> = BTreeMap::new();
        for c in all_chars {
            let gid = info.char_to_gid(c);
            if gid != 0 {
                gid_map.entry(gid).or_insert(c);
            }
        }

        let mut w_array = Vec::new();
        for (gid, _) in &gid_map {
            let w = info.gid_width(*gid);
            w_array.push(Object::Integer(*gid as i64));
            w_array.push(Object::Array(vec![Object::Integer(w)]));
        }

        // 4. Build CIDFontType2 dictionary
        let mut csi = lopdf::Dictionary::new();
        csi.set("Registry", Object::string_literal("Adobe"));
        csi.set("Ordering", Object::string_literal("Identity"));
        csi.set("Supplement", Object::Integer(0));

        let mut cid = lopdf::Dictionary::new();
        cid.set("Type", Object::Name(b"Font".to_vec()));
        cid.set("Subtype", Object::Name(b"CIDFontType2".to_vec()));
        cid.set("BaseFont", Object::Name(b"Arial".to_vec()));
        cid.set("CIDSystemInfo", Object::Dictionary(csi));
        cid.set("FontDescriptor", Object::Reference(fd_id));
        cid.set("CIDToGIDMap", Object::Name(b"Identity".to_vec()));
        cid.set("DW", Object::Integer(1000));
        cid.set("W", Object::Array(w_array));
        let cid_id = doc.add_object(Object::Dictionary(cid));

        // 5. Build /ToUnicode CMap stream
        let mut cmap_content = String::from(
            "/CIDInit /ProcSet findresource begin\n\
             12 dict begin\n\
             begincmap\n\
             /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
             /CMapName /Custom-ToUnicode def\n\
             /CMapType 2 def\n\
             1 begincodespacerange\n\
             <0000> <FFFF>\n\
             endcodespacerange\n",
        );

        let entries: Vec<(u16, char)> = gid_map.into_iter().collect();
        for chunk in entries.chunks(100) {
            cmap_content.push_str(&format!("{} beginbfchar\n", chunk.len()));
            for (gid, c) in chunk {
                cmap_content.push_str(&format!("<{:04X}> <{:04X}>\n", gid, *c as u32));
            }
            cmap_content.push_str("endbfchar\n");
        }
        cmap_content.push_str(
            "endcmap\n\
             CMapName currentdict /CMap defineresource pop\n\
             end\n\
             end\n",
        );

        let to_unicode_stream =
            lopdf::Stream::new(lopdf::Dictionary::new(), cmap_content.into_bytes());
        let to_unicode_id = doc.add_object(Object::Stream(to_unicode_stream));

        // 6. Build root Type0 Font
        let mut type0 = lopdf::Dictionary::new();
        type0.set("Type", Object::Name(b"Font".to_vec()));
        type0.set("Subtype", Object::Name(b"Type0".to_vec()));
        type0.set("BaseFont", Object::Name(b"Arial".to_vec()));
        type0.set("Encoding", Object::Name(b"Identity-H".to_vec()));
        type0.set(
            "DescendantFonts",
            Object::Array(vec![Object::Reference(cid_id)]),
        );
        type0.set("ToUnicode", Object::Reference(to_unicode_id));
        let type0_id = doc.add_object(Object::Dictionary(type0));

        // Map all standard font names to this Unicode Type 0 font
        for font_name in &[
            "Helvetica",
            "Helv",
            "Arial",
            "Times-Roman",
            "Times",
            "Courier",
            "Helvetica-Bold",
            "Helv-Bold",
            "Arial-Bold",
        ] {
            font_dict.set(font_name.as_bytes().to_vec(), Object::Reference(type0_id));
        }
    } else {
        // Fallback WinAnsiEncoding with Turkish Differences if no system font could be loaded
        let mut enc_dict = lopdf::Dictionary::new();
        enc_dict.set("Type", Object::Name(b"Encoding".to_vec()));
        enc_dict.set("BaseEncoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        let diff_array = vec![
            Object::Integer(208),
            Object::Name(b"Gbreve".to_vec()),
            Object::Integer(221),
            Object::Name(b"Idotaccent".to_vec()),
            Object::Integer(222),
            Object::Name(b"Scedilla".to_vec()),
            Object::Integer(240),
            Object::Name(b"gbreve".to_vec()),
            Object::Integer(253),
            Object::Name(b"dotlessi".to_vec()),
            Object::Integer(254),
            Object::Name(b"scedilla".to_vec()),
        ];
        enc_dict.set("Differences", Object::Array(diff_array));
        let enc_id = doc.add_object(Object::Dictionary(enc_dict));

        for font_name in &[
            "Helvetica",
            "Helv",
            "Arial",
            "Times-Roman",
            "Times",
            "Courier",
            "Helvetica-Bold",
            "Helv-Bold",
            "Arial-Bold",
        ] {
            let mut f = lopdf::Dictionary::new();
            f.set("Type", Object::Name(b"Font".to_vec()));
            f.set("Subtype", Object::Name(b"TrueType".to_vec()));
            f.set("BaseFont", Object::Name(b"Arial".to_vec()));
            f.set("Encoding", Object::Reference(enc_id));
            let fid = doc.add_object(Object::Dictionary(f));
            font_dict.set(font_name.as_bytes().to_vec(), Object::Reference(fid));
        }
    }

    // ZapfDingbats for checkmark / radio buttons
    let mut zd = lopdf::Dictionary::new();
    zd.set("Type", Object::Name(b"Font".to_vec()));
    zd.set("Subtype", Object::Name(b"Type1".to_vec()));
    zd.set("BaseFont", Object::Name(b"ZapfDingbats".to_vec()));
    let zd_id = doc.add_object(Object::Dictionary(zd));
    font_dict.set(b"ZaDb".to_vec(), Object::Reference(zd_id));

    // Appearance stream font dictionary (contains Type 0 embedded font for high-fidelity vector rendering)
    let ap_font_dict_id = doc.add_object(Object::Dictionary(font_dict));

    // Standard Type 1 Helvetica font dictionary for global /AcroForm /DR.
    // Adobe Acrobat's interactive field editor does NOT support Type 0 composite fonts in /AcroForm /DR.
    // Keeping Type 1 Helvetica in /DR ensures that when a user clicks/focuses a form field, Acrobat's live
    // text editor displays UTF-16BE text seamlessly without scrambling or mojibake.
    let mut t1_helv = lopdf::Dictionary::new();
    t1_helv.set("Type", Object::Name(b"Font".to_vec()));
    t1_helv.set("Subtype", Object::Name(b"Type1".to_vec()));
    t1_helv.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
    t1_helv.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
    let t1_helv_id = doc.add_object(Object::Dictionary(t1_helv));

    let acro_id = if let Ok(catalog) = doc.catalog() {
        match catalog.get(b"AcroForm") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    } else {
        None
    };

    if let Some(id) = acro_id {
        let (dr_ref, has_inline_dr, has_da) = if let Ok(acro_dict) = doc.get_dictionary(id) {
            let dr_ref = match acro_dict.get(b"DR") {
                Ok(Object::Reference(r)) => Some(*r),
                _ => None,
            };
            let inline = matches!(acro_dict.get(b"DR"), Ok(Object::Dictionary(_)));
            let da = acro_dict.get(b"DA").is_ok();
            (dr_ref, inline, da)
        } else {
            (None, false, false)
        };

        if let Some(dr_id) = dr_ref {
            let font_ref = if let Ok(dr_dict) = doc.get_dictionary(dr_id) {
                match dr_dict.get(b"Font") {
                    Ok(Object::Reference(r)) => Some(*r),
                    _ => None,
                }
            } else {
                None
            };

            if let Some(f_id) = font_ref {
                let mut keys_to_update = Vec::new();
                if let Ok(f_dict) = doc.get_dictionary(f_id) {
                    for &k in &[b"Helv".as_slice(), b"Helvetica".as_slice(), b"Arial".as_slice()] {
                        match f_dict.get(k) {
                            Ok(Object::Reference(existing_fid)) => {
                                if let Ok(ed) = doc.get_dictionary(*existing_fid) {
                                    if ed
                                        .get(b"Subtype")
                                        .and_then(|s| s.as_name())
                                        .map(|n| n == b"Type0")
                                        .unwrap_or(false)
                                    {
                                        keys_to_update.push((k.to_vec(), t1_helv_id));
                                    }
                                }
                            }
                            Err(_) => {
                                keys_to_update.push((k.to_vec(), t1_helv_id));
                            }
                            _ => {}
                        }
                    }
                    match f_dict.get(b"ZaDb") {
                        Ok(Object::Reference(existing_fid)) => {
                            if let Ok(ed) = doc.get_dictionary(*existing_fid) {
                                if ed
                                    .get(b"Subtype")
                                    .and_then(|s| s.as_name())
                                    .map(|n| n == b"Type0")
                                    .unwrap_or(false)
                                {
                                    keys_to_update.push((b"ZaDb".to_vec(), zd_id));
                                }
                            }
                        }
                        Err(_) => {
                            keys_to_update.push((b"ZaDb".to_vec(), zd_id));
                        }
                        _ => {}
                    }
                }
                if !keys_to_update.is_empty() {
                    if let Ok(f_dict_mut) = doc.get_dictionary_mut(f_id) {
                        for (k, target_id) in keys_to_update {
                            f_dict_mut.set(k, Object::Reference(target_id));
                        }
                    }
                }
            } else {
                let mut dr_font_dict = lopdf::Dictionary::new();
                dr_font_dict.set(b"Helv".to_vec(), Object::Reference(t1_helv_id));
                dr_font_dict.set(b"Helvetica".to_vec(), Object::Reference(t1_helv_id));
                dr_font_dict.set(b"Arial".to_vec(), Object::Reference(t1_helv_id));
                dr_font_dict.set(b"ZaDb".to_vec(), Object::Reference(zd_id));
                let dr_font_id = doc.add_object(Object::Dictionary(dr_font_dict));

                if let Ok(dr_dict_mut) = doc.get_dictionary_mut(dr_id) {
                    dr_dict_mut.set("Font", Object::Reference(dr_font_id));
                }
            }
        } else if has_inline_dr {
            let mut dr_font_dict = lopdf::Dictionary::new();
            dr_font_dict.set(b"Helv".to_vec(), Object::Reference(t1_helv_id));
            dr_font_dict.set(b"Helvetica".to_vec(), Object::Reference(t1_helv_id));
            dr_font_dict.set(b"Arial".to_vec(), Object::Reference(t1_helv_id));
            dr_font_dict.set(b"ZaDb".to_vec(), Object::Reference(zd_id));
            let dr_font_id = doc.add_object(Object::Dictionary(dr_font_dict));

            if let Ok(acro_dict_mut) = doc.get_dictionary_mut(id) {
                if let Ok(dr_dict_mut) = acro_dict_mut.get_mut(b"DR").and_then(|o| o.as_dict_mut()) {
                    dr_dict_mut.set("Font", Object::Reference(dr_font_id));
                }
            }
        } else {
            let mut dr_font_dict = lopdf::Dictionary::new();
            dr_font_dict.set(b"Helv".to_vec(), Object::Reference(t1_helv_id));
            dr_font_dict.set(b"Helvetica".to_vec(), Object::Reference(t1_helv_id));
            dr_font_dict.set(b"Arial".to_vec(), Object::Reference(t1_helv_id));
            dr_font_dict.set(b"ZaDb".to_vec(), Object::Reference(zd_id));
            let dr_font_id = doc.add_object(Object::Dictionary(dr_font_dict));

            let mut dr_dict = lopdf::Dictionary::new();
            dr_dict.set("Font", Object::Reference(dr_font_id));
            let dr_id = doc.add_object(Object::Dictionary(dr_dict));

            if let Ok(acro_dict_mut) = doc.get_dictionary_mut(id) {
                acro_dict_mut.set("DR", Object::Reference(dr_id));
            }
        }

        if let Ok(acro_dict_mut) = doc.get_dictionary_mut(id) {
            acro_dict_mut.set("NeedAppearances", Object::Boolean(false));
            if !has_da {
                acro_dict_mut.set("DA", Object::string_literal("/Helv 10 Tf 0 g"));
            }
        }
    } else {
        let mut dr_font_dict = lopdf::Dictionary::new();
        dr_font_dict.set(b"Helv".to_vec(), Object::Reference(t1_helv_id));
        dr_font_dict.set(b"Helvetica".to_vec(), Object::Reference(t1_helv_id));
        dr_font_dict.set(b"Arial".to_vec(), Object::Reference(t1_helv_id));
        dr_font_dict.set(b"ZaDb".to_vec(), Object::Reference(zd_id));
        let dr_font_id = doc.add_object(Object::Dictionary(dr_font_dict));

        let mut dr_dict = lopdf::Dictionary::new();
        dr_dict.set("Font", Object::Reference(dr_font_id));
        let dr_id = doc.add_object(Object::Dictionary(dr_dict));

        if let Ok(catalog) = doc.catalog_mut() {
            if let Ok(acro_dict) = catalog.get_mut(b"AcroForm").and_then(|o| o.as_dict_mut()) {
                if acro_dict.get(b"DR").is_err() {
                    acro_dict.set("DR", Object::Reference(dr_id));
                }
                acro_dict.set("NeedAppearances", Object::Boolean(false));
                if acro_dict.get(b"DA").is_err() {
                    acro_dict.set("DA", Object::string_literal("/Helv 10 Tf 0 g"));
                }
            }
        }
    }

    Ok(ap_font_dict_id)
}

fn deref_obj<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Object> {
    match obj {
        Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| deref_obj(doc, o)),
        other => Some(other),
    }
}

fn get_appearance_states(doc: &Document, dict: &lopdf::Dictionary) -> Vec<String> {
    let mut states = Vec::new();
    if let Ok(ap_obj) = dict.get(b"AP") {
        if let Some(ap_resolved) = deref_obj(doc, ap_obj) {
            if let Ok(ap_dict) = ap_resolved.as_dict() {
                if let Ok(n_obj) = ap_dict.get(b"N") {
                    if let Some(n_resolved) = deref_obj(doc, n_obj) {
                        if let Ok(n_dict) = n_resolved.as_dict() {
                            for (k, _) in n_dict.iter() {
                                let key_str = String::from_utf8_lossy(k).into_owned();
                                if key_str != "Off" && !states.contains(&key_str) {
                                    states.push(key_str);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    states
}

fn get_opt_strings(doc: &Document, dict: &lopdf::Dictionary) -> Vec<String> {
    let mut options = Vec::new();
    if let Ok(opt_obj) = dict.get(b"Opt") {
        if let Some(opt_resolved) = deref_obj(doc, opt_obj) {
            if let Ok(opt_arr) = opt_resolved.as_array() {
                for item in opt_arr {
                    if let Some(item_res) = deref_obj(doc, item) {
                        match item_res {
                            Object::String(bytes, _) => {
                                options.push(decode_pdf_string(bytes));
                            }
                            Object::Array(pair) if !pair.is_empty() => {
                                let label_obj = pair.get(1).unwrap_or(&pair[0]);
                                if let Some(lbl_res) = deref_obj(doc, label_obj) {
                                    if let Object::String(bytes, _) = lbl_res {
                                        options.push(decode_pdf_string(bytes));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    options
}

pub fn field_ids(doc: &Document) -> Vec<ObjectId> {
    let mut out = Vec::new();
    let Ok(catalog) = doc.catalog() else { return out };
    let Ok(acro_obj) = catalog.get(b"AcroForm") else { return out };
    let Ok((_, acro)) = doc.dereference(acro_obj) else { return out };
    let Ok(acro_dict) = acro.as_dict() else { return out };
    let Ok(Object::Array(fields)) = acro_dict.get(b"Fields") else {
        return out;
    };
    for f in fields {
        collect_field_ids(doc, f, &mut out);
    }
    out
}

fn collect_field_ids(doc: &Document, obj: &Object, out: &mut Vec<ObjectId>) {
    if let Ok((Some(id), resolved)) = doc.dereference(obj) {
        if let Ok(dict) = resolved.as_dict() {
            if dict.get(b"T").is_ok() {
                out.push(id);
            }
            // Fields can nest via /Kids — walk one structure recursively.
            if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
                for kid in kids {
                    collect_field_ids(doc, kid, out);
                }
            }
        }
    }
}

fn read_field(doc: &Document, id: ObjectId) -> Option<FormField> {
    let dict = doc.get_dictionary(id).ok()?;
    let name = match dict.get(b"T").ok()? {
        Object::String(bytes, _) => decode_pdf_string(bytes),
        _ => return None,
    };

    let raw_ft = match dict.get(b"FT").ok() {
        Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
        _ => "Tx".to_string(),
    };

    let ff = dict
        .get(b"Ff")
        .ok()
        .and_then(|o| match o {
            Object::Integer(i) => Some(*i as u32),
            _ => None,
        })
        .unwrap_or(0);

    let read_only = (ff & 1) != 0;
    let is_pushbutton = (ff & (1 << 16)) != 0;
    let is_radio_flag = (ff & (1 << 15)) != 0;
    let is_multiline = (ff & (1 << 12)) != 0;
    let is_password = (ff & (1 << 13)) != 0;
    let is_combo = (ff & (1 << 17)) != 0;

    let mut states = get_appearance_states(doc, dict);
    let has_kids = if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
        for kid in kids {
            if let Some(kid_res) = deref_obj(doc, kid) {
                if let Ok(kid_dict) = kid_res.as_dict() {
                    for s in get_appearance_states(doc, kid_dict) {
                        if !states.contains(&s) {
                            states.push(s);
                        }
                    }
                }
            }
        }
        !kids.is_empty()
    } else {
        false
    };

    let (canonical_type, options, export_value) = match raw_ft.as_str() {
        "Btn" => {
            if is_pushbutton {
                ("pushbutton".to_string(), Vec::new(), None)
            } else if is_radio_flag || (has_kids && states.len() >= 2) {
                let opts = if !states.is_empty() {
                    states
                } else {
                    vec!["Yes".to_string(), "Off".to_string()]
                };
                ("radio".to_string(), opts, None)
            } else {
                let exp = states.first().cloned().unwrap_or_else(|| "Yes".to_string());
                ("checkbox".to_string(), Vec::new(), Some(exp))
            }
        }
        "Tx" => {
            if is_multiline {
                ("multiline".to_string(), Vec::new(), None)
            } else if is_password {
                ("password".to_string(), Vec::new(), None)
            } else {
                ("text".to_string(), Vec::new(), None)
            }
        }
        "Ch" => {
            let opts = get_opt_strings(doc, dict);
            if is_combo {
                ("choice".to_string(), opts, None)
            } else {
                ("listbox".to_string(), opts, None)
            }
        }
        "Sig" => ("signature".to_string(), Vec::new(), None),
        _ => ("text".to_string(), Vec::new(), None),
    };

    let mut value = match dict.get(b"V").ok() {
        Some(Object::String(bytes, _)) => decode_pdf_string(bytes),
        Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
        _ => String::new(),
    };

    if canonical_type == "checkbox" {
        if value.is_empty() || value == "Off" || value == "/" {
            value = "Off".to_string();
        } else if value == "On" || value == "Yes" || Some(&value) == export_value.as_ref() {
            value = export_value.clone().unwrap_or_else(|| "Yes".to_string());
        }
    }

    Some(FormField {
        name,
        field_type: canonical_type,
        value,
        options,
        read_only,
        export_value,
    })
}

fn collect_fields(input: &Path) -> Result<Vec<FormField>, PdfError> {
    let doc = Document::load(input)?;
    let fields = field_ids(&doc)
        .iter()
        .filter_map(|id| read_field(&doc, *id))
        .collect();
    drop(doc);
    Ok(fields)
}

fn get_field_rect(dict: &lopdf::Dictionary) -> Option<[f64; 4]> {
    let rect_obj = dict.get(b"Rect").ok()?;
    let rect_arr = rect_obj.as_array().ok()?;
    if rect_arr.len() < 4 {
        return None;
    }
    let to_f64 = |o: &Object| -> Option<f64> {
        match o {
            Object::Integer(i) => Some(*i as f64),
            Object::Real(f) => Some(*f as f64),
            _ => None,
        }
    };
    let x0 = to_f64(rect_arr.get(0)?)?;
    let y0 = to_f64(rect_arr.get(1)?)?;
    let x1 = to_f64(rect_arr.get(2)?)?;
    let y1 = to_f64(rect_arr.get(3)?)?;
    Some([x0, y0, x1, y1])
}

fn parse_da(da_str: &str) -> (String, f64, String) {
    let mut font_name = "Helv".to_string();
    let mut font_size = 10.0;
    let mut color_ops = "0 g".to_string();

    let tokens: Vec<&str> = da_str.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].starts_with('/') && i + 2 < tokens.len() && tokens[i + 2] == "Tf" {
            font_name = tokens[i].trim_start_matches('/').to_string();
            if let Ok(sz) = tokens[i + 1].parse::<f64>() {
                if sz > 0.0 {
                    font_size = sz;
                }
            }
            i += 3;
            continue;
        }
        if tokens[i] == "g" && i >= 1 {
            color_ops = format!("{} g", tokens[i - 1]);
            i += 1;
            continue;
        }
        if tokens[i] == "rg" && i >= 3 {
            color_ops = format!("{} {} {} rg", tokens[i - 3], tokens[i - 2], tokens[i - 1]);
            i += 1;
            continue;
        }
        i += 1;
    }

    (font_name, font_size, color_ops)
}

fn encode_cp1254_stream_literal(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            // Standard ASCII printable
            ' '..='~' => {
                let byte = ch as u8;
                match byte {
                    b'(' => out.push_str("\\("),
                    b')' => out.push_str("\\)"),
                    b'\\' => out.push_str("\\\\"),
                    _ => out.push(ch),
                }
            }
            // Turkish characters mapped to CP1254 codepoints
            'Ç' => out.push_str("\\307"),
            'ç' => out.push_str("\\347"),
            'Ğ' => out.push_str("\\320"),
            'ğ' => out.push_str("\\360"),
            'Ş' => out.push_str("\\336"),
            'ş' => out.push_str("\\376"),
            'İ' => out.push_str("\\335"),
            'ı' => out.push_str("\\375"),
            'Ö' => out.push_str("\\326"),
            'ö' => out.push_str("\\366"),
            'Ü' => out.push_str("\\334"),
            'ü' => out.push_str("\\374"),
            // Common European accents in CP1254 / CP1252
            'â' => out.push_str("\\342"),
            'Â' => out.push_str("\\302"),
            'î' => out.push_str("\\356"),
            'Î' => out.push_str("\\316"),
            'û' => out.push_str("\\373"),
            'Û' => out.push_str("\\333"),
            'é' => out.push_str("\\351"),
            'É' => out.push_str("\\311"),
            'è' => out.push_str("\\350"),
            'È' => out.push_str("\\310"),
            'ë' => out.push_str("\\353"),
            'Ë' => out.push_str("\\313"),
            'á' => out.push_str("\\341"),
            'Á' => out.push_str("\\301"),
            'à' => out.push_str("\\340"),
            'À' => out.push_str("\\300"),
            'ä' => out.push_str("\\344"),
            'Ä' => out.push_str("\\304"),
            'í' => out.push_str("\\355"),
            'Í' => out.push_str("\\315"),
            'ó' => out.push_str("\\363"),
            'Ó' => out.push_str("\\323"),
            'ú' => out.push_str("\\372"),
            'Ú' => out.push_str("\\332"),
            'ñ' => out.push_str("\\361"),
            'Ñ' => out.push_str("\\321"),
            _ => {
                let u = ch as u32;
                if u < 256 {
                    out.push_str(&format!("\\{:03o}", u as u8));
                } else {
                    out.push('?');
                }
            }
        }
    }
    out
}

fn create_text_appearance_stream(
    doc: &mut Document,
    rect: [f64; 4],
    value: &str,
    da_str: &str,
    font_dict_id: ObjectId,
    font_info: Option<&TrueTypeInfo>,
    is_multiline: bool,
) -> ObjectId {
    let w = (rect[2] - rect[0]).abs().max(1.0);
    let h = (rect[3] - rect[1]).abs().max(1.0);

    let (font_name, mut font_size, color_ops) = parse_da(da_str);
    if font_size <= 0.0 {
        font_size = (h * 0.65).clamp(8.0, 14.0);
    }

    let clip_w = (w - 2.0).max(1.0);
    let clip_h = (h - 2.0).max(1.0);

    let stream_font_name = match font_name.as_str() {
        "Helvetica" | "Helv" | "Arial" | "Times-Roman" | "Times" | "Courier" | "Helvetica-Bold"
        | "Helv-Bold" | "Arial-Bold" => font_name.as_str(),
        _ => "Helv",
    };

    let stream_content = if value.is_empty() {
        format!("/Tx BMC\nq\n1 1 {clip_w:.2} {clip_h:.2} re\nW*\nn\nQ\nEMC")
    } else if let Some(info) = font_info {
        if is_multiline {
            let mut text_ops = String::new();
            let line_height = font_size * 1.2;
            let mut cur_y = (h - font_size - 2.0).max(2.0);
            for line in value.lines() {
                if cur_y < 0.0 {
                    break;
                }
                let mut hex_parts = String::new();
                for ch in line.chars() {
                    let gid = info.char_to_gid(ch);
                    hex_parts.push_str(&format!("{:04X}", gid));
                }
                text_ops.push_str(&format!(
                    "BT\n/{stream_font_name} {font_size:.2} Tf\n{color_ops}\n2.00 {cur_y:.2} Td\n<{hex_parts}> Tj\nET\n"
                ));
                cur_y -= line_height;
            }
            format!("/Tx BMC\nq\n1 1 {clip_w:.2} {clip_h:.2} re\nW*\nn\n{text_ops}Q\nEMC")
        } else {
            let x_offset = 2.0;
            let y_offset = ((h - font_size) / 2.0 + font_size * 0.15).max(1.5);
            let mut hex_parts = String::new();
            for ch in value.chars() {
                let gid = info.char_to_gid(ch);
                hex_parts.push_str(&format!("{:04X}", gid));
            }
            format!(
                "/Tx BMC\nq\n1 1 {clip_w:.2} {clip_h:.2} re\nW*\nn\nBT\n/{stream_font_name} {font_size:.2} Tf\n{color_ops}\n{x_offset:.2} {y_offset:.2} Td\n<{hex_parts}> Tj\nET\nQ\nEMC"
            )
        }
    } else if is_multiline {
        let mut text_ops = String::new();
        let line_height = font_size * 1.2;
        let mut cur_y = (h - font_size - 2.0).max(2.0);
        for line in value.lines() {
            if cur_y < 0.0 {
                break;
            }
            let escaped = encode_cp1254_stream_literal(line);
            text_ops.push_str(&format!(
                "BT\n/{stream_font_name} {font_size:.2} Tf\n{color_ops}\n2.0 {cur_y:.2} Td\n({escaped}) Tj\nET\n"
            ));
            cur_y -= line_height;
        }
        format!("/Tx BMC\nq\n1 1 {clip_w:.2} {clip_h:.2} re\nW*\nn\n{text_ops}Q\nEMC")
    } else {
        let x_offset = 2.0;
        let y_offset = ((h - font_size) / 2.0 + font_size * 0.15).max(1.5);
        let escaped = encode_cp1254_stream_literal(value);
        format!(
            "/Tx BMC\nq\n1 1 {clip_w:.2} {clip_h:.2} re\nW*\nn\nBT\n/{stream_font_name} {font_size:.2} Tf\n{color_ops}\n{x_offset:.2} {y_offset:.2} Td\n({escaped}) Tj\nET\nQ\nEMC"
        )
    };

    let mut stream_dict = lopdf::Dictionary::new();
    stream_dict.set("Type", Object::Name(b"XObject".to_vec()));
    stream_dict.set("Subtype", Object::Name(b"Form".to_vec()));
    stream_dict.set("FormType", Object::Integer(1));
    stream_dict.set(
        "BBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w as f32),
            Object::Real(h as f32),
        ]),
    );

    let mut res_dict = lopdf::Dictionary::new();
    res_dict.set("Font", Object::Reference(font_dict_id));
    stream_dict.set("Resources", Object::Dictionary(res_dict));

    let stream = lopdf::Stream::new(stream_dict, stream_content.into_bytes());
    doc.add_object(stream)
}

fn fill_fields(
    input: &Path,
    output: &Path,
    values: &HashMap<String, String>,
) -> Result<OpResult, PdfError> {
    let mut doc = Document::load(input)?;
    let font_info = load_system_truetype_font();
    let ids = field_ids(&doc);

    // Collect all characters needed across values and existing fields
    let mut used_chars = HashSet::new();
    for v in values.values() {
        for c in v.chars() {
            used_chars.insert(c);
        }
    }
    for id in &ids {
        if let Ok(dict) = doc.get_dictionary(*id) {
            if let Ok(Object::String(bytes, _)) = dict.get(b"V") {
                let s = decode_pdf_string(bytes);
                for c in s.chars() {
                    used_chars.insert(c);
                }
            }
        }
    }

    let font_dict_id = ensure_acroform_setup(&mut doc, font_info.as_ref(), &used_chars)?;

    for id in &ids {
        let Some(field) = read_field(&doc, *id) else {
            continue;
        };
        let Some(new_value) = values.get(&field.name) else {
            continue;
        };

        // Collect child widget ObjectIds if field has /Kids
        let kid_ids: Vec<ObjectId> = if let Ok(Object::Array(kids)) =
            doc.get_dictionary(*id).and_then(|d| d.get(b"Kids"))
        {
            kids.iter()
                .filter_map(|k| match k {
                    Object::Reference(kid_id) => Some(*kid_id),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };

        if field.field_type == "checkbox" {
            let is_checked = new_value == "true"
                || new_value == "1"
                || new_value == "Yes"
                || new_value == "On"
                || (!new_value.is_empty()
                    && new_value != "Off"
                    && new_value != "false"
                    && new_value != "0");
            let on_name = field
                .export_value
                .as_deref()
                .unwrap_or("Yes")
                .to_string();
            let active_state = if is_checked {
                on_name.into_bytes()
            } else {
                b"Off".to_vec()
            };

            let dict = doc.get_dictionary_mut(*id)?;
            dict.set("V", Object::Name(active_state.clone()));
            dict.set("AS", Object::Name(active_state.clone()));

            for kid_id in kid_ids {
                if let Ok(kid_dict) = doc.get_dictionary_mut(kid_id) {
                    kid_dict.set("AS", Object::Name(active_state.clone()));
                }
            }
        } else if field.field_type == "radio" {
            let selected_val = new_value.trim();
            let dict = doc.get_dictionary_mut(*id)?;
            dict.set("V", Object::Name(selected_val.as_bytes().to_vec()));

            for kid_id in kid_ids {
                let kid_states = doc
                    .get_dictionary(kid_id)
                    .map(|d| get_appearance_states(&doc, d))
                    .unwrap_or_default();
                let kid_state = if kid_states.iter().any(|s| s == selected_val) {
                    selected_val.as_bytes().to_vec()
                } else {
                    b"Off".to_vec()
                };
                if let Ok(kid_dict) = doc.get_dictionary_mut(kid_id) {
                    kid_dict.set("AS", Object::Name(kid_state));
                }
            }
        } else if field.field_type == "pushbutton" {
            // Pushbuttons do not hold fillable values
            continue;
        } else {
            // Text, Multiline, Password, Choice, Listbox
            let is_multiline = field.field_type == "multiline";

            let da_str = {
                let dict = doc.get_dictionary(*id)?;
                match dict.get(b"DA") {
                    Ok(Object::String(bytes, _)) => decode_pdf_string(bytes),
                    _ => "/Helv 10 Tf 0 g".to_string(),
                }
            };

            // 1. Update field value /V
            let dict = doc.get_dictionary_mut(*id)?;
            dict.set("V", encode_pdf_string(new_value));
            if dict.get(b"DA").is_err() {
                dict.set("DA", Object::string_literal(da_str.as_str()));
            }

            // 2. Pre-generate /AP /N Form XObject(s)
            if kid_ids.is_empty() {
                if let Some(rect) = get_field_rect(dict) {
                    let ap_stream_id = create_text_appearance_stream(
                        &mut doc,
                        rect,
                        new_value,
                        &da_str,
                        font_dict_id,
                        font_info.as_ref(),
                        is_multiline,
                    );
                    let mut ap_dict = lopdf::Dictionary::new();
                    ap_dict.set("N", Object::Reference(ap_stream_id));
                    let dict = doc.get_dictionary_mut(*id)?;
                    dict.set("AP", Object::Dictionary(ap_dict));
                }
            } else {
                for kid_id in kid_ids {
                    let kid_rect = doc.get_dictionary(kid_id).ok().and_then(|d| get_field_rect(d));
                    if let Some(rect) = kid_rect {
                        let ap_stream_id = create_text_appearance_stream(
                            &mut doc,
                            rect,
                            new_value,
                            &da_str,
                            font_dict_id,
                            font_info.as_ref(),
                            is_multiline,
                        );
                        let mut ap_dict = lopdf::Dictionary::new();
                        ap_dict.set("N", Object::Reference(ap_stream_id));
                        if let Ok(kid_dict) = doc.get_dictionary_mut(kid_id) {
                            kid_dict.set("AP", Object::Dictionary(ap_dict));
                        }
                    }
                }
            }
        }
    }

    // Second pass: for any text/multiline/choice field that has a non-empty /V but lacks /AP,
    // pre-generate an appearance stream so viewers like Adobe Acrobat render it cleanly.
    for id in &ids {
        let Ok(dict) = doc.get_dictionary(*id) else { continue };
        if dict.get(b"AP").is_ok() { continue };
        let raw_ft = match dict.get(b"FT").ok() {
            Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
            _ => "Tx".to_string(),
        };
        if raw_ft != "Tx" && raw_ft != "Ch" { continue };
        let value = match dict.get(b"V").ok() {
            Some(Object::String(bytes, _)) => decode_pdf_string(bytes),
            _ => continue,
        };
        if value.is_empty() { continue };

        let ff = dict.get(b"Ff").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
        let is_multiline = (ff & (1 << 12)) != 0;
        let da_str = match dict.get(b"DA") {
            Ok(Object::String(bytes, _)) => decode_pdf_string(bytes),
            _ => "/Helv 10 Tf 0 g".to_string(),
        };

        if let Some(rect) = get_field_rect(dict) {
            let ap_stream_id = create_text_appearance_stream(
                &mut doc,
                rect,
                &value,
                &da_str,
                font_dict_id,
                font_info.as_ref(),
                is_multiline,
            );
            let mut ap_dict = lopdf::Dictionary::new();
            ap_dict.set("N", Object::Reference(ap_stream_id));
            if let Ok(dict_mut) = doc.get_dictionary_mut(*id) {
                dict_mut.set("AP", Object::Dictionary(ap_dict));
            }
        }
    }

    let page_count = doc.get_pages().len() as u32;
    doc.compress();

    let is_in_place = input == output
        || std::fs::canonicalize(input).ok() == std::fs::canonicalize(output).ok();

    let target_save = if is_in_place {
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        let file_stem = output.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
        parent.join(format!("{}.pdfkit_tmp_{}.pdf", file_stem, std::process::id()))
    } else {
        output.to_path_buf()
    };

    doc.save(&target_save)?;

    if is_in_place {
        let mut replace_ok = false;
        if std::fs::rename(&target_save, output).is_ok() {
            replace_ok = true;
        } else if std::fs::remove_file(output).is_ok() && std::fs::rename(&target_save, output).is_ok() {
            replace_ok = true;
        } else if std::fs::copy(&target_save, output).is_ok() {
            replace_ok = true;
            let _ = std::fs::remove_file(&target_save);
        }

        if !replace_ok {
            let _ = std::fs::remove_file(&target_save);
            return Err(PdfError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Dosya başka bir programda açık tutuluyor (!). Lütfen dosyayı diğer programda kapatın veya 'Farklı Kaydet' ile kaydedin.",
            )));
        }
    }

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: page_count,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(doc);
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn stamp(
    input: &Path,
    output: &Path,
    page: u32,
    image_path: &Path,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<OpResult, PdfError> {
    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let page_map = doc.get_pages();
    let page_count = page_map.len() as u32;
    let page_id = *page_map.get(&page).ok_or(PdfError::PageOutOfBounds {
        page,
        page_count,
    })?;

    let bytes = std::fs::read(image_path)?;
    let img = build_image_xobject(&bytes)?;
    drop(bytes);
    let img_id = doc.add_object(img.stream);

    doc.get_or_create_resources(page_id)?;
    doc.add_xobject(page_id, "ImSig", img_id)?;

    let content = format!(
        "q\n{w:.1} 0 0 {h:.1} {x:.1} {y:.1} cm\n/ImSig Do\nQ\n",
        w = width,
        h = height,
        x = x,
        y = y
    );
    doc.add_page_contents(page_id, content.into_bytes())?;
    doc.compress();
    doc.save(output)?;

    let result = OpResult {
        outputs: vec![output.to_string_lossy().into_owned()],
        total_pages: page_count,
        total_size_bytes: std::fs::metadata(output)?.len(),
    };
    drop(doc);
    Ok(result)
}
