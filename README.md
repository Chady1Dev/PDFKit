<div align="center">

# 📑 PDFkit
### Ultra-Hafif, Bağımsız, Sıfır-Ağ (Zero-Network) Masaüstü PDF İstasyonu

[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011%20(x64)-0078D6?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2.0-FFC131?style=for-the-badge&logo=tauri&logoColor=black)](https://tauri.app/)
[![Rust Core](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?style=for-the-badge&logo=svelte&logoColor=white)](https://svelte.dev/)
[![Zero Network](https://img.shields.io/badge/Security-Zero--Network%20(Offline)-2ea44f?style=for-the-badge&logo=shield&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![Corporate Deployment](https://img.shields.io/badge/Enterprise-MSI%20%26%20GPO%20Ready-blueviolet?style=for-the-badge&logo=windows-terminal&logoColor=white)](https://github.com/cagdass1128/PDFKit)
[![License: AGPL v3](https://img.shields.io/badge/License-GNU%20AGPL%20v3.0-blue?style=for-the-badge&logo=gnu&logoColor=white)](LICENSE)

<br/>

> **"Belgeleriniz bilgisayarınızdan hiç çıkmaz. Abonelik yok, telemetri yok, ~4 MB'lık kurulum."**

<br/>

[📥 Windows Kurulum Paketi (.exe)](https://github.com/cagdass1128/PDFKit/releases) • 
[🏢 Kurumsal Dağıtım Paketi (.msi)](https://github.com/cagdass1128/PDFKit/releases) • 
[📖 Teknik Dokümantasyon](DOCUMENTATION.md)

</div>

---

## 💡 Neden PDFkit?

Belge iş akışında genellikle iki seçenek vardır ve ikisinin de bedeli vardır:

1. **Online PDF araçları** belgeleri üçüncü taraf sunuculara yükler. Kişisel veriler, sözleşmeler ve ticari bilgiler kurum dışına çıkar; bu da **KVKK / GDPR** açısından risk oluşturur.
2. **Geleneksel masaüstü PDF yazılımları** yüksek disk alanı kaplar, arka planda güncelleme ve lisans servisleri çalıştırır ve kullanıcı başına yıllık abonelik ister.

**PDFkit**, belgeleri bilgisayar dışına çıkarmadan; düzenleme, filigran temizleme, kalıcı sansürleme, şifreleme ve Office dönüşümü yapan **yerel, ücretsiz ve açık kaynak** bir alternatiftir.

---

## ⚡ Karşılaştırma

| Kriter | Geleneksel Masaüstü PDF Yazılımları | Online PDF Araçları | 📑 PDFkit |
| :--- | :---: | :---: | :---: |
| **Maliyet** | Yıllık abonelik | Kotalı / aylık ücret | **Ücretsiz & açık kaynak** |
| **Kurulum boyutu** | Yüzlerce MB – birkaç GB | Kurulumsuz (tarayıcı) | **~4 MB (installer)** |
| **Veri gizliliği** | Bulut senkronizasyonu / telemetri | Dosyalar sunucuya yüklenir | **%100 yerel, ağ erişimi yok** |
| **Arka plan servisleri** | Güncelleme / lisans servisleri | — | **Yok** |
| **Kalıcı sansürleme** | Genellikle ücretli sürümde | Güvenilir değil | **Orijinal metin dosyadan silinir** |
| **Filigran temizleme** | Manuel | Sınırlı | **Tek tıkla, bölge seçimli** |
| **Kurumsal dağıtım** | Lisans altyapısı gerektirebilir | Yok | **MSI + GPO / Intune** |

---

## ✨ Öne Çıkan Özellikler

### 🛡️ 1. Sıfır Ağ İletişimi
Uygulama katı bir **İçerik Güvenlik Politikası (CSP)** ile derlenir; dışarıya HTTP/WebSocket isteği atamaz. Güncelleme servisi, telemetri veya hesap sistemi yoktur. Tüm işlemler yerelde yapılır.

### 🪓 2. Kalıcı Sansürleme
Basit araçlar metnin üstüne siyah kutu çizer; alttaki metin seçilip kopyalanabilir. PDFkit'te sansür içeren sayfa kaydedilirken görüntüye dönüştürülür ve **orijinal metin, font ve açıklama nesneleri dosyadan tamamen silinir.** Sansürlenen bilgi geri getirilemez.

### 🎯 3. Akıllı Filigran Temizleme
Ders notları, sınav kitapçıkları ve taslak belgelerdeki çapraz/şeffaf filigranları içerik akışını analiz ederek temizler. Üst, alt ve gövde bölgeleri ayrı ayrı seçilebilir.

### 📄 4. Office Dönüşümü (Word / PowerPoint / Excel / TXT)
İki sütunlu düzen algılama, tablo yeniden inşası ve Türkçe karakter desteği. Tablo işlemleri için geliştirilen hücre önbelleği sayesinde ağır tablolu belgeler dakikalar yerine saniyeler içinde dönüştürülür.

### 🖋️ 5. Diğer Araçlar
- **Düzenleme:** Metin, şekil, vurgulama, imza çizimi ve görsel ekleme
- **Formlar:** AcroForm alanlarını doldurma (Türkçe karakter uyumlu)
- **Sayfa yönetimi:** Birleştirme, ayırma, döndürme, sıralama, silme
- **Güvenlik:** AES-256 şifreleme, yazdırma/kopyalama/düzenleme izin kısıtlama
- **Sıkıştırma:** Kayıpsız, dengeli ve maksimum seviyeler
- **Yazdırma:** Windows yazıcılarına doğrudan baskı

---

## 📦 Office Dönüşümü İçin Python (İsteğe Bağlı)

Office dönüşümü dışındaki tüm özellikler **ek bir kurulum gerektirmez.** Word ve PowerPoint dönüşümü için bilgisayarda Python bulunmalıdır (Excel ve TXT, Python olmadan da basit modda çalışır):

```powershell
# 1. Python'u kurun (veya python.org adresinden indirin)
winget install -e --id Python.Python.3.13

# 2. Dönüştürme kütüphanelerini kurun
py -m pip install pymupdf pdf2docx python-docx python-pptx xlsxwriter
```

Kurulumdan sonra dönüştürme işlemleri de tamamen çevrimdışı çalışır. Detaylar için [DOCUMENTATION.md](DOCUMENTATION.md#2-sistem-gereksinimleri).

---

## 🏢 Kurumsal Dağıtım (IT & Sistem Yöneticileri)

PDFkit; Active Directory (GPO), Microsoft Intune, SCCM veya PDQ Deploy ile merkezi dağıtım için `.msi` paketi olarak sunulur.

```powershell
# Sessiz kurulum
msiexec.exe /i "PDFkit_0.1.0_x64_en-US.msi" /qn /norestart

# Sessiz kaldırma
msiexec.exe /x "PDFkit_0.1.0_x64_en-US.msi" /qn /norestart
```

---

## 🔒 Windows SmartScreen Bildirimi

> PDFkit bağımsız ve açık kaynaklı bir projedir. Ticari kod imzalama sertifikası bulunmadığı için Windows SmartScreen ilk çalıştırmada *"Bilinmeyen Yayıncı"* uyarısı gösterebilir.
>
> Uygulama telemetri içermez ve kaynak kodu tamamen incelenebilir. Kurulum ekranında **"Ek Bilgi" → "Yine de Çalıştır"** seçeneğiyle devam edebilirsiniz.

---

## 📄 Lisans
Bu proje **[GNU Affero General Public License v3.0 (AGPL-3.0)](LICENSE)** ile lisanslanmıştır.

Copyright (c) 2026 Çağdaş Polat ([github.com/cagdass1128](https://github.com/cagdass1128))

* Bireyler ve kurumlar PDFkit'i ticari veya kurum içi süreçlerde **ücretsiz ve sınırsız** kullanabilir.
* AGPL-3.0 gereğince değiştirilmiş sürümlerin kaynak kodu da açık tutulmalıdır.
* Detaylar için [LICENSE](LICENSE) dosyasına bakabilirsiniz.

---

<div align="center">
Geliştirici: <b>Çağdaş Polat</b> • Sistem Destek & Altyapı Yönetimi<br/>
<i>"Kurumlar için daha hafif, daha güvenli ve daha akılcı çözümler."</i>
</div>
