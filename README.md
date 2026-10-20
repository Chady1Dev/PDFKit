<div align="center">

<p align="center">
  <img src="public/logo.png" width="130" alt="PDFkit Logo" />
</p>

# PDFkit
### Ultra-Hafif, Bağımsız, Sıfır-Ağ (Zero-Network) Masaüstü PDF İstasyonu

<p align="center">
  <b>Türkçe</b> &nbsp;•&nbsp; <a href="README-EN.md">English</a>
</p>

[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011%20(x64)-0078D6?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2.0-FFC131?style=for-the-badge&logo=tauri&logoColor=black)](https://tauri.app/)
[![Rust Core](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?style=for-the-badge&logo=svelte&logoColor=white)](https://svelte.dev/)
[![Zero Network](https://img.shields.io/badge/Security-Zero--Network%20(Offline)-2ea44f?style=for-the-badge&logo=shield&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![Corporate Deployment](https://img.shields.io/badge/Enterprise-MSI%20%26%20GPO%20Ready-blueviolet?style=for-the-badge&logo=windows-terminal&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![License: AGPL v3](https://img.shields.io/badge/License-GNU%20AGPL%20v3.0-blue?style=for-the-badge&logo=gnu&logoColor=white)](LICENSE)

<br/>

> **"Belgeleriniz bilgisayarınızdan hiç çıkmaz. Abonelik yok, telemetri yok, 4.15 MB'lık ultra-hafif kurulum."**

<br/>

[📥 Windows Kurulum Paketi (.exe)](https://github.com/cagdass1128/PDFKit/releases) • 
[🏢 Kurumsal Dağıtım Paketi (.msi)](https://github.com/cagdass1128/PDFKit/releases) • 
[📖 Teknik Dokümantasyon](DOCUMENTATION.md)

</div>

---

> 📌 **Önemli Not:** En iyi sonuç için lütfen uygulamanın en güncel sürümünü kullanın. PDF'ten Word, Excel ve PowerPoint'e dönüştürme motorumuz düzenli olarak güncellenip iyileştirilmektedir.

---

## 💡 Neden PDFkit?

Günlük belge işlemlerinde genellikle iki seçenek arasında kalınır:

1. **Online PDF Araçları (iLovePDF, Smallpdf vb.):** Belgeleri yabancı sunuculara yükler. Kişisel veriler, ticari sırlar veya sınav belgeleri internete çıkar; **KVKK ve GDPR** açısından ciddi risk taşır. Ayrıca dosya sınırı ve kota koyarak abonelik ister.
2. **Geleneksel Masaüstü Devleri (Adobe Acrobat Pro vb.):** Yüzlerce megabayt disk alanı kaplar, arka planda onlarca telemetri ve güncelleme servisi çalıştırır ve yıllık fahiş lisans ücretleri talep eder.

**PDFkit**, belgeleri bilgisayarınızın dışına çıkarmadan; düzenleme, sayfa yönetimi, akıllı filigran silme, fiziksel sansürleme ve Office formatlarına dönüşüm sağlayan **tamamen yerel, güvenli, bağımsız ve ücretsiz** bir PDF çalışma istasyonudur.

---

## ⚡ Karşılaştırma Tablosu

| Özellik / Kriter | 📑 **PDFkit** | 🔴 **Adobe Acrobat (Pro / Reader)** | 📄 **LibreOffice (Draw)** |
| :--- | :---: | :---: | :---: |
| **Maliyet / Lisans** | **%100 Ücretsiz & Açık Kaynak** | **₺478,80/ay (₺5.745/yıl)** *(Reader'da araçlar kilitlidir)* | Ücretsiz & Açık Kaynak |
| **Kurulum Boyutu** | **`4.15 MB`** *(Tek tıkla hazır)* | ~`350 – 500 MB` | ~`300 – 450 MB` |
| **Açılış Hızı** | **`< 0.3 saniye`** *(Anında)* | 2.5 – 5.0 saniye | 3.0 – 6.0 saniye |
| **Boşta RAM Tüketimi** | **`~35 MB`** *(Tauri v2 + Rust)* | 350 – 500 MB+ | 250 – 400 MB |
| **Gizlilik & Ağ İletişimi** | **%100 Çevrimdışı (0 KB Veri)** | Arka plan bulut servisleri | Yerel (telemetri opsiyonlu) |
| **PDF Sayfa Yönetimi** | Sırala, ayır, birleştir, döndür, sil | Pro abonelik gerektirir (Reader'da kilitli) | Sayfaları çizim nesnesine bozar |
| **Kalıcı / Fiziksel Sansürleme** | **Var** *(Metin dosyadan tamamen silinir)* | Sadece ücretli Pro sürümde | Yok *(Üstüne çizilen kutu kaldırılabilir)* |
| **Akıllı Filigran Silme** | **Tek Tıkla Bölge Analizi (Başlık/Gövde/Dipnot)** | Yok *(Manuel uğraş)* | Yok *(Her harf ayrı vektör nesnesi olur)* |
| **Kurumsal Dağıtım (GPO)** | **`5.78 MB MSI`** paketi hazır | Hantal MSI paketleri | Büyük MSI paketi |
| **Windows Optimizasyonu** | Yerel Windows yazdırma ve GDI motoru ile derin entegrasyon | Çapraz platform ağırlığı | Ağır VCL çalışma zamanı |

---

## ✨ Öne Çıkan Özellikler

### 🛡️ 1. Sıfır Ağ İletişimi (Zero-Network)
Uygulama katı bir İçerik Güvenlik Politikası (CSP) ile derlenmiştir. Dışarıya hiçbir HTTP/WebSocket isteği atamaz; telemetri, veri toplama veya gizli arka plan servisleri barındırmaz.

### 🪓 2. Kalıcı ve Fiziksel Sansürleme (Redaction)
Geleneksel araçlar metnin üstünü sadece siyah bir kutuyla örter; altındaki yazı kopyalanabilir. PDFkit'te sansür uygulanan sayfadaki **orijinal metin, yazı tipi ve karakter baytları dosyadan kalıcı olarak silinir.** Bilgi asla geri döndürülemez.

### 🎯 3. Akıllı Filigran Temizleme
Üniversite ders notları, açıköğretim sınav soruları ve resmi taslak evraklardaki şeffaf veya çapraz filigranları içerik akışını bozmadan tek tıkla temizler. Üst bilgi, gövde ve alt bilgi bölgeleri seçilerek temizlik yapılabilir.

### 📄 4. Office Dönüşümü (Word / PowerPoint / Excel / TXT)
Çift sütunlu metin algılama, gelişmiş tablo yapılandırması ve tam Türkçe karakter desteği. Ağır tablolu belgeler dakikalar yerine saniyeler içinde Word veya Excel'e dönüştürülür.

### 🖋️ 5. Pratik Düzenleme ve Sayfa Araçları
- **Sayfa Yönetimi:** Sayfaları sürükle-bırak yöntemiyle sıralama, döndürme, silme, bölme ve birleştirme.
- **İçerik Düzenleme:** Metin, şekil, vurgulama ve el yazısı imza ekleme.
- **Form Doldurma:** AcroForm alanlarını algılama ve Türkçe karakter desteğiyle doldurma.
- **Güvenlik:** AES-256 şifreleme; yazdırma, kopyalama ve düzenleme izinlerini kısıtlama.
- **Sıkıştırma:** Kayıpsız veya dengeli sıkıştırma profilleri.
- **Doğrudan Yazdırma:** Windows yazıcı sürücüleriyle doğrudan iletişim.

---

## 📦 Office Dönüşümü İçin Python (İsteğe Bağlı)

PDFkit'in tüm temel özellikleri (düzenleme, sansür, sayfa araçları, filigran, formlar) **hiçbir ek kurulum gerektirmeden** çalışır. Word ve PowerPoint dönüşümleri için bilgisayarınızda Python bulunması önerilir:

```powershell
# 1. Python'u kurun (veya python.org adresinden edinin)
winget install -e --id Python.Python.3.13

# 2. Gerekli açık kaynak kütüphaneleri yükleyin
py -m pip install pymupdf pdf2docx python-docx python-pptx xlsxwriter
```

Detaylı bilgi için [DOCUMENTATION.md](DOCUMENTATION.md) dosyasına göz atabilirsiniz.

---

## 🏢 Kurumsal Dağıtım (IT & Sistem Yöneticileri)

PDFkit; Active Directory (GPO), Microsoft Intune, SCCM veya PDQ Deploy ile merkezi dağıtım için hazır imzalanabilir `.msi` paketi olarak sunulur.

```powershell
# Sessiz (Silent) Kurulum
msiexec.exe /i "PDFkit_0.1.0_x64_tr-TR.msi" /qn /norestart

# Sessiz Kaldırma
msiexec.exe /x "PDFkit_0.1.0_x64_tr-TR.msi" /qn /norestart
```

---

## 🔒 Windows SmartScreen Bildirimi

> PDFkit bağımsız ve açık kaynaklı bir projedir. Ticari kod imzalama sertifikası bulunmadığı için Windows SmartScreen ilk çalıştırmada *"Bilinmeyen Yayıncı"* uyarısı gösterebilir.
>
> Uygulama hiçbir telemetri içermez ve tüm kaynak kodu açıktır. Kurulum ekranında **"Ek Bilgi" → "Yine de Çalıştır"** seçeneğiyle güvenle ilerleyebilirsiniz.

---

## 📄 Lisans
Bu proje **[GNU Affero General Public License v3.0 (AGPL-3.0)](LICENSE)** ile lisanslanmıştır.

Copyright (c) 2026 Çağdaş Polat ([github.com/cagdass1128](https://github.com/cagdass1128))

* Bireysel kullanıcılar ve kurumlar PDFkit'i ticari veya kurum içi süreçlerde **ücretsiz ve sınırsız** kullanabilir.
* Detaylar için [LICENSE](LICENSE) belgesini inceleyebilirsiniz.

---

<div align="center">
Geliştirici: <b>Çağdaş Polat</b> • Sistem Destek & Altyapı Yönetimi<br/>
<i>"Kurumlar için daha hafif, daha güvenli ve daha akılcı çözümler."</i>
</div>
