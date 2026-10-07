//! PDFKit core library.
//!
//! Local-first PDF toolbox. All processing happens in-process via `lopdf`;
//! no network access, no external runtimes. Every command loads documents,
//! performs the operation, and drops them immediately so memory is released
//! before the IPC response is sent.

pub mod commands;
mod error;
mod pdfutil;
pub mod types;

use tauri::{Emitter, Manager};

pub use error::PdfError;

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn get_cli_args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let _ = app.emit("open-files", &args);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            exit_app,
            get_cli_args,
            // Phase 1 — core
            commands::document::get_pdf_info,
            commands::document::reveal_in_explorer,
            commands::merge::merge_pdfs,
            commands::split::split_pdf,
            commands::rotate::rotate_pages,
            commands::organize::organize_pdf,
            commands::images::images_to_pdf,
            commands::images::write_binary_file,
            commands::compress::compress_pdf,
            // Phase 2 — advanced
            commands::security::encrypt_pdf,
            commands::security::decrypt_pdf,
            commands::annotate::add_watermark,
            commands::annotate::remove_watermark,
            commands::annotate::remove_watermark_bytes,
            commands::annotate::write_pdf_bytes,
            commands::annotate::add_page_numbers,
            commands::annotate::stamp_page_overlays,
            commands::annotate::load_editor_document,
            commands::printer::get_system_printers,
            commands::printer::open_printer_properties,
            commands::printer::direct_print_pages,
            commands::forms::list_form_fields,
            commands::forms::fill_form,
            commands::forms::stamp_image,
            commands::convert_office::export_office_document,
            commands::convert_office::cancel_conversion,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PDFKit");
}
