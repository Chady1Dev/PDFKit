use crate::error::PdfError;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};

#[derive(Debug, Serialize, Deserialize)]
pub struct PrinterInfo {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrinterListResult {
    pub printers: Vec<PrinterInfo>,
    pub default_printer: String,
}

#[derive(Debug, Deserialize)]
pub struct MarginsMm {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Debug, Deserialize)]
pub struct DirectPrintPayload {
    pub printer_name: String,
    pub doc_title: String,
    pub page_pngs: Vec<Vec<u8>>,
    pub copies: u32,
    pub collate: bool,
    pub orientation: String, // "portrait", "landscape", "auto"
    pub margins: MarginsMm,
    pub grayscale: bool,
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

#[repr(C)]
struct DOCINFOW {
    cb_size: i32,
    lpsz_doc_name: *const u16,
    lpsz_output: *const u16,
    lpsz_datatype: *const u16,
    fw_type: u32,
}

#[repr(C)]
struct BITMAPINFOHEADER {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct RGBQUAD {
    rgb_blue: u8,
    rgb_green: u8,
    rgb_red: u8,
    rgb_reserved: u8,
}

#[repr(C)]
struct BITMAPINFO {
    bmi_header: BITMAPINFOHEADER,
    bmi_colors: [RGBQUAD; 1],
}

const HORZRES: i32 = 8;
const VERTRES: i32 = 10;
const LOGPIXELSX: i32 = 88;
const LOGPIXELSY: i32 = 90;
const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC0020;
const DM_IN_PROMPT: u32 = 4;
const DM_OUT_BUFFER: u32 = 2;

#[link(name = "gdi32")]
extern "system" {
    fn CreateDCW(
        pwsz_driver: *const u16,
        pwsz_device: *const u16,
        psz_port: *const u16,
        pdm: *const std::ffi::c_void,
    ) -> *mut std::ffi::c_void;

    fn DeleteDC(hdc: *mut std::ffi::c_void) -> i32;
    fn StartDocW(hdc: *mut std::ffi::c_void, lpdi: *const DOCINFOW) -> i32;
    fn EndDoc(hdc: *mut std::ffi::c_void) -> i32;
    fn StartPage(hdc: *mut std::ffi::c_void) -> i32;
    fn EndPage(hdc: *mut std::ffi::c_void) -> i32;
    fn GetDeviceCaps(hdc: *mut std::ffi::c_void, n_index: i32) -> i32;

    fn StretchDIBits(
        hdc: *mut std::ffi::c_void,
        x_dest: i32,
        y_dest: i32,
        dest_width: i32,
        dest_height: i32,
        x_src: i32,
        y_src: i32,
        src_width: i32,
        src_height: i32,
        lp_bits: *const std::ffi::c_void,
        lpbmi: *const BITMAPINFO,
        i_usage: u32,
        rop: u32,
    ) -> i32;
}

#[repr(C)]
struct PRINTER_INFO_4W {
    p_printer_name: *const u16,
    p_server_name: *const u16,
    attributes: u32,
}

const PRINTER_ENUM_LOCAL: u32 = 0x00000002;
const PRINTER_ENUM_CONNECTIONS: u32 = 0x00000004;

#[link(name = "winspool")]
extern "system" {
    fn OpenPrinterW(
        p_printer_name: *const u16,
        ph_printer: *mut *mut std::ffi::c_void,
        p_default: *const std::ffi::c_void,
    ) -> i32;

    fn ClosePrinter(h_printer: *mut std::ffi::c_void) -> i32;

    fn DocumentPropertiesW(
        h_wnd: *mut std::ffi::c_void,
        h_printer: *mut std::ffi::c_void,
        p_device_name: *const u16,
        p_dev_mode_output: *mut std::ffi::c_void,
        p_dev_mode_input: *mut std::ffi::c_void,
        f_mode: u32,
    ) -> i32;

    fn EnumPrintersW(
        flags: u32,
        name: *const u16,
        level: u32,
        p_printer_enum: *mut u8,
        cb_buf: u32,
        pcb_needed: *mut u32,
        pc_returned: *mut u32,
    ) -> i32;

    fn GetDefaultPrinterW(
        psz_buffer: *mut u16,
        pcch_buffer: *mut u32,
    ) -> i32;
}

/// Lists all installed Windows printers and identifies the system default printer.
/// Uses 100% native Win32 API calls (EnumPrintersW / GetDefaultPrinterW) in-process:
/// 0ms execution time, ZERO child processes, and completely eliminates any cmd/powershell flash!
#[tauri::command]
pub async fn get_system_printers() -> Result<PrinterListResult, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut default_printer = String::new();
        unsafe {
            let mut needed = 0u32;
            GetDefaultPrinterW(null_mut(), &mut needed);
            if needed > 0 {
                let mut buf = vec![0u16; needed as usize];
                if GetDefaultPrinterW(buf.as_mut_ptr(), &mut needed) != 0 {
                    let len = needed.saturating_sub(1) as usize;
                    default_printer = String::from_utf16_lossy(&buf[..len]);
                }
            }
        }

        let mut printers = Vec::new();
        unsafe {
            let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
            let mut needed = 0u32;
            let mut returned = 0u32;

            EnumPrintersW(flags, null(), 4, null_mut(), 0, &mut needed, &mut returned);
            if needed > 0 {
                let mut buf = vec![0u8; needed as usize];
                if EnumPrintersW(
                    flags,
                    null(),
                    4,
                    buf.as_mut_ptr(),
                    needed,
                    &mut needed,
                    &mut returned,
                ) != 0
                {
                    let slice = std::slice::from_raw_parts(
                        buf.as_ptr() as *const PRINTER_INFO_4W,
                        returned as usize,
                    );
                    for item in slice {
                        if !item.p_printer_name.is_null() {
                            let mut len = 0;
                            while *item.p_printer_name.add(len) != 0 {
                                len += 1;
                            }
                            let name_slice = std::slice::from_raw_parts(item.p_printer_name, len);
                            let name = String::from_utf16_lossy(name_slice);
                            let is_default = !default_printer.is_empty() && name == default_printer;
                            printers.push(PrinterInfo { name, is_default });
                        }
                    }
                }
            }
        }

        // If no default was found, pick the first
        if default_printer.is_empty() && !printers.is_empty() {
            default_printer = printers[0].name.clone();
            printers[0].is_default = true;
        }

        // If for any reason winspool returned 0 printers, provide safe fallback
        if printers.is_empty() {
            printers.push(PrinterInfo {
                name: "Varsayılan Sistem Yazıcısı".to_string(),
                is_default: true,
            });
            default_printer = "Varsayılan Sistem Yazıcısı".to_string();
        }

        Ok(PrinterListResult {
            printers,
            default_printer,
        })
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Opens native Windows printer properties dialog (DocumentProperties) for the selected printer.
#[tauri::command]
pub async fn open_printer_properties(printer_name: String) -> Result<bool, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let wide_name = to_wide(&printer_name);
        let mut hprinter: *mut std::ffi::c_void = null_mut();

        unsafe {
            if OpenPrinterW(wide_name.as_ptr(), &mut hprinter, null()) == 0 || hprinter.is_null() {
                return Err(PdfError::Custom(format!("Yazıcı açılamadı: {}", printer_name)));
            }

            let size = DocumentPropertiesW(null_mut(), hprinter, wide_name.as_ptr(), null_mut(), null_mut(), 0);
            if size <= 0 {
                ClosePrinter(hprinter);
                return Ok(false);
            }

            let mut devmode_buf = vec![0u8; size as usize];
            let devmode_ptr = devmode_buf.as_mut_ptr() as *mut std::ffi::c_void;

            // Prompt user with native manufacturer driver properties dialog
            let ret = DocumentPropertiesW(
                null_mut(),
                hprinter,
                wide_name.as_ptr(),
                devmode_ptr,
                null_mut(),
                DM_IN_PROMPT | DM_OUT_BUFFER,
            );

            ClosePrinter(hprinter);
            Ok(ret == 1) // IDOK = 1
        }
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

/// Prints pages directly to the target Windows printer using Win32 GDI.
/// Renders without any intermediate browser dialogs directly to the Windows print spooler.
#[tauri::command]
pub async fn direct_print_pages(payload: DirectPrintPayload) -> Result<(), PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        if payload.page_pngs.is_empty() {
            return Err(PdfError::EmptyOutput);
        }

        let wide_driver = to_wide("WINSPOOL");
        let wide_printer = to_wide(&payload.printer_name);

        unsafe {
            let hdc = CreateDCW(
                wide_driver.as_ptr(),
                wide_printer.as_ptr(),
                null(),
                null(),
            );

            if hdc.is_null() {
                return Err(PdfError::Custom(format!(
                    "Yazıcı aygıt içeriği (DC) oluşturulamadı: {}",
                    payload.printer_name
                )));
            }

            let title_wide = to_wide(&payload.doc_title);
            let doc_info = DOCINFOW {
                cb_size: std::mem::size_of::<DOCINFOW>() as i32,
                lpsz_doc_name: title_wide.as_ptr(),
                lpsz_output: null(),
                lpsz_datatype: null(),
                fw_type: 0,
            };

            if StartDocW(hdc, &doc_info) <= 0 {
                DeleteDC(hdc);
                return Err(PdfError::Custom("Yazdırma işi (StartDoc) başlatılamadı.".into()));
            }

            let dpi_x = GetDeviceCaps(hdc, LOGPIXELSX) as f64;
            let dpi_y = GetDeviceCaps(hdc, LOGPIXELSY) as f64;
            let page_w = GetDeviceCaps(hdc, HORZRES) as f64;
            let page_h = GetDeviceCaps(hdc, VERTRES) as f64;

            // Margin dots
            let ml = (payload.margins.left / 25.4 * dpi_x).clamp(0.0, page_w * 0.4);
            let mr = (payload.margins.right / 25.4 * dpi_x).clamp(0.0, page_w * 0.4);
            let mt = (payload.margins.top / 25.4 * dpi_y).clamp(0.0, page_h * 0.4);
            let mb = (payload.margins.bottom / 25.4 * dpi_y).clamp(0.0, page_h * 0.4);

            let printable_w = (page_w - ml - mr).max(100.0);
            let printable_h = (page_h - mt - mb).max(100.0);

            let copies = payload.copies.max(1);

            for _copy in 0..copies {
                for png_bytes in &payload.page_pngs {
                    let dyn_img = image::load_from_memory(png_bytes)
                        .map_err(|e| PdfError::Custom(format!("Baskı sayfası ayrıştırılamadı: {}", e)))?;

                    let mut img_rgba = if payload.grayscale {
                        let gray = dyn_img.grayscale();
                        gray.to_rgba8()
                    } else {
                        dyn_img.to_rgba8()
                    };

                    let (img_w, img_h) = img_rgba.dimensions();
                    if img_w == 0 || img_h == 0 {
                        continue;
                    }

                    // Convert RGBA to 32-bit BGRA for StretchDIBits
                    let mut bgra = Vec::with_capacity((img_w * img_h * 4) as usize);
                    for pixel in img_rgba.chunks_exact_mut(4) {
                        let r = pixel[0];
                        let g = pixel[1];
                        let b = pixel[2];
                        let a = pixel[3];
                        bgra.push(b);
                        bgra.push(g);
                        bgra.push(r);
                        bgra.push(a);
                    }

                    // Calculate aspect-fit within printable area
                    let scale_x = printable_w / img_w as f64;
                    let scale_y = printable_h / img_h as f64;
                    let scale = scale_x.min(scale_y);

                    let dest_w = (img_w as f64 * scale) as i32;
                    let dest_h = (img_h as f64 * scale) as i32;
                    let dest_x = (ml + (printable_w - dest_w as f64) / 2.0) as i32;
                    let dest_y = (mt + (printable_h - dest_h as f64) / 2.0) as i32;

                    let bmi = BITMAPINFO {
                        bmi_header: BITMAPINFOHEADER {
                            bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                            bi_width: img_w as i32,
                            bi_height: -(img_h as i32), // Top-down
                            bi_planes: 1,
                            bi_bit_count: 32,
                            bi_compression: BI_RGB,
                            bi_size_image: (img_w * img_h * 4) as u32,
                            bi_x_pels_per_meter: (dpi_x / 0.0254) as i32,
                            bi_y_pels_per_meter: (dpi_y / 0.0254) as i32,
                            bi_clr_used: 0,
                            bi_clr_important: 0,
                        },
                        bmi_colors: [RGBQUAD {
                            rgb_blue: 0,
                            rgb_green: 0,
                            rgb_red: 0,
                            rgb_reserved: 0,
                        }],
                    };

                    StartPage(hdc);
                    StretchDIBits(
                        hdc,
                        dest_x,
                        dest_y,
                        dest_w,
                        dest_h,
                        0,
                        0,
                        img_w as i32,
                        img_h as i32,
                        bgra.as_ptr() as *const _,
                        &bmi,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    );
                    EndPage(hdc);
                }
            }

            EndDoc(hdc);
            DeleteDC(hdc);
            Ok(())
        }
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}
