//! PDFKit Modular Conversion Architecture
//! Common types, cancellation tracking, XML escaping, and Python process execution.

pub mod docx;
pub mod pptx;
pub mod xlsx;
pub mod txt;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use crate::error::PdfError;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PageTextData {
    pub page: u32,
    pub text: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversionProgress {
    pub current: u32,
    pub total: u32,
    pub percent: u32,
    pub stage: String,
}

pub static ACTIVE_CONVERSION_PID: AtomicU32 = AtomicU32::new(0);
pub static CANCELLATION_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Cancels any currently executing conversion subprocess.
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

/// Helper function to escape XML special characters and strip illegal XML 1.0 control codes.
pub fn escape_xml(input: &str) -> String {
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

/// Locates the Python conversion entrypoint script.
pub fn find_python_script(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;

    // 1. Check in packaged resource directory
    if let Ok(res_dir) = app.path().resource_dir() {
        let candidates = [
            res_dir.join("scripts").join("python_engine").join("dispatcher.py"),
            res_dir.join("python_engine").join("dispatcher.py"),
            res_dir.join("scripts").join("pdf2docx_engine.py"),
            res_dir.join("pdf2docx_engine.py"),
        ];
        for c in &candidates {
            if c.exists() {
                return Ok(c.clone());
            }
        }
    }

    // 2. Check relative to current executable
    if let Ok(exe_dir) = std::env::current_exe().map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf()) {
        let candidates = [
            exe_dir.join("scripts").join("python_engine").join("dispatcher.py"),
            exe_dir.join("python_engine").join("dispatcher.py"),
            exe_dir.join("scripts").join("pdf2docx_engine.py"),
            exe_dir.join("pdf2docx_engine.py"),
            exe_dir.join("..").join("..").join("python_engine").join("dispatcher.py"),
            exe_dir.join("..").join("..").join("src-tauri").join("scripts").join("pdf2docx_engine.py"),
            exe_dir.join("..").join("src-tauri").join("scripts").join("pdf2docx_engine.py"),
        ];
        for c in &candidates {
            if c.exists() {
                return Ok(c.clone());
            }
        }
    }

    // 3. Fallback dev paths
    let dev_candidates = [
        "python_engine/dispatcher.py",
        "src-tauri/scripts/python_engine/dispatcher.py",
        "src-tauri/scripts/pdf2docx_engine.py",
        "scripts/pdf2docx_engine.py",
        "../python_engine/dispatcher.py",
        "../src-tauri/scripts/pdf2docx_engine.py",
    ];
    for p in &dev_candidates {
        if Path::new(p).exists() {
            return Ok(PathBuf::from(p));
        }
    }

    Err("Dönüştürme modülü scripti bulunamadı".to_string())
}

/// Executes the isolated Python engine for the requested target format.
pub fn run_python_engine(
    app: &tauri::AppHandle,
    input_pdf: &str,
    output_path: &str,
    format: &str,
) -> Result<(), String> {
    use tauri::Emitter;
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    CANCELLATION_REQUESTED.store(false, Ordering::SeqCst);

    // Verify input file access and check sharing locks
    if let Err(e) = std::fs::File::open(input_pdf) {
        #[cfg(target_os = "windows")]
        if e.raw_os_error() == Some(32) {
            return Err("Dosya başka bir programda açık (!)".to_string());
        }
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err("Dosya başka bir programda açık (!)".to_string());
        }
    }

    // Verify output destination writeability
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

    let script = find_python_script(app)?;
    let python_candidates = ["python", "py", "python3"];
    let mut last_error_message = None;

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
                } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                    if val.get("type").and_then(|t| t.as_str()) == Some("error") {
                        if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                            last_error_message = Some(msg.to_string());
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
            _ => {}
        }
    }

    if CANCELLATION_REQUESTED.load(Ordering::SeqCst) {
        if Path::new(output_path).exists() {
            let _ = std::fs::remove_file(output_path);
        }
        return Err("Dönüştürme işlemi iptal edildi.".to_string());
    }

    if let Some(err) = last_error_message {
        return Err(err);
    }

    Err("Gelişmiş motor tamamlanamadı.".to_string())
}
