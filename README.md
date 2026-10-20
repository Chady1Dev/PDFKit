<div align="center">

<p align="center">
  <img src="public/logo.png" width="130" alt="PDFkit Logo" />
</p>

# PDFkit
### Ultra-Hafif, Bağımsız, Sıfır-Ağ (Zero-Network) Masaüstü PDF İstasyonu

<p align="center">
  <b>Türkçe</b> &nbsp;•&nbsp; <a href="README-EN.md">English</a>
</p>

<br/>

> **"En iyi sonuç ve iyileştirilmiş belge dönüştürme için lütfen uygulamanın en güncel sürümünü kullanın.PDF'ten Word, Excel ve PowerPoint'e dönüştürme motorumuz düzenli olarak güncellenecektir."**

<br/>

[📥 Windows Kurulum Paketi (.exe)](https://github.com/Chady1Dev/PDFKit/releases) • 
[🏢 Kurumsal Dağıtım Paketi (.msi)](https://github.com/Chady1Dev/PDFKit/releases) • 
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

## ⚡ Karşılaştırma

### Genel

| | **PDFkit** | **Adobe Acrobat Reader** | **LibreOffice Draw** |
| :--- | :---: | :---: | :---: |
| **Lisans** | Ücretsiz, açık kaynak | Ücretsiz, kapalı kaynak | Ücretsiz, açık kaynak |
| **Platform** | Windows | Windows, macOS, mobil | Windows, macOS, Linux |
| **Kurulum boyutu** | **4.15 MB** | Yüzlerce MB | ~350 MB *(ofis paketiyle birlikte)* |
| **Hesap / internet** | Gerekmez *(%100 Çevrimdışı)* | Bazı özellikler hesap ister | Gerekmez |

### Ne Yapabilirim?

| İşlem | **PDFkit** | **Adobe Acrobat Reader** | **LibreOffice Draw** |
| :--- | :---: | :---: | :---: |
| Not, vurgulama, imza | ✅ | ✅ | ✅ |
| Form doldurma | ✅ | ✅ | ⚠️ Sınırlı |
| Birleştirme / ayırma | ✅ | ❌ Ücretli sürümde | ⚠️ Manuel |
| Sayfa sıralama, döndürme, silme | ✅ | ❌ Ücretli sürümde | ✅ |
| Şifreleme ve izinler | ✅ | ❌ Ücretli sürümde | ✅ Dışa aktarırken |
| Sıkıştırma | ✅ | ❌ Ücretli sürümde | ⚠️ Dışa aktarırken |
| Kalıcı sansürleme | ✅ | ❌ Ücretli sürümde | ⚠️ Tüm belge resme çevrilir |
| Filigran temizleme | ✅ | ❌ | ❌ |
| PDF → Word / Excel / PowerPoint | ✅ *(Word ve PPTx için Python gerekir)* | ❌ Ücretli sürümde | ❌ |
| Belge düzenini koruma | ✅ | ✅ | ⚠️ PDF çizim olarak açılır, düzen kayabilir |

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
> 📌 **Önemli Not:** En iyi sonuç için lütfen uygulamanın en güncel sürümünü kullanın. PDF'ten Word, Excel ve PowerPoint'e dönüştürme motorumuz düzenli olarak güncellenip iyileştirilmektedir.
---

## 🔒 Windows SmartScreen Bildirimi

> PDFkit bağımsız ve açık kaynaklı bir projedir. Ticari kod imzalama sertifikası bulunmadığı için Windows SmartScreen ilk çalıştırmada *"Bilinmeyen Yayıncı"* uyarısı gösterebilir.
>
> Uygulama hiçbir telemetri içermez ve tüm kaynak kodu açıktır. Kurulum ekranında **"Ek Bilgi" → "Yine de Çalıştır"** seçeneğiyle güvenle ilerleyebilirsiniz.

---

## 📄 Lisans
Bu proje **[GNU Affero General Public License v3.0 (AGPL-3.0)](LICENSE)** ile lisanslanmıştır.

Copyright (c) 2026 Çağdaş Polat ([github.com/Chady1Dev](https://github.com/Chady1Dev))

* Bireysel kullanıcılar ve kurumlar PDFkit'i ticari veya kurum içi süreçlerde **ücretsiz ve sınırsız** kullanabilir.
* Detaylar için [LICENSE](LICENSE) belgesini inceleyebilirsiniz.

---

<div align="center">
Geliştirici: <b>Çağdaş Polat</b> • Sistem Destek & Altyapı Yönetimi<br/>
<i>"Kurumlar için daha hafif, daha güvenli ve daha akılcı çözümler."</i>
</div>
