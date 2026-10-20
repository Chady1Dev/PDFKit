<div align="center">

<p align="center">
  <img src="public/logo.png" width="130" alt="PDFkit Logo" />
</p>

# PDFkit
### Ultra-Lightweight, Local-First, Zero-Network PDF Workstation for Windows

<p align="center">
  <a href="README.md">Türkçe</a> &nbsp;•&nbsp; <b>English</b>
</p>

[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011%20(x64)-0078D6?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2.0-FFC131?style=for-the-badge&logo=tauri&logoColor=black)](https://tauri.app/)
[![Rust Core](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?style=for-the-badge&logo=svelte&logoColor=white)](https://svelte.dev/)
[![Zero Network](https://img.shields.io/badge/Security-Zero--Network%20(Offline)-2ea44f?style=for-the-badge&logo=shield&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![Corporate Deployment](https://img.shields.io/badge/Enterprise-MSI%20%26%20GPO%20Ready-blueviolet?style=for-the-badge&logo=windows-terminal&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![License: AGPL v3](https://img.shields.io/badge/License-GNU%20AGPL%20v3.0-blue?style=for-the-badge&logo=gnu&logoColor=white)](LICENSE)

<br/>

> **"Your documents never leave your machine. No subscription, no telemetry, in a 4.15 MB ultra-compact installer."**

<br/>

[📥 Windows Installer (.exe)](https://github.com/cagdass1128/PDFKit/releases) • 
[🏢 Enterprise Package (.msi)](https://github.com/cagdass1128/PDFKit/releases) • 
[📖 Technical Documentation](DOCUMENTATION.md)

</div>

---

> 📌 **Important Note:** For optimal results, please ensure you are using the latest version. Our PDF to Word, Excel, and PowerPoint conversion engine is continuously updated and improved.

---

## 💡 Why PDFkit?

Modern knowledge workers and organizations are typically forced to choose between two problematic options:

1. **Online Web Converters (iLovePDF, Smallpdf, etc.):** Files are uploaded to third-party servers. Sensitive contracts, intellectual property, or exam papers leak outside your perimeter—introducing severe **GDPR and data compliance risks**, alongside file size quotas and subscription walls.
2. **Traditional Desktop Giants (Adobe Acrobat Pro, Foxit, etc.):** Bulky 500 MB+ installations, dozens of background telemetry and update services, high idle memory consumption, and costly recurring licenses.

**PDFkit** is a private, lightweight, and independent PDF workstation built on **Tauri v2 + Rust**. It lets you organize pages, purge watermarks, redact sensitive data down to the byte level, and convert documents to Office formats—**100% offline and free forever.**

---

## ⚡ Comparison

| Feature / Metric | 📑 **PDFkit** | 🔴 **Adobe Acrobat Pro** | 📄 **LibreOffice (Draw)** |
| :--- | :---: | :---: | :---: |
| **Cost / License** | **100% Free & Open Source** | **Paid Subscription** | Free & Open Source |
| **Installer Size** | **`4.15 MB`** *(Instant setup)* | ~`350 – 500 MB` | ~`300 – 450 MB` |
| **Startup Time** | **`< 0.3 sec`** *(Instantaneous)* | 2.5 – 5.0 sec | 3.0 – 6.0 sec |
| **Idle RAM Usage** | **`~35 MB`** *(Rust core)* | 350 – 500 MB+ | 250 – 400 MB |
| **Privacy & Network** | **100% Offline (0 KB Data)** | Background cloud telemetry | Local (optional telemetry) |
| **Page Organization** | Reorder, merge, split, rotate, delete | Available | Breaks layouts into vector shapes |
| **Physical Redaction** | **Yes** *(Text bytecode permanently removed)* | Available | No *(Overlay boxes can be deleted)* |
| **Smart Watermark Removal** | **One-Click Zone Analysis (Header/Body/Footer)** | No *(Tedious manual editing)* | No *(Each letter becomes a shape)* |
| **Enterprise Deployment** | **`5.78 MB MSI`** ready for GPO | Complex bulky MSI transforms | Large MSI package |
| **Windows Optimization** | Deep native integration with Windows Print Spooler and GDI | Cross-platform bloat | Heavy VCL runtime overhead |

---

## ✨ Key Features

### 🛡️ 1. Zero-Network Security Model
Compiled with a strict Content Security Policy (CSP). PDFkit cannot initiate HTTP or WebSocket requests. There are no analytics, no background update daemons, and zero external telemetry.

### 🪓 2. True Physical Redaction
Conventional tools simply draw black rectangles over confidential text, leaving the underlying text selectable and copyable. PDFkit re-renders redacted pages and **permanently purges the original text objects, fonts, and bounding boxes from the file stream.** The redacted data is irrecoverable.

### 🎯 3. Intelligent Watermark Stripping
Easily eliminate invasive diagonal and background watermarks from lecture notes, exam booklets, or draft memos. Target headers, footers, or central page bodies independently with one click.

### 📄 4. High-Fidelity Office Export (Word / PowerPoint / Excel / TXT)
Automatic multi-column flow detection, table reconstruction heuristics, and full Unicode Turkish character support. Complex documents are parsed in seconds instead of minutes.

### 🖋️ 5. Comprehensive Utility Suite
- **Page Operations:** Drag-and-drop page reordering, splitting, merging, rotating, and deleting.
- **Form Filling:** Native AcroForm field detection with robust UTF-8 and Turkish character encoding.
- **Security & Permissions:** AES-256 encryption with granular printing, copying, and modification flags.
- **Compression:** Lossless and balanced profile presets for optimized archiving.
- **Direct Printing:** Hardware-accelerated direct printing through native Windows spoolers.

---

## 📦 Optional Python Runtime for Office Conversion

All native PDF editing, redaction, watermarking, and page management features run **with zero dependencies**. For high-fidelity Word and PowerPoint export, having Python on your system is recommended:

```powershell
# 1. Install Python (or download from python.org)
winget install -e --id Python.Python.3.13

# 2. Install conversion modules
py -m pip install pymupdf pdf2docx python-docx python-pptx xlsxwriter
```

For more details, consult [DOCUMENTATION.md](DOCUMENTATION.md).

---

## 🏢 Enterprise Deployment (IT & Sysadmins)

PDFkit is ready for automated corporate rollouts via Active Directory (GPO), Microsoft Intune, SCCM, or PDQ Deploy.

```powershell
# Silent unattended installation
msiexec.exe /i "PDFkit_0.1.0_x64_en-US.msi" /qn /norestart

# Silent uninstall
msiexec.exe /x "PDFkit_0.1.0_x64_en-US.msi" /qn /norestart
```

---

## 🔒 Windows SmartScreen Notice

> PDFkit is an independent open-source project. Because it is not packaged with an enterprise-paid code signing certificate, Windows SmartScreen may present an *"Unknown Publisher"* prompt during initial execution.
>
> The software is fully auditable and 100% offline. You can safely proceed by selecting **"More info" → "Run anyway"**.

---

## 📄 License
This software is distributed under the **[GNU Affero General Public License v3.0 (AGPL-3.0)](LICENSE)**.

Copyright (c) 2026 Çağdaş Polat ([github.com/cagdass1128](https://github.com/cagdass1128))

* Individuals and enterprises may use PDFkit for internal workflows **completely free and without restrictions**.
* See the [LICENSE](LICENSE) file for terms and conditions.

---

<div align="center">
Developer: <b>Çağdaş Polat</b> • Systems & Infrastructure Engineering<br/>
<i>"Lighter, safer, and smarter solutions for modern organizations."</i>
</div>
