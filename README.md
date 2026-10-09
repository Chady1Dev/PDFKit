<div align="center">

# 📑 PDFKit
### Ultra-Hafif, Bağımsız, Sıfır-Ağ (Zero-Network) Masaüstü PDF İstasyonu

[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011%20(x64)-0078D6?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2.0-FFC131?style=for-the-badge&logo=tauri&logoColor=black)](https://tauri.app/)
[![Rust Core](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?style=for-the-badge&logo=svelte&logoColor=white)](https://svelte.dev/)
[![Zero Network](https://img.shields.io/badge/Security-Zero--Network%20(Offline)-2ea44f?style=for-the-badge&logo=shield&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![Corporate Deployment](https://img.shields.io/badge/Enterprise-MSI%20%26%20GPO%20Ready-blueviolet?style=for-the-badge&logo=windows-terminal&logoColor=white)](https://github.com/cagdass1128/PDFKit)

<br/>

> **"Kişisel verileriniz ve şirket sözleşmeleriniz asla internete sızmasın. Ağır abonelikler ve hantal yazılımlar yerine 3.5 MB'lık saf mühendislik."**

<br/>

[📥 Windows Kurulum Paketini İndir (.exe)](https://github.com/cagdass1128/PDFKit/releases) • 
[🏢 Kurumsal Dağıtım Paketi (.msi)](https://github.com/cagdass1128/PDFKit/releases) • 
[📖 Teknik Dokümantasyon](DOCUMENTATION.md)

</div>

---

## 💡 Neden PDFKit? (Problem & Çözüm)

Modern bilgi çalışanları, IT departmanları ve kurumlar belge akışında iki tehlikeli çıkmazın arasına sıkışmıştır:

1. **Bulut Tabanlı Çeviriciler (iLovePDF, Smallpdf vb.):** Belgeleri üçüncü taraf sunuculara yükler. Kişisel veriler, ticari sırlar veya sözleşmeler internete çıkar; **KVKK ve GDPR açısından ciddi bir güvenlik açığı** oluşturur.
2. **Geleneksel Masaüstü Devleri (Adobe Acrobat vb.):** 2 GB+ disk alanı kaplar, arka planda onlarca telemetri ve güncelleme servisi çalıştırır, yüzlerce MB RAM tüketir ve kullanıcı başına yıllık yüksek abonelik ücretleri talep eder.

**PDFKit**, belgeleri **asla bilgisayar dışına çıkarmadan**, tek bir bayt bile internete göndermeden, **3.5 MB'lık** bir kurulum paketiyle çalışan; kurumsal düzeyde düzenleme, filigran temizleme, fiziksel sansürleme ve Office dönüşümü sağlayan **%100 yerel ve ücretsiz** bir çözümdür.

---

## ⚡ Karşılaştırma Tablosu

| Kriter | Adobe Acrobat Pro DC | Bulut Araçları (iLovePDF vb.) | 📑 PDFKit (Bu Proje) |
| :--- | :---: | :---: | :---: |
| **Maliyet** | ~Yıllık Binlerce TL | Kotalı / Aylık Ücret | **%100 Ücretsiz & Bağımsız** |
| **Kurulum Boyutu** | ~1.8 GB – 2.5 GB | Kurulumsuz (Tarayıcı) | **⚡ 3.45 MB (Installer)** |
| **Boşta RAM Tüketimi** | ~350 MB – 650 MB | ~300 MB (Sekme başına) | **🚀 ~45 MB – 65 MB** |
| **Veri Güvenliği (KVKK)** | Bulut / Telemetri Açık | ❌ **Sıfır (Dosyalar sunucuya gider)** | 🛡️ **%100 Yerel (Sıfır Ağ Çağrısı)** |
| **Fiziksel Sansürleme** | Sadece Pro'da | Güvensiz | 🪓 **Gerçek Bayt İmhası (`lopdf`)** |
| **AÖF / Sınav Filigranı Silme** | Zor / Katman Bozar | Sayfayı bozar | 🎯 **Tek Tıkla Akıllı Motor** |
| **Kurumsal Dağıtım** | Ağır / Lisans Sunucusu | Yok | 🏢 **MSI + GPO / Intune Uyumlu** |

---

## ✨ Öne Çıkan Özellikler

### 🛡️ 1. Sıfır Ağ İletişimi & %100 Yerel Güvenlik (Zero-Network)
Uygulama `tauri.conf.json` üzerinde katı bir **İçerik Güvenlik Politikası (CSP)** ile yapılandırılmıştır. Ağ bağlantıları çekirdek seviyesinde kilitlidir. Açtığınız, düzenlediğiniz veya dönüştürdüğünüz hiçbir dosya bilgisayarınızın dışına çıkamaz.

### 🪓 2. Fiziksel Sansürleme (True Byte Redaction)
Amatör araçlar metnin üstünü sadece siyah bir kutuyla boyar (metin altta seçilebilir ve kopyalanabilir kalır). **PDFKit**, seçilen koordinatlardaki glifleri (`Tj`, `TJ`) doğrudan PDF'in içerik akışından ve bayt diziliminden **fiziksel olarak kazır**. Sansürlenen veri geri döndürülemez.

### 🎯 3. Akıllı Vektörel Filigran Temizleme
Akademik notlarda, sınav sorularında veya taslak sözleşmelerde bulunan çapraz ve şeffaf filigranları sayfa katmanlarından analiz ederek tek tıkla temizler. Üst, alt veya gövde bölgesi bazında seçici temizleme imkanı sunar.

### ⚡ 4. 500x Hızlandırılmış Ofis Dönüşüm Motoru (DOCX / PPTX)
PDF'ten Word (DOCX) ve Sunum (PPTX) dönüşümlerinde, özel geliştirilen `patch_docx_table_performance` algoritması sayesinde $O(N^2)$ karmaşıklığı $O(1)$ sabit erişim hızına indirilmiştir. Ağır tablolu belgeler dahi dakikalar yerine **saniyeler içinde** dönüştürülür.

### 🖋️ 5. Form Doldurma, İmzalar & Sayfa Yönetimi
- **AcroForms:** PDF form alanlarını doldurma ve düzleştirme (flattening).
- **Dijital Kaşe & İmza:** Vektörel imza çizim tuvali ile belgeleri anında imzalama.
- **Sayfa Organizasyonu:** Sürükle-bırak ile sayfaları yeniden sıralama, döndürme, ayırma ve birleştirme.

---

## 🏢 Kurumsal IT & Sistem Yöneticileri İçin Dağıtım (Enterprise Deployment)

PDFKit, kurumsal Windows ortamlarında (Active Directory, Microsoft Intune, SCCM veya PDQ Deploy) merkezi olarak dağıtılabilmesi için resmi `.msi` paketi ile paketlenmiştir.

### Sessiz Kurulum (Silent Install - PowerShell / CMD):
```powershell
# Ağ üzerinden veya yerel terminalden sessiz kurulum
msiexec.exe /i "PDFKit_0.1.0_x64_en-US.msi" /qn /norestart

# Sessiz kaldırma (Uninstall)
msiexec.exe /x "PDFKit_0.1.0_x64_en-US.msi" /qn /norestart
```

---

## 🛠️ Teknoloji Yığını ve Mimari

* **Arayüz (Frontend):** Svelte 5, TypeScript, Vite, TailwindCSS (60 FPS kesintisiz tuval yönetimi)
* **Yerel Çekirdek (Backend):** Rust (2021 Edition), Tauri v2 IPC, `lopdf` (Düşük seviyeli bayt manipülasyonu)
* **Sistem Entegrasyonu:** Windows Win32 GDI & WinSpool (Doğrudan yazıcı sürücüsü)
* **Dönüştürücü Motoru:** Python Subprocess (`PyMuPDF`, `pdf2docx`, `python-pptx`)

Daha derin teknik ve algoritmik detaylar için [DOCUMENTATION.md](DOCUMENTATION.md) dosyasını inceleyebilirsiniz.

---

## 🔒 Güvenlik & Windows SmartScreen Bildirimi

> **Not:** PDFKit tamamen bağımsız ve açık kaynaklı bir topluluk girişimidir. Yıllık binlerce dolarlık ticari kod imzalama (EV Code Signing) sertifikasına sahip olmadığı için, Windows SmartScreen ilk çalıştırmada *"Bilinmeyen Yayıncı"* uyarısı verebilir.
> 
> Uygulama tamamen güvenlidir ve sıfır telemetri içerir. Kurulum ekranında **"Ek Bilgi" (More Info) -> "Yine de Çalıştır" (Run Anyway)** seçeneğiyle güvenle başlatabilirsiniz.

---

## 🚀 Geliştirici Kurulumu (Kaynak Koddan Derleme)

```bash
# 1. Depoyu klonlayın
git clone https://github.com/cagdass1128/PDFKit.git
cd PDFKit

# 2. Bağımlılıkları yükleyin
npm install

# 3. Geliştirme ortamında çalıştırın
npm run dev

# 4. Release paketlerini derleyin
npm run build:app
```

---

## 📄 Lisans
Bu proje topluluğun ve kurumların güvenli, hafif ve ücretsiz araçlara erişebilmesi amacıyla geliştirilmiştir. Açık kaynak standartlarına uygundur.

---

<div align="center">
Geliştirici: <b>Çağdaş</b> • Sistem Destek & Altyapı Yönetimi<br/>
<i>"Kurumlar için daha hafif, daha güvenli ve daha akılcı çözümler."</i>
</div>
