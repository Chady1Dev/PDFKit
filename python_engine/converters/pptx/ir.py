"""
PPTX Intermediate Representation (IR) Layer
Independent data models representing analyzed PDF page elements prior to PowerPoint slide generation.
Strict 1:1 coordinate fidelity, positive dimension locking, and complete attributes.
"""

from dataclasses import dataclass, field
from typing import List, Optional, Tuple


def safe_float(val, default: float = 0.0) -> float:
    """Safely converts val to float; returns default on None or ValueError/TypeError."""
    if val is None:
        return default
    try:
        return float(val)
    except (ValueError, TypeError):
        return default


def safe_int(val, default: int = 0) -> int:
    """Safely converts val to int; returns default on None or ValueError/TypeError."""
    if val is None:
        return default
    try:
        return int(val)
    except (ValueError, TypeError):
        return default


def safe_color_rgb(
    color_val, default: Optional[Tuple[int, int, int]] = None
) -> Optional[Tuple[int, int, int]]:
    """
    Safely converts color tuples/lists from PyMuPDF (grayscale, RGB, CMYK)
    to standard RGB 0-255 tuple (r, g, b).
    """
    if color_val is None:
        return default
    try:
        if isinstance(color_val, (int, float)):
            c = max(0, min(255, int(safe_float(color_val) * 255)))
            return (c, c, c)
        if isinstance(color_val, (tuple, list)):
            if len(color_val) == 1:
                c = max(0, min(255, int(safe_float(color_val[0]) * 255)))
                return (c, c, c)
            elif len(color_val) == 3:
                r = max(0, min(255, int(safe_float(color_val[0]) * 255)))
                g = max(0, min(255, int(safe_float(color_val[1]) * 255)))
                b = max(0, min(255, int(safe_float(color_val[2]) * 255)))
                return (r, g, b)
            elif len(color_val) >= 4:
                # CMYK: c, m, y, k (0.0 to 1.0)
                c, m, y, k = [safe_float(x) for x in color_val[:4]]
                r = max(0, min(255, int(255 * (1.0 - c) * (1.0 - k))))
                g = max(0, min(255, int(255 * (1.0 - m) * (1.0 - k))))
                b = max(0, min(255, int(255 * (1.0 - y) * (1.0 - k))))
                return (r, g, b)
        return default
    except Exception:
        return default


@dataclass
class Rect:
    """
    Immutable-friendly 2D bounding rectangle in points.
    Strict positive dimension locking: width and height are guaranteed >= 4.0 pt.
    """
    x0: float
    y0: float
    x1: float
    y1: float

    def __post_init__(self):
        self.x0 = max(0.0, safe_float(self.x0, 0.0))
        self.y0 = max(0.0, safe_float(self.y0, 0.0))
        self.x1 = max(self.x0 + 4.0, safe_float(self.x1, self.x0 + 4.0))
        self.y1 = max(self.y0 + 4.0, safe_float(self.y1, self.y0 + 4.0))

    @property
    def width(self) -> float:
        return max(4.0, self.x1 - self.x0)

    @property
    def height(self) -> float:
        return max(4.0, self.y1 - self.y0)

    @property
    def center_x(self) -> float:
        return (self.x0 + self.x1) / 2.0

    @property
    def center_y(self) -> float:
        return (self.y0 + self.y1) / 2.0

    @property
    def area(self) -> float:
        return self.width * self.height

    def contains(self, other: "Rect") -> bool:
        """Returns True if this rect completely contains other rect."""
        return (
            self.x0 <= other.x0 + 1e-3
            and self.y0 <= other.y0 + 1e-3
            and self.x1 >= other.x1 - 1e-3
            and self.y1 >= other.y1 - 1e-3
        )

    def intersects(self, other: "Rect") -> bool:
        """Returns True if this rect intersects other rect."""
        return not (
            self.x1 <= other.x0 or
            self.x0 >= other.x1 or
            self.y1 <= other.y0 or
            self.y0 >= other.y1
        )

    def intersection(self, other: "Rect") -> Optional["Rect"]:
        """Returns the intersecting Rect or None."""
        ix0 = max(self.x0, other.x0)
        iy0 = max(self.y0, other.y0)
        ix1 = min(self.x1, other.x1)
        iy1 = min(self.y1, other.y1)
        if ix1 > ix0 and iy1 > iy0:
            return Rect(ix0, iy0, ix1, iy1)
        return None

    def intersection_area(self, other: "Rect") -> float:
        inter = self.intersection(other)
        return inter.area if inter else 0.0

    def union(self, other: "Rect") -> "Rect":
        return Rect(
            min(self.x0, other.x0),
            min(self.y0, other.y0),
            max(self.x1, other.x1),
            max(self.y1, other.y1),
        )

    def expanded(self, dx: float, dy: float) -> "Rect":
        return Rect(max(0.0, self.x0 - dx), max(0.0, self.y0 - dy), self.x1 + dx, self.y1 + dy)

    def clamp_to_slide(self, slide_w: float, slide_h: float) -> "Rect":
        """Clamps rectangle strictly within slide boundary ensuring at least 4.0pt dimension."""
        x0 = max(0.0, min(self.x0, slide_w - 4.0))
        y0 = max(0.0, min(self.y0, slide_h - 4.0))
        x1 = max(x0 + 4.0, min(self.x1, slide_w))
        y1 = max(y0 + 4.0, min(self.y1, slide_h))
        return Rect(x0, y0, x1, y1)


def safe_rect(
    coords,
    default: Optional[Rect] = None,
    origin_x: float = 0.0,
    origin_y: float = 0.0,
    scale: float = 1.0,
) -> Optional[Rect]:
    """
    Safely creates a normalized Rect from any 4-tuple/list or fitz.Rect object.
    Applies origin offset and universal scale while maintaining positive dimensions.
    """
    if coords is None:
        return default
    try:
        if hasattr(coords, "x0") and hasattr(coords, "y0") and hasattr(coords, "x1") and hasattr(coords, "y1"):
            raw_x0 = safe_float(coords.x0)
            raw_y0 = safe_float(coords.y0)
            raw_x1 = safe_float(coords.x1)
            raw_y1 = safe_float(coords.y1)
        elif len(coords) >= 4:
            raw_x0 = safe_float(coords[0])
            raw_y0 = safe_float(coords[1])
            raw_x1 = safe_float(coords[2])
            raw_y1 = safe_float(coords[3])
        else:
            return default
        min_x = (min(raw_x0, raw_x1) - origin_x) * scale
        min_y = (min(raw_y0, raw_y1) - origin_y) * scale
        max_x = (max(raw_x0, raw_x1) - origin_x) * scale
        max_y = (max(raw_y0, raw_y1) - origin_y) * scale
        return Rect(min_x, min_y, max_x, max_y)
    except Exception:
        return default


@dataclass
class FontInfo:
    name: str = "Calibri"
    size: float = 10.0
    bold: bool = False
    italic: bool = False
    color: Tuple[int, int, int] = (30, 41, 59)  # Slate-800 default

    def __post_init__(self):
        self.size = max(4.0, safe_float(self.size, 10.0))
        self.bold = bool(self.bold)
        self.italic = bool(self.italic)
        if not self.color or len(self.color) < 3:
            self.color = (30, 41, 59)


@dataclass
class TextSpanIR:
    text: str
    bbox: Rect
    font: FontInfo
    clean_font_name: str = "Calibri"
    visual_font_size: float = 10.0
    char_spacing_spc: int = 0  # OOXML character spacing in 1/100 pt (e.g. -15 for condensing)
    baseline_y: float = 0.0
    parent_container_rect: Optional[Rect] = None
    span_id: int = 0

    def __post_init__(self):
        if not self.clean_font_name:
            self.clean_font_name = self.font.name
        if not self.visual_font_size or self.visual_font_size <= 0:
            self.visual_font_size = self.font.size
        if not self.baseline_y:
            self.baseline_y = self.bbox.y1 - self.bbox.height * 0.15


@dataclass
class TextLineIR:
    bbox: Rect
    spans: List[TextSpanIR] = field(default_factory=list)
    baseline_y: float = 0.0

    def __post_init__(self):
        if not self.baseline_y and self.spans:
            self.baseline_y = self.spans[0].baseline_y
        elif not self.baseline_y:
            self.baseline_y = self.bbox.y1 - self.bbox.height * 0.15

    @property
    def text(self) -> str:
        return "".join(s.text for s in self.spans if s.text)

    @property
    def x0(self) -> float:
        return self.bbox.x0

    @property
    def x1(self) -> float:
        return self.bbox.x1

    @property
    def center_x(self) -> float:
        return self.bbox.center_x


@dataclass
class TextBlockIR:
    id: str
    bbox: Rect
    lines: List[TextLineIR] = field(default_factory=list)
    block_type: str = "paragraph"  # "heading", "paragraph", "list_item", "caption", "header", "footer"
    column_index: int = 0
    z_index: int = 0
    alignment: str = "LEFT"  # "LEFT", "CENTER", "RIGHT"
    exact_line_spacing_pt: Optional[float] = None
    parent_container_rect: Optional[Rect] = None
    is_single_line: bool = True
    orig_width: float = 0.0
    is_protected: bool = False
    dominant_font_size: float = 10.0

    def __post_init__(self):
        if not self.orig_width and self.bbox:
            self.orig_width = self.bbox.width
        if self.lines:
            sizes = {}
            for l in self.lines:
                for s in l.spans:
                    sz = round(safe_float(s.font.size if s.font else 10.0, 10.0), 1)
                    sizes[sz] = sizes.get(sz, 0) + len(s.text)
            if sizes:
                self.dominant_font_size = max(sizes.keys(), key=lambda k: sizes[k])

    @property
    def text(self) -> str:
        return "\n".join(l.text for l in self.lines if l.text)


@dataclass
class TableCellIR:
    text: str
    font: FontInfo
    bg_color: Optional[Tuple[int, int, int]] = None
    is_header: bool = False
    col_span: int = 1
    font_size: float = 9.5
    spans: List[TextSpanIR] = field(default_factory=list)

    def __post_init__(self):
        if not self.font_size:
            self.font_size = self.font.size if self.font else 9.5


@dataclass
class TableIR:
    id: str
    bbox: Rect
    rows: int
    cols: int
    col_widths: List[float] = field(default_factory=list)
    row_heights: List[float] = field(default_factory=list)
    cells: List[List[TableCellIR]] = field(default_factory=list)
    header_bg_rgb: Optional[Tuple[int, int, int]] = (88, 89, 91)
    z_index: int = 0


@dataclass
class CalloutIR:
    """Represents colored alert/callout banners (e.g. green NOTE boxes, warnings, code blocks)."""
    id: str
    bbox: Rect
    bg_color: Tuple[int, int, int]
    border_color: Optional[Tuple[int, int, int]] = None
    border_width: float = 0.75
    is_rounded: bool = True
    accent_bar_rect: Optional[Rect] = None
    accent_bar_color: Optional[Tuple[int, int, int]] = None
    text_blocks: List[TextBlockIR] = field(default_factory=list)
    z_index: int = 0

    def __post_init__(self):
        self.border_width = max(0.0, safe_float(self.border_width, 0.75))


@dataclass
class ShapeIR:
    id: str
    shape_type: str  # "line", "rect", "connector"
    bbox: Rect
    p1: Optional[Tuple[float, float]] = None
    p2: Optional[Tuple[float, float]] = None
    stroke_color: Optional[Tuple[int, int, int]] = None
    stroke_width: float = 0.75
    fill_color: Optional[Tuple[int, int, int]] = None
    z_index: int = 0

    def __post_init__(self):
        self.stroke_width = max(0.0, safe_float(self.stroke_width, 0.75))


@dataclass
class ImageIR:
    id: str
    bbox: Rect
    image_bytes: bytes
    image_format: str = "png"
    z_index: int = 0


@dataclass
class DiagramIR:
    """Complex non-deconstructible vector mesh preserved via high-res raster fallback."""
    id: str
    bbox: Rect
    image_bytes: bytes
    caption: Optional[str] = None
    element_count: int = 0
    z_index: int = 0


@dataclass
class PageIR:
    page_number: int
    width: float
    height: float
    rotation: int = 0
    text_blocks: List[TextBlockIR] = field(default_factory=list)
    callouts: List[CalloutIR] = field(default_factory=list)
    tables: List[TableIR] = field(default_factory=list)
    shapes: List[ShapeIR] = field(default_factory=list)
    images: List[ImageIR] = field(default_factory=list)
    diagrams: List[DiagramIR] = field(default_factory=list)
