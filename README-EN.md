<div align="center">

<p align="center">
  <img src="public/logo.png" width="130" alt="PDFkit Logo" />
</p>

# PDFkit
### Ultra-Lightweight, Independent, Zero-Network Desktop PDF Workstation

<p align="center">
  <a href="README.md">Türkçe</a> &nbsp;•&nbsp; <b>English</b>
</p>

<br/>

> **"For the best results and improved document conversion, please ensure you are using the latest version of the application. Our PDF to Word, Excel, and PowerPoint conversion engine will be updated regularly."**

<br/>

[📥 Windows Installer (.exe)](https://github.com/Chady1Dev/PDFKit/releases/download/v1.0.1/PDFkit_0.1.0_x64-setup.exe) • 
[🏢 Enterprise Deployment Package (.msi)](https://github.com/Chady1Dev/PDFKit/releases/download/v1.0.1/PDFkit_0.1.0_x64_tr-TR.msi) • 
[📖 Technical Documentation](DOCUMENTATION.md)

</div>

---

> 📌 **Important Note:** For optimal results, please ensure you are using the latest version of the application. Our PDF to Word, Excel, and PowerPoint conversion engine is continuously updated and improved.

---

## 💡 Why PDFkit?

In everyday document workflows, users are typically forced to choose between two compromises:

1. **Online PDF Tools (iLovePDF, Smallpdf, etc.):** Files are uploaded to third-party servers. Personal data, trade secrets, or exam papers leave your computer—introducing severe risks under **KVKK and GDPR**. They also impose file limits, queues, and paid subscription paywalls.
2. **Traditional Desktop Giants (Adobe Acrobat Pro, etc.):** Take up hundreds of megabytes of disk space, run dozens of background telemetry and update daemons, and demand annual recurring license fees.

**PDFkit** is a **completely local, secure, independent, and free** PDF workstation that lets you edit, organize pages, remove watermarks intelligently, redact content physically, and convert documents to Office formats—without files ever leaving your machine.

---

## ⚡ Comparison

### Overview

| | **PDFkit** | **Adobe Acrobat Reader** | **LibreOffice Draw** |
| :--- | :---: | :---: | :---: |
| **License** | Free & open source | Free, proprietary | Free & open source |
| **Platform** | Windows | Windows, macOS, Mobile | Windows, macOS, Linux |
| **Installed Size** | **14 MB** | ~1.1 GB | ~340 MB |
| **PDF Edit - RAM Usage** | **~300 MB** | ~620 MB | ~ Froze / Not Responding |
| **Account / Network** | Not required *(100% Offline)* | Some features require login | Not required |

### What Can I Do?

| Action | **PDFkit** | **Adobe Acrobat Reader** | **LibreOffice Draw** |
| :--- | :---: | :---: | :---: |
| Annotate, highlight, sign | ✅ | ✅ | ✅ |
| Form filling | ✅ | ✅ | ⚠️ Limited |
| Merge / split | ✅ | ❌ Paid version only | ⚠️ Manual |
| Page reorder, rotate, delete | ✅ | ❌ Paid version only | ✅ |
| Encryption and permissions | ✅ | ❌ Paid version only | ✅ On export |
| Compression | ✅ | ❌ Paid version only | ⚠️ On export |
| True physical redaction | ✅ | ❌ Paid version only | ⚠️ Flattens entire doc to raster images |
| Watermark removal | ✅ | ❌ Paid version only | ❌ |
| PDF → Word / Excel / PowerPoint | ✅ *(Python required for Word and PPTx)* | ❌ Paid version only | ❌ |
| Document layout preservation | ✅ | ✅ | ⚠️ Opens as vector drawing, layout may drift |

---

## 🎬 Demo Video

https://github.com/user-attachments/assets/a4c56783-6342-4612-a137-90294922c5ff

---

## ✨ Key Features

### 🛡️ 1. Zero-Network Security Model
The application is compiled with a strict Content Security Policy (CSP). It cannot initiate external HTTP/WebSocket requests, and contains zero telemetry, analytics, or hidden background services.

### 🪓 2. Permanent and Physical Redaction
Conventional tools merely draw a black rectangle over sensitive text; the underlying text can still be selected and copied. In PDFkit, when saving a redacted page, the **original text, font glyphs, and character bytecode are permanently stripped from the file stream.** The information can never be recovered.

### 🎯 3. Intelligent Watermark Removal
Removes intrusive diagonal or transparent watermarks from university course notes, open university exam papers, and official draft documents in a single click without disturbing content flow. Header, body, and footer zones can be targeted independently.

### 📄 4. Office Conversion (Word / PowerPoint / Excel / TXT)
Dual-column text flow heuristics, advanced table reconstruction, and full Unicode Turkish character support. Documents with complex tables are parsed into Word or Excel in seconds instead of minutes.

### 🖋️ 5. Practical Editing & Page Tools
- **Page Management:** Drag-and-drop page reordering, rotating, deleting, splitting, and merging.
- **Content Editing:** Add text, shapes, highlights, and handwritten signatures.
- **Form Filling:** Detect AcroForm fields and fill them out with full Turkish character support.
- **Security:** AES-256 encryption; restrict printing, copying, and modification permissions.
- **Compression:** Lossless and balanced compression profiles.
- **Direct Printing:** Hardware-accelerated direct communication with Windows printer drivers.

---

## 📦 Optional Python Runtime for Office Conversion

All core features of PDFkit (editing, redaction, page management, watermarking, forms) work **with zero additional setup or dependencies.** Having Python installed is recommended for high-fidelity Word and PowerPoint export.

---

## 🏢 Enterprise Deployment (IT & Sysadmins)

PDFkit is delivered as an `.msi` package ready for centralized deployment via Active Directory (GPO), Microsoft Intune, SCCM, or PDQ Deploy.

```powershell
# Silent unattended installation
msiexec.exe /i "PDFkit_0.1.0_x64_tr-TR.msi" /qn /norestart

# Silent uninstall
msiexec.exe /x "PDFkit_0.1.0_x64_tr-TR.msi" /qn /norestart
```
For detailed guidance, refer to [DOCUMENTATION.md](DOCUMENTATION.md).

> 📌 **Important Note:** For optimal results, please ensure you are using the latest version of the application. Our PDF to Word, Excel, and PowerPoint conversion engine is continuously updated and improved.
---

## 🔒 Windows SmartScreen Notice

> PDFkit is an independent and open-source project. Because it is not packaged with an expensive corporate code-signing certificate, Windows SmartScreen may show an *"Unknown Publisher"* warning on first launch.
>
> The application contains zero telemetry and the entire source code is open for inspection. You can proceed safely by clicking **"More info" → "Run anyway"**.

---

## 📄 License
This project is licensed under the **[GNU Affero General Public License v3.0 (AGPL-3.0)](LICENSE)**.

Copyright (c) 2026 Çağdaş Polat ([github.com/Chady1Dev](https://github.com/Chady1Dev))

* Individuals and organizations may use PDFkit for commercial or internal workflows **completely free and without restriction**.
* For further details, please review the [LICENSE](LICENSE) file.

---

<div align="center">
Developer: <b>Çağdaş Polat</b> • Systems & Infrastructure Engineering<br/>
<i>"Lighter, safer, and smarter solutions for modern organizations."</i>
</div>
