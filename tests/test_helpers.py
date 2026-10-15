"""
Test Helpers for PDFKit Conversion Tests
Generates test PDF fixtures with Unicode text, tables, callout banners, multi-column layouts, and technical diagrams.
"""

import os
import tempfile
import pymupdf


def get_system_font():
    """Returns path to Arial font on Windows if available, else None."""
    candidates = [
        "C:/Windows/Fonts/arial.ttf",
        "C:/Windows/Fonts/calibri.ttf",
        "C:/Windows/Fonts/segoeui.ttf",
    ]
    for c in candidates:
        if os.path.exists(c):
            return c
    return None


def create_sample_pdf(num_pages: int = 2) -> str:
    """Creates a temporary PDF file with sample text, Turkish characters, and a table."""
    tmp = tempfile.NamedTemporaryFile(suffix=".pdf", delete=False)
    tmp_path = tmp.name
    tmp.close()

    font_path = get_system_font()

    doc = pymupdf.open()
    for p in range(1, num_pages + 1):
        page = doc.new_page(width=595, height=842)  # A4 size

        kwargs = {}
        if font_path:
            kwargs = {"fontname": "arial", "fontfile": font_path}

        # Page Header
        page.insert_text((50, 50), f"Sayfa {p} - Test Belgesi", fontsize=16, **kwargs)

        # Turkish Unicode Characters
        page.insert_text(
            (50, 90),
            "Türkçe Karakter Desteği: ç, ğ, ı, ö, ş, ü, Ç, Ğ, İ, Ö, Ş, Ü.",
            fontsize=11,
            **kwargs
        )

        # Paragraph text
        page.insert_text(
            (50, 120),
            "Bu metin PDFKit dönüşüm motoru testleri için otomatik oluşturulmuştur.",
            fontsize=10,
            **kwargs
        )

        # Tabular data lines
        page.insert_text((50, 160), "Ürün Adı\tMiktar\tFiyat", fontsize=10, **kwargs)
        page.insert_text((50, 180), "Kahve Çekirdeği\t2\t150,00 TL", fontsize=10, **kwargs)
        page.insert_text((50, 200), "Şeker\t1\t45,50 TL", fontsize=10, **kwargs)

        # Draw a rectangle line
        page.draw_line(pymupdf.Point(50, 215), pymupdf.Point(545, 215), color=(0.2, 0.4, 0.8), width=1.5)

    doc.save(tmp_path)
    doc.close()
    return tmp_path


def create_multicolumn_pdf() -> str:
    """Creates a 2-column technical document with header and distinct column flows."""
    tmp = tempfile.NamedTemporaryFile(suffix=".pdf", delete=False)
    tmp_path = tmp.name
    tmp.close()

    font_path = get_system_font()
    kwargs = {"fontname": "arial", "fontfile": font_path} if font_path else {}

    doc = pymupdf.open()
    page = doc.new_page(width=595, height=842)

    # Main Header across full width
    page.insert_text((50, 45), "CCNA Routing and Switching Study Guide", fontsize=16, **kwargs)
    page.draw_line(pymupdf.Point(50, 60), pymupdf.Point(545, 60), color=(0.3, 0.3, 0.3), width=1.0)

    # Left Column (X: 50..270)
    page.insert_text((50, 90), "Bölüm 1: Ağ Temelleri", fontsize=12, **kwargs)
    page.insert_text((50, 110), "OSI referans modeli yedi katmandan oluşur.", fontsize=10, **kwargs)
    page.insert_text((50, 125), "Fiziksel katman veri bitlerinin iletimini sağlar.", fontsize=10, **kwargs)
    page.insert_text((50, 140), "Veri bağı katmanı MAC adreslerini kullanır.", fontsize=10, **kwargs)

    # Right Column (X: 310..530)
    page.insert_text((310, 90), "Bölüm 2: IP Yönlendirme", fontsize=12, **kwargs)
    page.insert_text((310, 110), "Yönlendiriciler paketleri hedef ağa iletir.", fontsize=10, **kwargs)
    page.insert_text((310, 125), "Statik ve dinamik yönlendirme protokolleri.", fontsize=10, **kwargs)
    page.insert_text((310, 140), "OSPF hiyerarşik alan tabanlı bir protokoldür.", fontsize=10, **kwargs)

    # Footer
    page.insert_text((250, 800), "Sayfa 42 / CCNA Rehberi", fontsize=9, **kwargs)

    doc.save(tmp_path)
    doc.close()
    return tmp_path


def create_callout_note_pdf() -> str:
    """Creates a page featuring a colored callout NOTE banner box with enclosed text."""
    tmp = tempfile.NamedTemporaryFile(suffix=".pdf", delete=False)
    tmp_path = tmp.name
    tmp.close()

    font_path = get_system_font()
    kwargs = {"fontname": "arial", "fontfile": font_path} if font_path else {}

    doc = pymupdf.open()
    page = doc.new_page(width=595, height=842)

    # Preceding text
    page.insert_text((50, 60), "Yapılandırma Adımları Öncesi Önemli Bilgi", fontsize=14, **kwargs)

    # Green NOTE banner background (width: 480, height: 70)
    banner_rect = pymupdf.Rect(50, 90, 530, 160)
    page.draw_rect(banner_rect, color=(0.1, 0.5, 0.2), fill=(0.9, 0.96, 0.92), width=1.0)

    # Text inside the NOTE box
    page.insert_text((65, 115), "NOTE: Router arayüzünü açmak için no shutdown komutu gereklidir.", fontsize=10, **kwargs)
    page.insert_text((65, 135), "Aksi takdirde port yönetimsel olarak kapalı (administratively down) kalır.", fontsize=9, **kwargs)

    # Following text below banner
    page.insert_text((50, 190), "Adım 1: Konsol kablosunu bağlayın ve terminal yazılımını başlatın.", fontsize=10, **kwargs)

    doc.save(tmp_path)
    doc.close()
    return tmp_path


def create_technical_diagram_pdf() -> str:
    """Creates a page with a multi-shape technical diagram (flowchart with lines, boxes, connectors)."""
    tmp = tempfile.NamedTemporaryFile(suffix=".pdf", delete=False)
    tmp_path = tmp.name
    tmp.close()

    font_path = get_system_font()
    kwargs = {"fontname": "arial", "fontfile": font_path} if font_path else {}

    doc = pymupdf.open()
    page = doc.new_page(width=595, height=842)

    page.insert_text((50, 50), "Şekil 3.1: Paket Yönlendirme Karar Akışı", fontsize=14, **kwargs)

    # Diagram drawing mesh: rectangles and connecting lines
    page.draw_rect(pymupdf.Rect(100, 100, 200, 140), color=(0.1, 0.3, 0.7), fill=(0.85, 0.9, 1.0), width=1.0)
    page.insert_text((115, 122), "Paket Alındı", fontsize=9, **kwargs)

    page.draw_line(pymupdf.Point(200, 120), pymupdf.Point(260, 120), color=(0.2, 0.2, 0.2), width=1.5)

    page.draw_rect(pymupdf.Rect(260, 100, 360, 140), color=(0.1, 0.3, 0.7), fill=(0.85, 0.9, 1.0), width=1.0)
    page.insert_text((270, 122), "Hedef IP Kontrolü", fontsize=9, **kwargs)

    page.draw_line(pymupdf.Point(360, 120), pymupdf.Point(420, 120), color=(0.2, 0.2, 0.2), width=1.5)

    page.draw_rect(pymupdf.Rect(420, 100, 520, 140), color=(0.1, 0.5, 0.2), fill=(0.85, 0.95, 0.85), width=1.0)
    page.insert_text((435, 122), "Arayüze İletildi", fontsize=9, **kwargs)

    page.draw_line(pymupdf.Point(310, 140), pymupdf.Point(310, 180), color=(0.2, 0.2, 0.2), width=1.5)
    page.draw_rect(pymupdf.Rect(250, 180, 370, 220), color=(0.7, 0.2, 0.1), fill=(1.0, 0.9, 0.9), width=1.0)
    page.insert_text((265, 202), "ARP Tablosu Ara", fontsize=9, **kwargs)

    # Text after diagram
    page.insert_text((50, 270), "Yukarıdaki şemada paket iletim mantığı ayrıntılı gösterilmiştir.", fontsize=10, **kwargs)

    doc.save(tmp_path)
    doc.close()
    return tmp_path


def create_fill_only_banner_pdf() -> str:
    """Creates a PDF with a filled banner without stroke/border width (PyMuPDF width=None)."""
    tmp = tempfile.NamedTemporaryFile(suffix=".pdf", delete=False)
    tmp_path = tmp.name
    tmp.close()

    font_path = get_system_font()
    kwargs = {"fontname": "arial", "fontfile": font_path} if font_path else {}

    doc = pymupdf.open()
    page = doc.new_page(width=595, height=842)

    # Title
    page.insert_text((50, 50), "Teknik Doküman - Uyarı Notu", fontsize=14, **kwargs)

    # Filled banner with color=None (width will be None in get_drawings)
    banner_rect = pymupdf.Rect(50, 80, 520, 150)
    page.draw_rect(banner_rect, fill=(0.92, 0.96, 0.99), color=None)

    page.insert_text((65, 110), "DİKKAT: Bu işlem öncesinde yedek alınmalıdır.", fontsize=10, **kwargs)
    page.insert_text((65, 130), "Konfigürasyon kaydedilmezse yeniden başlatmada kaybolur.", fontsize=9, **kwargs)

    page.insert_text((50, 180), "Sonraki Adım: Test işlemlerine devam edin.", fontsize=10, **kwargs)

    doc.save(tmp_path)
    doc.close()
    return tmp_path

