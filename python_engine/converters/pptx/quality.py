"""
PPTX Quality Assurance and Active Healing Engine
Audits converted PageIR models and actively resolves text overlaps,
diagram ghost text collisions, and table vertical spacing before PowerPoint generation.
"""

from dataclasses import dataclass, field
from typing import List, Tuple
from .ir import PageIR, Rect, TableIR, TextBlockIR
from .layout import apply_table_vertical_push_down


@dataclass
class QualityReport:
    page_number: int
    editable_text_blocks: int = 0
    editable_tables: int = 0
    editable_shapes: int = 0
    editable_callouts: int = 0
    raster_diagrams: int = 0
    raster_images: int = 0
    total_characters: int = 0
    editability_ratio: float = 1.0
    detected_overlaps: List[Tuple[str, str]] = field(default_factory=list)


class QualityAuditor:
    """Performs automated quality audit and active collision healing on PageIR."""

    @classmethod
    def audit_and_heal(cls, page_ir: PageIR) -> QualityReport:
        # 1. Apply Table Vertical Push-Down
        apply_table_vertical_push_down(page_ir)

        # 2. Active Table Validation & Pruning
        valid_tables: List[TableIR] = []
        for t in page_ir.tables:
            total_cells = t.rows * t.cols
            if total_cells == 0:
                continue
            filled_cells = sum(1 for row in t.cells for cell in row if cell.text.strip())
            if filled_cells == 0 or (total_cells <= 4 and filled_cells <= 1):
                continue
            valid_tables.append(t)
        page_ir.tables = valid_tables

        # 3. KURAL: TableIR ve DiagramIR ile kesişen TextBlockIR'leri filtrele
        clean_blocks: List[TextBlockIR] = []
        for b in page_ir.text_blocks:
            # Check overlap with confirmed tables (> 25% area)
            overlaps_table = False
            for t in page_ir.tables:
                if b.bbox.intersects(t.bbox) and b.bbox.area > 0:
                    if (b.bbox.intersection_area(t.bbox) / b.bbox.area) > 0.25:
                        overlaps_table = True
                        break
            if overlaps_table:
                continue

            is_real_body_para = (len(b.lines) >= 3 and b.bbox.width >= page_ir.width * 0.50)
            in_diagram_label = False

            for d in page_ir.diagrams:
                if b.bbox.intersects(d.bbox) and b.bbox.area > 0:
                    inter_ratio = b.bbox.intersection_area(d.bbox) / b.bbox.area
                    if inter_ratio >= 0.75 and not is_real_body_para:
                        in_diagram_label = True
                        b.is_protected = False
                        break

            if in_diagram_label:
                continue

            if b.is_protected:
                clean_blocks.append(b)
                continue

            is_ghost = False
            for d in page_ir.diagrams:
                if d.bbox.contains(Rect(b.bbox.center_x, b.bbox.center_y, b.bbox.center_x + 1, b.bbox.center_y + 1)):
                    is_ghost = True
                    break
                if b.bbox.intersects(d.bbox):
                    inter = b.bbox.intersection_area(d.bbox)
                    if b.bbox.area > 0 and (inter / b.bbox.area) >= 0.05:
                        is_ghost = True
                        break
            if not is_ghost:
                clean_blocks.append(b)
        page_ir.text_blocks = clean_blocks

        # 4. ADIM 3.1 & 3.2: Dikey itme ve yatay kırpma TAMAMEN kaldırıldı.
        # Hiçbir metin bloğu orijinal PDF y0 koordinatından kaydırılamaz ve orig_width altına düşürülemez.
        for b in page_ir.text_blocks:
            min_w = max(4.0, b.orig_width)
            b.bbox.x1 = max(b.bbox.x0 + min_w, b.bbox.x1)
            b.bbox.y1 = max(b.bbox.y0 + 4.0, b.bbox.y1)

        # 7. Generate Quality Report
        report = QualityReport(page_number=page_ir.page_number)
        report.editable_text_blocks = len(page_ir.text_blocks)
        report.editable_tables = len(page_ir.tables)
        report.editable_shapes = len(page_ir.shapes)
        report.editable_callouts = len(page_ir.callouts)
        report.raster_diagrams = len(page_ir.diagrams)
        report.raster_images = len(page_ir.images)

        chars = sum(len(b.text) for b in page_ir.text_blocks)
        for c in page_ir.callouts:
            chars += sum(len(b.text) for b in c.text_blocks)
        for t in page_ir.tables:
            for row in t.cells:
                chars += sum(len(cell.text) for cell in row)
        report.total_characters = chars

        editable_count = (
            report.editable_text_blocks
            + report.editable_tables
            + report.editable_shapes
            + report.editable_callouts
        )
        total_objects = editable_count + report.raster_diagrams + report.raster_images
        if total_objects > 0:
            report.editability_ratio = round(editable_count / total_objects, 3)

        blocks = page_ir.text_blocks
        for i in range(len(blocks)):
            for j in range(i + 1, len(blocks)):
                b1, b2 = blocks[i], blocks[j]
                if b1.bbox.intersects(b2.bbox):
                    inter = b1.bbox.intersection_area(b2.bbox)
                    min_area = min(b1.bbox.area, b2.bbox.area)
                    if min_area > 0 and (inter / min_area) > 0.25:
                        report.detected_overlaps.append((b1.id, b2.id))

        return report

    @classmethod
    def audit(cls, page_ir: PageIR) -> QualityReport:
        """Backward-compatible audit method delegating to audit_and_heal."""
        return cls.audit_and_heal(page_ir)
