//! Office Document Conversion Command Facade
//! Routes export requests to format-specific conversion modules (DOCX, PPTX, XLSX, TXT).

use std::path::Path;
use std::sync::atomic::Ordering;

use crate::error::PdfError;
use super::conversion::{self, CANCELLATION_REQUESTED};

pub use super::conversion::{ConversionProgress, PageTextData};

#[tauri::command]
pub fn cancel_conversion() -> Result<(), PdfError> {
    conversion::cancel_conversion()
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
    let in_path_ref = input_path.as_deref();

    match fmt.as_str() {
        "docx" => conversion::docx::convert(&app, in_path_ref, path, &doc_title, &pages, lang),
        "pptx" => conversion::pptx::convert(&app, in_path_ref, path, &pages, lang),
        "xlsx" => conversion::xlsx::convert(&app, in_path_ref, path, &pages),
        "txt" => conversion::txt::convert(&app, in_path_ref, path, &pages),
        other => Err(PdfError::Custom(format!("Desteklenmeyen dosya formatı: {}", other))),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PythonEnvStatus {
    pub installed: bool,
    pub command: Option<String>,
    pub version: Option<String>,
    pub packages_ready: bool,
    pub missing_packages: Vec<String>,
}

#[tauri::command]
pub async fn check_python_environment() -> Result<PythonEnvStatus, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let python_candidates = conversion::get_python_candidates();
        let probe_code = r#"import sys, importlib.util as u; print("VER:" + sys.version.split()[0]); pkgs = ['pymupdf', 'pdf2docx', 'docx', 'pptx', 'xlsxwriter']; missing = [p for p in pkgs if u.find_spec(p) is None]; print("MISSING:" + ",".join(missing) if missing else "ALL_OK")"#;

        let mut fallback_installed: Option<PythonEnvStatus> = None;

        for cmd_name in &python_candidates {
            let mut cmd = std::process::Command::new(cmd_name);
            cmd.arg("-c").arg(probe_code);

            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let mut version = None;
                    let mut packages_ready = false;
                    let mut missing_packages = Vec::new();

                    for line in text.lines() {
                        let line = line.trim();
                        if let Some(v) = line.strip_prefix("VER:") {
                            version = Some(v.to_string());
                        } else if line == "ALL_OK" {
                            packages_ready = true;
                        } else if let Some(m) = line.strip_prefix("MISSING:") {
                            missing_packages = m
                                .split(',')
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                                .collect();
                        }
                    }

                    if version.is_some() {
                        let status = PythonEnvStatus {
                            installed: true,
                            command: Some(cmd_name.to_string()),
                            version,
                            packages_ready,
                            missing_packages,
                        };
                        if packages_ready {
                            return status;
                        }
                        if fallback_installed.is_none() {
                            fallback_installed = Some(status);
                        }
                    }
                }
            }
        }

        if let Some(installed_status) = fallback_installed {
            return installed_status;
        }

        PythonEnvStatus {
            installed: false,
            command: None,
            version: None,
            packages_ready: false,
            missing_packages: vec![
                "pymupdf".to_string(),
                "pdf2docx".to_string(),
                "docx".to_string(),
                "pptx".to_string(),
                "xlsxwriter".to_string(),
            ],
        }
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let clean_url = url.trim();
        let has_unsafe_chars = clean_url.chars().any(|c| {
            matches!(
                c,
                '\r' | '\n' | '\0' | '"' | '\'' | '&' | '|' | ';' | '`' | '$' | '<' | '>' | '^' | '%' | ' ' | '\t'
            )
        });

        if !has_unsafe_chars
            && (clean_url.starts_with("https://www.python.org/")
                || clean_url.starts_with("https://github.com/"))
        {
            let mut cmd = Command::new("rundll32");
            cmd.args(["url.dll,FileProtocolHandler", clean_url]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.spawn().map_err(|e| e.to_string())?;
            return Ok(());
        }
        Err("Güvenlik kısıtlaması: Yalnızca doğrulanmış resmi bağlantılar açılabilir.".into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = url;
        Ok(())
    }
}

#[tauri::command]
pub async fn install_python_runtime() -> Result<String, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            let mut cmd = std::process::Command::new("winget");
            cmd.args([
                "install",
                "-e",
                "--id",
                "Python.Python.3.13",
                "--scope",
                "user",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);

            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    return Ok("Python başarıyla indirildi ve kuruldu!".to_string());
                }
            }

            // Eğer sistemde zaten Python varsa veya winget 'already installed' döndürdüyse doğrula
            for candidate in conversion::get_python_candidates() {
                let mut check_cmd = std::process::Command::new(&candidate);
                check_cmd.arg("--version");
                check_cmd.creation_flags(CREATE_NO_WINDOW);
                if let Ok(out) = check_cmd.output() {
                    if out.status.success() {
                        return Ok("Python sistemde hazır!".to_string());
                    }
                }
            }
        }

        Err(PdfError::Custom(
            "Otomatik Python kurulumu başlatılamadı. Lütfen 'python.org'dan İndir' butonunu kullanarak kurun.".into(),
        ))
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

#[tauri::command]
pub async fn install_python_packages() -> Result<String, PdfError> {
    tauri::async_runtime::spawn_blocking(move || {
        let python_candidates = conversion::get_python_candidates();
        let packages = ["pymupdf", "pdf2docx", "python-docx", "python-pptx", "xlsxwriter"];

        for cmd_name in &python_candidates {
            // 1. Standart kurulum denemesi
            let mut cmd = std::process::Command::new(cmd_name);
            cmd.arg("-m")
                .arg("pip")
                .arg("install")
                .arg("--disable-pip-version-check")
                .args(packages);

            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    return Ok("Dönüştürme kütüphaneleri başarıyla kuruldu!".to_string());
                }

                // 2. Eğer Python 'C:\Program Files' altına (All Users) kurulmuşsa ve yönetici izni yoksa '--user' ile kullanıcı profiline kur
                let mut user_cmd = std::process::Command::new(cmd_name);
                user_cmd
                    .arg("-m")
                    .arg("pip")
                    .arg("install")
                    .arg("--user")
                    .arg("--disable-pip-version-check")
                    .args(packages);

                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    user_cmd.creation_flags(CREATE_NO_WINDOW);
                }

                if let Ok(user_out) = user_cmd.output() {
                    if user_out.status.success() {
                        return Ok("Dönüştürme kütüphaneleri başarıyla kuruldu!".to_string());
                    }
                }
            }
        }
        Err(PdfError::Custom(
            "Otomatik paket kurulumu tamamlanamadı. Lütfen internet bağlantınızı kontrol edin.".into(),
        ))
    })
    .await
    .map_err(|e| PdfError::TaskJoin(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
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
        let result = conversion::docx::generate_docx(&docx_path, "Test Faturası", &pages, "tr");
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
        let result = conversion::xlsx::generate_xlsx(&xlsx_path, &pages);
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
        let result = conversion::pptx::generate_pptx(&pptx_path, &pages, "tr");
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
        let result = conversion::txt::generate_txt(&txt_path, &pages);
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
