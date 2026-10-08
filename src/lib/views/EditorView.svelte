<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    MousePointer, Type, PenLine, Highlighter, Square, Circle,
    ArrowUpRight, Minus, ShieldAlert, FileSignature, Undo2, Redo2,
    Trash2, ZoomIn, ZoomOut, Save, ChevronLeft,
    ChevronRight, Check, X, Plus, FileText, PanelLeftClose, PanelLeftOpen,
    Eraser, Sparkles, ChevronDown, ChevronUp, Printer, Search, Stamp,
    Maximize, Layers, Underline, Strikethrough, Copy
  } from "@lucide/svelte";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import SignaturePadModal from "../components/SignaturePadModal.svelte";
  import ConfirmModal from "../components/ConfirmModal.svelte";
  import SidebarPageThumb from "../components/SidebarPageThumb.svelte";
  import ContinuousPage from "../components/ContinuousPage.svelte";
  import PrintModal from "../components/PrintModal.svelte";
  import ExportConvertModal from "../components/ExportConvertModal.svelte";
  import PdfSearchBar, { type SearchMatch } from "../components/PdfSearchBar.svelte";
  import {
    openDocs, activeDocId, openPdfDocument, closePdfDocument,
    setPageAnnotations, markDocumentSaved, markDocumentModified, updateDocPage, updateDocScale,
    updateDocPath, type Annotation, type OpenDocument
  } from "../docStore";
  import {
    stampPageOverlays, removeWatermark, cleanWatermarkBytes, writePdfBytes,
    fillForm, baseName, type PageOverlayPayload
  } from "../api";
  import { openPdf, type PdfLink, type PdfFormField } from "../pdf";
  import { renderArrow, drawSingleAnnotation } from "../editor/render";
  import { calculateAnnotationsBoundingBox } from "../editor/geometry";

  import { pushToast, pendingFiles } from "../stores";

  import { logActivity } from "../activityStore";

  let showRemoveWatermarkModal = false;
  let isRemovingWatermark = false;

  $: if ($pendingFiles && $pendingFiles.targetView === "editor") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      openPdfDocument(pdfs[0]);
    }
    pendingFiles.set(null);
  }

  let sidebarWidth = 104; // Default width in px
  let isResizingSidebar = false;

  function startResizeSidebar(e: MouseEvent) {
    isResizingSidebar = true;
    const startX = e.clientX;
    const startWidth = sidebarWidth;

    function onMouseMove(ev: MouseEvent) {
      const delta = ev.clientX - startX;
      // Allow dynamic resizing from 80px up to 220px (providing up to +110% extra expansion)
      sidebarWidth = Math.max(80, Math.min(220, startWidth + delta));
    }

    function onMouseUp() {
      isResizingSidebar = false;
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    }

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  }

  type ToolMode =
    | "select"
    | "text"
    | "pen"
    | "highlighter"
    | "eraser"
    | "rect"
    | "circle"
    | "arrow"
    | "line"
    | "redact"
    | "signature";

  let busy = false;
  let showSidebar = true;
  let showSignatureModal = false;
  let showExportConvertModal = false;
  let showPrintModal = false;
  let showSearchBar = false;
  let activeSearchMatch: SearchMatch | null = null;
  let showSaveMenu = false;
  let showRedactionConfirmModal = false;
  let pendingSaveAs = false;
  let pendingCloseDocId: string | null = null;

  // Faz 2 Reader state
  let viewLayout: "single" | "continuous" = "single";
  let isReadingMode = false;
  let showZoomMenu = false;
  let textLayerDiv: HTMLDivElement | null = null;
  let currentLinks: PdfLink[] = [];
  let currentFormFields: PdfFormField[] = [];
  let docFormValues: Record<string, string> = {};
  let selectedText = "";

  let selectedTextRect: { left: number; top: number; width: number; height: number } | null = null;
  let visiblePageRange: { start: number; end: number } = { start: 1, end: 3 };
  let showHighlighterMenu = false;

  // Tools state - Default color for text is PURPLE (#9333ea) as requested!
  let currentTool: ToolMode = "select";
  let currentColor = "#9333ea";
  let currentFillColor: string = "transparent";
  let currentStrokeWidth = 3;
  let currentFontSize = 18;
  let currentOpacity = 100;

  // Split dropdown state for shape tools (Kutu, Daire, Ok, Çizgi)
  type ShapeToolType = "rect" | "circle" | "arrow" | "line";
  let activeShapeTool: ShapeToolType = "rect";
  let showShapeMenu = false;

  // Eraser menu state
  let showEraserMenu = false;

  // Style popover state (Adobe "dörtgen.png" style)
  let showStylePopover = false;
  let activeStyleTab: "stroke" | "fill" = "stroke";

  // Exact 21 swatches (7 cols x 3 rows) from Adobe screenshot ("dörtgen.png")
  const adobeSwatches: string[][] = [
    ["#1373e8", "#068a1c", "#ffc100", "#ff6200", "#db3425", "#c037c4", "#9643fc"],
    ["#38e5ff", "#c5fb72", "#fcf485", "#ffa97b", "#f86464", "#fb88ff", "#dcaaff"],
    ["#ffffff", "#cccccc", "#aaaaaa", "#767676", "#444444", "#333333", "#000000"],
  ];

  function selectShapeTool(shape: ShapeToolType) {
    activeShapeTool = shape;
    currentTool = shape;
    showShapeMenu = false;
  }

  function isDarkColor(hex: string): boolean {
    if (!hex || hex === "transparent") return false;
    const c = hex.replace("#", "");
    if (c.length !== 6) return false;
    const r = parseInt(c.substring(0, 2), 16);
    const g = parseInt(c.substring(2, 4), 16);
    const b = parseInt(c.substring(4, 6), 16);
    const yiq = (r * 299 + g * 587 + b * 114) / 1000;
    return yiq < 140;
  }

  $: stylePopoverTitle = (() => {
    if (selectedAnnotationIds.length > 0 && activeDoc) {
      const list = activeDoc.annotations[activeDoc.currentPage] || [];
      const item = list.find((a) => a.id === selectedAnnotationIds[selectedAnnotationIds.length - 1]);
      if (item?.type === "rect") return "Dikdörtgen rengi";
      if (item?.type === "circle") return "Daire rengi";
      if (item?.type === "arrow") return "Ok rengi";
      if (item?.type === "line") return "Çizgi rengi";
      if (item?.type === "text") return "Metin rengi";
      if (item?.type === "stroke") return "Kalem rengi";
    }
    if (currentTool === "rect") return "Dikdörtgen rengi";
    if (currentTool === "circle") return "Daire rengi";
    if (currentTool === "arrow") return "Ok rengi";
    if (currentTool === "line") return "Çizgi rengi";
    if (currentTool === "text") return "Metin rengi";
    if (currentTool === "pen") return "Kalem rengi";
    return "Stil ve Renk";
  })();

  function syncStyleFromSelected() {
    if (!activeDoc || selectedAnnotationIds.length === 0) return;
    const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
    const item = pageList.find((a) => a.id === selectedAnnotationIds[selectedAnnotationIds.length - 1]);
    if (!item) return;

    if (item.color) currentColor = item.color;
    if (item.fillColor) currentFillColor = item.fillColor;
    else currentFillColor = "transparent";
    if (item.strokeWidth) currentStrokeWidth = item.strokeWidth;
    if (item.opacity !== undefined) currentOpacity = Math.round(item.opacity * 100);
    else currentOpacity = 100;
    if (item.fontSize) currentFontSize = item.fontSize;

    if (item.type !== "rect" && item.type !== "circle") {
      activeStyleTab = "stroke";
    }
  }

  function updateSelectedAnnotationsStyle(updates: {
    color?: string;
    fillColor?: string | null;
    strokeWidth?: number;
    opacity?: number;
    fontSize?: number;
  }) {
    if (!activeDoc || selectedAnnotationIds.length === 0) return;
    const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
    const selectedSet = new Set(selectedAnnotationIds);
    let changed = false;

    pushHistory(activeDoc.id, activeDoc.currentPage);

    for (const item of pageList) {
      if (!selectedSet.has(item.id)) continue;
      if (updates.color !== undefined) item.color = updates.color;
      if (updates.fillColor !== undefined) {
        item.fillColor = updates.fillColor === "transparent" || !updates.fillColor ? undefined : updates.fillColor;
      }
      if (updates.strokeWidth !== undefined) item.strokeWidth = updates.strokeWidth;
      if (updates.opacity !== undefined) item.opacity = updates.opacity;
      if (updates.fontSize !== undefined && item.type === "text") item.fontSize = updates.fontSize;
      changed = true;
    }

    if (changed) {
      activeDoc.annotations[activeDoc.currentPage] = pageList;
      setPageAnnotations(activeDoc.id, activeDoc.currentPage, pageList);
      notifyPageThumbnailsUpdated(activeDoc.currentPage);
      redrawOverlay(pageList);
    }
  }

  function setStrokeColor(c: string) {
    currentColor = c;
    if (selectedAnnotationIds.length > 0) {
      updateSelectedAnnotationsStyle({ color: c });
    }
  }

  function setFillColor(c: string) {
    currentFillColor = c;
    if (selectedAnnotationIds.length > 0) {
      updateSelectedAnnotationsStyle({ fillColor: c });
    }
  }

  function setStrokeWidth(w: number) {
    currentStrokeWidth = w;
    if (selectedAnnotationIds.length > 0) {
      updateSelectedAnnotationsStyle({ strokeWidth: w });
    }
  }

  function setOpacity(val: number) {
    currentOpacity = val;
    if (selectedAnnotationIds.length > 0) {
      updateSelectedAnnotationsStyle({ opacity: val / 100 });
    }
  }

  function handleWindowClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target) return;
    if (!target.closest("#shape-dropdown-container")) {
      showShapeMenu = false;
    }
    if (!target.closest("#eraser-dropdown-container")) {
      showEraserMenu = false;
    }
    if (!target.closest("#style-popover-container")) {
      showStylePopover = false;
    }
    if (!target.closest("#zoom-dropdown-container")) {
      showZoomMenu = false;
    }
    if (!target.closest("#highlighter-dropdown-container")) {
      showHighlighterMenu = false;
    }
  }

  // Annotation Clipboard System (Ctrl+C / Ctrl+V across pages with offset)

  let annotationClipboard: Annotation[] = [];
  let pasteOffsetCount = 1;
  let lastDragPageTime = 0;
  let dragOverSidebarPage: number | null = null;
  let didPageChangeDuringDrag = false;

  // Smooth Page Transition & Badge State
  let pageTransitionDirection: "down" | "up" | null = null;
  let showPageChangeBadge = false;
  let pageChangeBadgeTimer: any = null;
  let pageTransitionClass = "";
  let pageTransitionTimer: any = null;

  function copySelectedAnnotations() {
    if (!activeDoc || selectedAnnotationIds.length === 0) return;
    const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
    const selected = pageList.filter((a) => selectedAnnotationIds.includes(a.id));
    if (selected.length === 0) return;

    annotationClipboard = selected.map((item) => ({
      ...JSON.parse(JSON.stringify(item)),
      imageEl: item.imageEl,
    }));
    pasteOffsetCount = 1;
    pushToast(`${annotationClipboard.length} nesne kopyalandı (Ctrl+V ile çoğaltın)`, "info");
  }

  function pasteAnnotations() {
    if (!activeDoc || annotationClipboard.length === 0) return;
    pushHistory(activeDoc.id, activeDoc.currentPage);

    const pageList = [...(activeDoc.annotations[activeDoc.currentPage] || [])];
    const offset = 24 * pasteOffsetCount;

    const maxW = (overlayCanvas?.width || 1200) / (activeDoc.scale || 1.0);
    const maxH = (overlayCanvas?.height || 1600) / (activeDoc.scale || 1.0);

    const pastedItems: Annotation[] = annotationClipboard.map((orig) => {
      const copy: Annotation = JSON.parse(JSON.stringify(orig));
      copy.id = Math.random().toString(36).slice(2);
      if (orig.imageEl) copy.imageEl = orig.imageEl;

      if (copy.x !== undefined && copy.y !== undefined) {
        copy.x = Math.min(Math.max(10, copy.x + offset), maxW - (copy.width || 40));
        copy.y = Math.min(Math.max(10, copy.y + offset), maxH - (copy.height || 40));
      } else if (copy.startX !== undefined && copy.startY !== undefined && copy.endX !== undefined && copy.endY !== undefined) {
        copy.startX += offset;
        copy.startY += offset;
        copy.endX += offset;
        copy.endY += offset;
      } else if (copy.points && copy.points.length > 0) {
        copy.points = copy.points.map((p) => ({
          x: Math.min(Math.max(10, p.x + offset), maxW - 10),
          y: Math.min(Math.max(10, p.y + offset), maxH - 10),
        }));
      }

      return copy;
    });

    pageList.push(...pastedItems);
    activeDoc.annotations[activeDoc.currentPage] = pageList;
    setPageAnnotations(activeDoc.id, activeDoc.currentPage, pageList);

    selectedAnnotationIds = pastedItems.map((a) => a.id);
    syncStyleFromSelected();
    notifyPageThumbnailsUpdated(activeDoc.currentPage);
    redrawOverlay(pageList);

    pasteOffsetCount++;
    pushToast(`${pastedItems.length} nesne çoğaltıldı`, "success");
  }

  function transferSelectedToPage(targetPage: number, placement: "current" | "top" | "bottom" = "current") {
    if (!activeDoc || selectedAnnotationIds.length === 0 || targetPage === activeDoc.currentPage) return;
    if (targetPage < 1 || targetPage > activeDoc.pageCount) return;

    const sourcePage = activeDoc.currentPage;
    const sourceList = activeDoc.annotations[sourcePage] || [];
    const selectedSet = new Set(selectedAnnotationIds);

    const itemsToMove = sourceList.filter((a) => selectedSet.has(a.id));
    if (itemsToMove.length === 0) return;

    pushHistory(activeDoc.id, sourcePage);
    const newSourceList = sourceList.filter((a) => !selectedSet.has(a.id));
    activeDoc.annotations[sourcePage] = newSourceList;
    setPageAnnotations(activeDoc.id, sourcePage, newSourceList);
    notifyPageThumbnailsUpdated(sourcePage);

    if (placement === "top") {
      let minY = Infinity;
      for (const it of itemsToMove) {
        const y = it.y ?? it.startY ?? (it.points ? Math.min(...it.points.map((p) => p.y)) : 0);
        if (y < minY) minY = y;
      }
      const shiftY = 30 - (minY === Infinity ? 0 : minY);
      for (const it of itemsToMove) {
        if (it.y !== undefined) it.y += shiftY;
        if (it.startY !== undefined) it.startY += shiftY;
        if (it.endY !== undefined) it.endY += shiftY;
        if (it.points) it.points.forEach((p) => (p.y += shiftY));
      }
    } else if (placement === "bottom") {
      const pageH = (overlayCanvas?.height || 1000) / (activeDoc.scale || 1.0);
      let maxY = -Infinity;
      for (const it of itemsToMove) {
        const y = (it.y ?? 0) + (it.height ?? 0);
        if (y > maxY) maxY = y;
      }
      const shiftY = pageH - 60 - (maxY <= 0 ? 0 : maxY);
      for (const it of itemsToMove) {
        if (it.y !== undefined) it.y += shiftY;
        if (it.startY !== undefined) it.startY += shiftY;
        if (it.endY !== undefined) it.endY += shiftY;
        if (it.points) it.points.forEach((p) => (p.y += shiftY));
      }
    }

    pushHistory(activeDoc.id, targetPage);
    const targetList = [...(activeDoc.annotations[targetPage] || []), ...itemsToMove];
    activeDoc.annotations[targetPage] = targetList;
    setPageAnnotations(activeDoc.id, targetPage, targetList);

    didPageChangeDuringDrag = true;
    const dir = targetPage > sourcePage ? "down" : "up";
    setPage(targetPage, true, dir);
    selectedAnnotationIds = itemsToMove.map((a) => a.id);
    syncStyleFromSelected();
    notifyPageThumbnailsUpdated(targetPage);
    pushToast(`Nesneler Sayfa ${targetPage}'e taşındı`, "info");
  }

  // Active doc derived from docStore
  $: activeDoc = $openDocs.find((d) => d.id === $activeDocId) || null;

  // Canvas references
  let pdfCanvas: HTMLCanvasElement;
  let overlayCanvas: HTMLCanvasElement;
  let overlayCtx: CanvasRenderingContext2D | null = null;
  let canvasContainer: HTMLDivElement;

  // Active interaction state
  let isPointerDown = false;
  let startX = 0;
  let startY = 0;
  let currentX = 0;
  let currentY = 0;
  let currentStrokePoints: { x: number; y: number }[] = [];

  // Text input placement state (Canvas & CSS coordinate separation)
  let textInputVisible = false;
  let textInputCanvasX = 0;
  let textInputCanvasY = 0;
  let textInputCssX = 0;
  let textInputCssY = 0;
  let textInputVal = "";
  let textInputRef: HTMLInputElement;

  // Selected items on canvas for moving/resizing (supports CTRL + Left Click multi-select)
  let selectedAnnotationIds: string[] = [];
  $: selectedAnnotationId = selectedAnnotationIds.length === 1 ? selectedAnnotationIds[0] : null;
  let isDraggingItem = false;
  let isHoveringItem = false;
  let dragOffsetX = 0;
  let dragOffsetY = 0;
  let dragInitialSnapshot: Annotation[] | null = null;
  let didMoveItem = false;

  // 8-Point Reference Resize Handle System (matches reference UI)
  type ResizeHandleType = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

  interface ResizeHandle {
    type: ResizeHandleType;
    x: number;
    y: number;
    cursor: string;
  }

  let hoverResizeHandle: ResizeHandle | null = null;
  let activeResizeHandle: ResizeHandleType | null = null;
  let isResizingItem = false;
  let didResizeItem = false;
  let resizeStartPos = { x: 0, y: 0 };
  let resizeInitialBounds: { x: number; y: number; width: number; height: number } | null = null;
  let resizeInitialSnapshot: Annotation[] | null = null;

  function getResizeHandles(bounds: { x: number; y: number; width: number; height: number }): ResizeHandle[] {
    const { x, y, width, height } = bounds;
    const midX = x + width / 2;
    const midY = y + height / 2;
    const right = x + width;
    const bottom = y + height;

    return [
      { type: "nw", x, y, cursor: "nwse-resize" },
      { type: "n", x: midX, y, cursor: "ns-resize" },
      { type: "ne", x: right, y, cursor: "nesw-resize" },
      { type: "e", x: right, y: midY, cursor: "ew-resize" },
      { type: "se", x: right, y: bottom, cursor: "nwse-resize" },
      { type: "s", x: midX, y: bottom, cursor: "ns-resize" },
      { type: "sw", x, y: bottom, cursor: "nesw-resize" },
      { type: "w", x, y: midY, cursor: "ew-resize" },
    ];
  }

  function findHitResizeHandle(
    pos: { x: number; y: number },
    bounds: { x: number; y: number; width: number; height: number },
    scale: number
  ): ResizeHandle | null {
    const handles = getResizeHandles(bounds);
    const tolerance = 9 / scale;
    for (const h of handles) {
      if (Math.hypot(pos.x - h.x, pos.y - h.y) <= tolerance) {
        return h;
      }
    }
    return null;
  }

  // Live sidebar thumbnail synchronization versions
  let docVersion = 0;
  let pageVersions: Record<number, number> = {};

  function notifyDocThumbnailsUpdated() {
    docVersion++;
  }

  function notifyPageThumbnailsUpdated(page: number) {
    pageVersions = { ...pageVersions, [page]: (pageVersions[page] || 0) + 1 };
  }

  // Middle-mouse button free-pan (scroll wheel hold) across all tools
  let isMiddlePanning = false;
  let middlePanStartX = 0;
  let middlePanStartY = 0;
  let middlePanStartScrollLeft = 0;
  let middlePanStartScrollTop = 0;

  function handleMiddlePanMouseDown(e: MouseEvent) {
    if (e.button === 1) {
      e.preventDefault();
      e.stopPropagation();
      isMiddlePanning = true;
      middlePanStartX = e.clientX;
      middlePanStartY = e.clientY;
      if (canvasContainer) {
        middlePanStartScrollLeft = canvasContainer.scrollLeft;
        middlePanStartScrollTop = canvasContainer.scrollTop;
      }
      window.addEventListener("mousemove", handleMiddlePanMouseMove, { passive: false });
      window.addEventListener("mouseup", handleMiddlePanMouseUp, { passive: false });
    }
  }

  function handleMiddlePanMouseMove(e: MouseEvent) {
    if (!isMiddlePanning || !canvasContainer) return;
    e.preventDefault();
    const dx = e.clientX - middlePanStartX;
    const dy = e.clientY - middlePanStartY;
    canvasContainer.scrollLeft = middlePanStartScrollLeft - dx;
    canvasContainer.scrollTop = middlePanStartScrollTop - dy;
  }

  function handleMiddlePanMouseUp(e: MouseEvent) {
    if (e.button === 1 || isMiddlePanning) {
      if (isMiddlePanning) {
        e.preventDefault();
        e.stopPropagation();
      }
      isMiddlePanning = false;
      window.removeEventListener("mousemove", handleMiddlePanMouseMove);
      window.removeEventListener("mouseup", handleMiddlePanMouseUp);
    }
  }

  function handleContainerMouseDown(e: MouseEvent) {
    if (e.button === 1) {
      handleMiddlePanMouseDown(e);
    }
  }

  function preventAuxDefault(e: MouseEvent) {
    if (e.button === 1) e.preventDefault();
  }

  onMount(() => {
    window.addEventListener("auxclick", preventAuxDefault, { passive: false });
    window.addEventListener("click", handleWindowClick);
    document.addEventListener("selectionchange", handleTextSelectionChange);
  });

  onDestroy(() => {
    window.removeEventListener("auxclick", preventAuxDefault);
    window.removeEventListener("click", handleWindowClick);
    document.removeEventListener("selectionchange", handleTextSelectionChange);
    window.removeEventListener("mousemove", handleMiddlePanMouseMove);
    window.removeEventListener("mouseup", handleMiddlePanMouseUp);
    window.removeEventListener("mousemove", onWindowMouseMove);
    window.removeEventListener("mouseup", onWindowMouseUp);
    if (pageChangeBadgeTimer) clearTimeout(pageChangeBadgeTimer);
    if (pageTransitionTimer) clearTimeout(pageTransitionTimer);
  });

  $: canvasCursor = (() => {
    if (isMiddlePanning) return "grabbing";
    if (currentTool === "select") {
      if (isResizingItem && activeResizeHandle) {
        const map: Record<ResizeHandleType, string> = {
          nw: "nwse-resize",
          n: "ns-resize",
          ne: "nesw-resize",
          e: "ew-resize",
          se: "nwse-resize",
          s: "ns-resize",
          sw: "nesw-resize",
          w: "ew-resize",
        };
        return map[activeResizeHandle] || "nwse-resize";
      }
      if (hoverResizeHandle) return hoverResizeHandle.cursor;
      if (isDraggingItem) return "grabbing";
      if (isHoveringItem) return "move";
      return "default";
    }
    if (currentTool === "eraser") {
      return eraserMode === "object" ? "crosshair" : "none";
    }
    if (currentTool === "text") return "text";
    return "crosshair";
  })();

  $: overlayPointerEvents =
    currentTool !== "select" ||
    isDraggingItem ||
    isResizingItem ||
    isHoveringItem ||
    hoverResizeHandle ||
    selectedAnnotationIds.length > 0
      ? "auto"
      : "none";

  function handleWrapperMouseMove(e: MouseEvent) {
    if (currentTool !== "select") return;
    if (!activeDoc || !overlayCanvas) return;
    const pos = getCanvasPos(e);
    const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
    hoverResizeHandle = null;
    if (selectedAnnotationIds.length === 1) {
      const selectedItem = pageList.find((a) => a.id === selectedAnnotationIds[0]);
      if (selectedItem) {
        const bounds = getAnnotationBounds(selectedItem);
        if (bounds) {
          const hitHandle = findHitResizeHandle(pos, bounds, activeDoc.scale);
          if (hitHandle) {
            hoverResizeHandle = hitHandle;
            isHoveringItem = false;
            return;
          }
        }
      }
    }
    const hit = findHitAnnotation(pos, pageList);
    isHoveringItem = !!hit;
  }

  // Eraser state: "object" (Bölgesel), "precision" (Hassas - çizim silici), "whiteout" (Daksil - arka plan da siler)
  let eraserMode: "object" | "precision" | "whiteout" = "object";
  let eraserSize = 24;
  let eraserClientPos: { x: number; y: number } | null = null;
  let eraserDisplayDiameter = 24;
  let isMouseOverCanvas = false;
  let erasedAnyInStroke = false;

  // Fluid wheel scroll pagination state
  let wheelAccumulator = 0;
  let lastPageChangeTime = 0;

  // Undo / Redo History Stacks
  interface HistorySnapshot {
    page: number;
    annotations: Annotation[];
    type?: "annotations" | "watermark";
    cleanPdfBytes?: Uint8Array | null;
    wasUnsaved?: boolean;
  }

  let undoStacks: Record<string, HistorySnapshot[]> = {};
  let redoStacks: Record<string, HistorySnapshot[]> = {};
  let historyVersion = 0;

  $: hasUndo = !!(historyVersion >= 0 && activeDoc && undoStacks[activeDoc.id] && undoStacks[activeDoc.id].length > 0);
  $: hasRedo = !!(historyVersion >= 0 && activeDoc && redoStacks[activeDoc.id] && redoStacks[activeDoc.id].length > 0);

  // Colors presets with PURPLE first!
  const penColors = ["#9333ea", "#0f172a", "#2563eb", "#dc2626", "#16a34a"];
  const highlighterColors = ["rgba(250, 204, 21, 0.45)", "rgba(74, 222, 128, 0.45)", "rgba(56, 189, 248, 0.45)", "rgba(244, 114, 182, 0.45)"];

  // Whenever activeDoc or activeDoc.currentPage or activeDoc.scale changes, re-render
  let renderedDocId: string | null = null;
  let renderedPage = 0;
  let renderedScale = 0;

  $: if (activeDoc) {
    if (
      activeDoc.id !== renderedDocId ||
      activeDoc.currentPage !== renderedPage ||
      activeDoc.scale !== renderedScale
    ) {
      renderedDocId = activeDoc.id;
      renderedPage = activeDoc.currentPage;
      renderedScale = activeDoc.scale;
      tick().then(renderCurrentPage);
    }
  }

  async function openNewFileDialog() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (typeof selected === "string") {
      await openPdfDocument(selected);
    }
  }

  function handleFileDrop(paths: string[]) {
    if (paths.length > 0) {
      openPdfDocument(paths[0]);
    }
  }

  function handleCloseTab(docId: string, e?: MouseEvent) {
    e?.stopPropagation();
    const doc = $openDocs.find((d) => d.id === docId);
    if (doc?.hasUnsavedChanges) {
      pendingCloseDocId = docId;
    } else {
      closePdfDocument(docId);
    }
  }

  function confirmCloseTab() {
    if (pendingCloseDocId) {
      closePdfDocument(pendingCloseDocId);
      pendingCloseDocId = null;
    }
  }

  async function renderCurrentPage() {
    if (!activeDoc || !pdfCanvas || !overlayCanvas) return;
    try {
      const rendered = await activeDoc.handle.renderPage(activeDoc.currentPage, activeDoc.scale);
      pdfCanvas.width = rendered.width;
      pdfCanvas.height = rendered.height;
      const ctx = pdfCanvas.getContext("2d")!;
      ctx.drawImage(rendered, 0, 0);

      overlayCanvas.width = rendered.width;
      overlayCanvas.height = rendered.height;
      overlayCtx = overlayCanvas.getContext("2d");
      redrawOverlay();

      if (textLayerDiv) {
        textLayerDiv.style.width = `${rendered.width}px`;
        textLayerDiv.style.height = `${rendered.height}px`;
        try {
          await activeDoc.handle.renderTextLayer(activeDoc.currentPage, activeDoc.scale, textLayerDiv);
        } catch (te) {
          console.warn("renderTextLayer error:", te);
        }
      }

      try {
        currentLinks = await activeDoc.handle.getPageLinks(activeDoc.currentPage, activeDoc.scale);
      } catch {
        currentLinks = [];
      }
      try {
        currentFormFields = await activeDoc.handle.getPageFormFields(activeDoc.currentPage, activeDoc.scale);
      } catch {
        currentFormFields = [];
      }
    } catch (e) {

      console.error("renderCurrentPage error:", e);
    }
  }

  function setPage(p: number, keepSelection: boolean = false, customDirection?: "down" | "up") {
    if (!activeDoc) return;
    if (p < 1 || p > activeDoc.pageCount) return;
    if (p === activeDoc.currentPage && !customDirection) return;

    const dir = customDirection || (p > activeDoc.currentPage ? "down" : "up");
    pageTransitionDirection = dir;

    // Trigger smooth slide transition
    pageTransitionClass = dir === "down" ? "translate-y-2 opacity-80" : "-translate-y-2 opacity-80";
    if (pageTransitionTimer) clearTimeout(pageTransitionTimer);
    pageTransitionTimer = setTimeout(() => {
      pageTransitionClass = "translate-y-0 opacity-100";
    }, 25);

    // Show floating page badge pill
    showPageChangeBadge = true;
    if (pageChangeBadgeTimer) clearTimeout(pageChangeBadgeTimer);
    pageChangeBadgeTimer = setTimeout(() => {
      showPageChangeBadge = false;
    }, 900);

    updateDocPage(activeDoc.id, p);
    textInputVisible = false;
    if (!keepSelection) {
      selectedAnnotationIds = [];
    }
    pasteOffsetCount = 1;
  }

  function changeZoom(delta: number) {
    if (!activeDoc) return;
    const next = Math.max(0.5, Math.min(3.0, activeDoc.scale + delta));
    updateDocScale(activeDoc.id, Math.round(next * 10) / 10);
  }

  function resetZoom() {
    if (!activeDoc) return;
    updateDocScale(activeDoc.id, 1.0);
    showZoomMenu = false;
  }

  function fitToWidth() {
    if (!activeDoc || !canvasContainer || !pdfCanvas) return;
    const containerW = canvasContainer.clientWidth - 48;
    const pageW = pdfCanvas.width / activeDoc.scale;
    if (pageW > 0) {
      const next = Math.max(0.3, Math.min(3.5, containerW / pageW));
      updateDocScale(activeDoc.id, Math.round(next * 100) / 100);
      showZoomMenu = false;
    }
  }

  function fitToPage() {
    if (!activeDoc || !canvasContainer || !pdfCanvas) return;
    const containerW = canvasContainer.clientWidth - 48;
    const containerH = canvasContainer.clientHeight - 80;
    const pageW = pdfCanvas.width / activeDoc.scale;
    const pageH = pdfCanvas.height / activeDoc.scale;
    if (pageW > 0 && pageH > 0) {
      const scaleW = containerW / pageW;
      const scaleH = containerH / pageH;
      const next = Math.max(0.3, Math.min(3.5, Math.min(scaleW, scaleH)));
      updateDocScale(activeDoc.id, Math.round(next * 100) / 100);
      showZoomMenu = false;
    }
  }

  function setZoomPreset(val: number) {
    if (!activeDoc) return;
    updateDocScale(activeDoc.id, val);
    showZoomMenu = false;
  }

  async function toggleReadingMode() {
    isReadingMode = !isReadingMode;
    try {
      await getCurrentWindow().setFullscreen(isReadingMode);
    } catch {}
  }

  function handleLinkClick(link: PdfLink, e?: MouseEvent) {
    e?.preventDefault();
    e?.stopPropagation();
    if (link.destPage && link.destPage >= 1 && activeDoc && link.destPage <= activeDoc.pageCount) {
      setPage(link.destPage);
      if (viewLayout === "continuous") {
        document.getElementById(`continuous-page-${link.destPage}`)?.scrollIntoView({ behavior: "smooth" });
      }
      pushToast({ message: `Sayfa ${link.destPage}'e gidildi`, type: "info" });
    } else if (link.url) {
      window.open(link.url, "_blank");
    }
  }

  function handleTextSelectionChange() {
    if (currentTool !== "select") {
      selectedText = "";
      selectedTextRect = null;
      return;
    }
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !sel.toString().trim()) {
      selectedText = "";
      selectedTextRect = null;
      return;
    }
    const range = sel.getRangeAt(0);
    const rect = range.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      selectedText = sel.toString().trim();
      selectedTextRect = {
        left: rect.left + rect.width / 2,
        top: rect.top - 8,
        width: rect.width,
        height: rect.height,
      };
    }
  }

  async function copySelectedText() {
    if (!selectedText) return;
    try {
      await navigator.clipboard.writeText(selectedText);
      pushToast({ message: "Metin kopyalandı", type: "success" });
    } catch {}
  }

  function applyTextAction(action: "highlight" | "underline" | "strikethrough") {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !overlayCanvas || !activeDoc) return;
    const range = sel.getRangeAt(0);
    const clientRects = Array.from(range.getClientRects());
    if (clientRects.length === 0) return;

    const canvasRect = overlayCanvas.getBoundingClientRect();
    const s = activeDoc.scale;
    const page = activeDoc.currentPage;
    pushHistory(activeDoc.id, page);

    const newAnns: Annotation[] = [];
    for (const cr of clientRects) {
      const x = (cr.left - canvasRect.left) / s;
      const y = (cr.top - canvasRect.top) / s;
      const w = cr.width / s;
      const h = cr.height / s;
      if (w <= 0 || h <= 0) continue;

      if (action === "highlight") {
        newAnns.push({
          id: `ann_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
          type: "rect",
          x,
          y,
          width: w,
          height: h,
          color: "transparent",
          fillColor: currentColor && currentColor.startsWith("rgba") ? currentColor : "rgba(250, 204, 21, 0.45)",
          opacity: 0.85,
        });
      } else if (action === "underline") {
        newAnns.push({
          id: `ann_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
          type: "line",
          startX: x,
          startY: y + h - 1,
          endX: x + w,
          endY: y + h - 1,
          color: currentColor && currentColor !== "transparent" ? currentColor : "#dc2626",
          strokeWidth: 2,
          opacity: 1,
        });
      } else if (action === "strikethrough") {
        newAnns.push({
          id: `ann_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
          type: "line",
          startX: x,
          startY: y + h / 2,
          endX: x + w,
          endY: y + h / 2,
          color: currentColor && currentColor !== "transparent" ? currentColor : "#dc2626",
          strokeWidth: 2,
          opacity: 1,
        });
      }
    }

    if (newAnns.length > 0) {
      const list = [...(activeDoc.annotations[page] || []), ...newAnns];
      activeDoc.annotations[page] = list;
      setPageAnnotations(activeDoc.id, page, list);
      notifyPageThumbnailsUpdated(page);
      redrawOverlay(list);
      sel.removeAllRanges();
      selectedText = "";
      selectedTextRect = null;
      pushToast({
        message: action === "highlight" ? "Metin vurgulandı" : action === "underline" ? "Metnin altı çizildi" : "Metnin üstü çizildi",
        type: "success",
      });
    }
  }

  function handleContinuousScroll() {
    if (viewLayout !== "continuous" || !canvasContainer || !activeDoc) return;
    const scrollTop = canvasContainer.scrollTop;
    const viewportHeight = canvasContainer.clientHeight;
    const centerY = scrollTop + viewportHeight / 2;

    let closestPage = activeDoc.currentPage;
    let minDistance = Infinity;

    for (let p = 1; p <= activeDoc.pageCount; p++) {
      const el = document.getElementById(`continuous-page-${p}`);
      if (el) {
        const top = el.offsetTop;
        const h = el.offsetHeight;
        const pageCenter = top + h / 2;
        const dist = Math.abs(pageCenter - centerY);
        if (dist < minDistance) {
          minDistance = dist;
          closestPage = p;
        }
      }
    }

    if (closestPage !== activeDoc.currentPage) {
      updateDocPage(activeDoc.id, closestPage);
    }

    visiblePageRange = {
      start: Math.max(1, closestPage - 2),
      end: Math.min(activeDoc.pageCount, closestPage + 2),
    };
  }

  // --- Interaction Handlers ---

  function getCanvasPos(e: MouseEvent): { x: number; y: number } {
    const rect = overlayCanvas.getBoundingClientRect();
    const factorX = overlayCanvas.width / rect.width;
    const factorY = overlayCanvas.height / rect.height;
    const px = (e.clientX - rect.left) * factorX;
    const py = (e.clientY - rect.top) * factorY;
    const s = activeDoc?.scale || 1.0;
    return {
      x: px / s,
      y: py / s,
    };
  }

  let lastMoveX = 0;
  let lastMoveY = 0;

  function distanceToSegment(px: number, py: number, x1: number, y1: number, x2: number, y2: number): number {
    const l2 = (x2 - x1) ** 2 + (y2 - y1) ** 2;
    if (l2 === 0) return Math.hypot(px - x1, py - y1);
    let t = ((px - x1) * (x2 - x1) + (py - y1) * (y2 - y1)) / l2;
    t = Math.max(0, Math.min(1, t));
    return Math.hypot(px - (x1 + t * (x2 - x1)), py - (y1 + t * (y2 - y1)));
  }

  function getAnnotationBounds(item: Annotation): { x: number; y: number; width: number; height: number } | null {
    if (item.type === "rect" || item.type === "redact" || item.type === "image" || item.type === "text" || item.type === "circle") {
      return {
        x: item.x ?? 0,
        y: item.y ?? 0,
        width: item.width ?? 40,
        height: item.height ?? 30,
      };
    }
    if (item.type === "arrow" || item.type === "line") {
      const minX = Math.min(item.startX ?? 0, item.endX ?? 0);
      const minY = Math.min(item.startY ?? 0, item.endY ?? 0);
      const maxX = Math.max(item.startX ?? 0, item.endX ?? 0);
      const maxY = Math.max(item.startY ?? 0, item.endY ?? 0);
      return {
        x: minX,
        y: minY,
        width: Math.max(12, maxX - minX),
        height: Math.max(12, maxY - minY),
      };
    }
    if (item.type === "stroke" || item.type === "highlighter") {
      if (!item.points || item.points.length === 0) return null;
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const p of item.points) {
        if (p.x < minX) minX = p.x;
        if (p.x > maxX) maxX = p.x;
        if (p.y < minY) minY = p.y;
        if (p.y > maxY) maxY = p.y;
      }
      return {
        x: minX,
        y: minY,
        width: Math.max(12, maxX - minX),
        height: Math.max(12, maxY - minY),
      };
    }
    return null;
  }

  function getDistanceToAnnotationEdge(item: Annotation, px: number, py: number): number {
    if (item.type === "line" || item.type === "arrow") {
      return distanceToSegment(px, py, item.startX ?? 0, item.startY ?? 0, item.endX ?? 0, item.endY ?? 0);
    }
    if (item.type === "rect") {
      const x = item.x ?? 0;
      const y = item.y ?? 0;
      const w = item.width ?? 40;
      const h = item.height ?? 30;
      const dTop = distanceToSegment(px, py, x, y, x + w, y);
      const dBottom = distanceToSegment(px, py, x, y + h, x + w, y + h);
      const dLeft = distanceToSegment(px, py, x, y, x, y + h);
      const dRight = distanceToSegment(px, py, x + w, y, x + w, y + h);
      return Math.min(dTop, dBottom, dLeft, dRight);
    }
    if (item.type === "circle") {
      const x = item.x ?? 0;
      const y = item.y ?? 0;
      const w = item.width ?? 40;
      const h = item.height ?? 30;
      const cx = x + w / 2;
      const cy = y + h / 2;
      const rx = w / 2;
      const ry = h / 2;
      const avgR = (rx + ry) / 2;
      const distFromCenter = Math.hypot(px - cx, py - cy);
      return Math.abs(distFromCenter - avgR);
    }
    if (item.type === "stroke" || item.type === "highlighter") {
      if (!item.points || item.points.length === 0) return Infinity;
      let minDist = Infinity;
      for (let i = 0; i < item.points.length; i++) {
        const p = item.points[i];
        const d = Math.hypot(p.x - px, p.y - py);
        if (d < minDist) minDist = d;
        if (i > 0) {
          const segD = distanceToSegment(px, py, item.points[i - 1].x, item.points[i - 1].y, p.x, p.y);
          if (segD < minDist) minDist = segD;
        }
      }
      return minDist;
    }
    if (item.type === "redact" || item.type === "image" || item.type === "text") {
      const x = item.x ?? 0;
      const y = item.y ?? 0;
      const w = item.width ?? 40;
      const h = item.height ?? 30;
      if (px >= x && px <= x + w && py >= y && py <= y + h) return 0;
      const dTop = distanceToSegment(px, py, x, y, x + w, y);
      const dBottom = distanceToSegment(px, py, x, y + h, x + w, y + h);
      const dLeft = distanceToSegment(px, py, x, y, x, y + h);
      const dRight = distanceToSegment(px, py, x + w, y, x + w, y + h);
      return Math.min(dTop, dBottom, dLeft, dRight);
    }
    return Infinity;
  }

  function findHitAnnotation(pos: { x: number; y: number }, list: Annotation[]): Annotation | null {
    let bestItem: Annotation | null = null;
    let minDistance = 12;

    for (let i = list.length - 1; i >= 0; i--) {
      const a = list[i];
      const dist = getDistanceToAnnotationEdge(a, pos.x, pos.y);
      if (dist < minDistance) {
        minDistance = dist;
        bestItem = a;
      }
    }
    return bestItem;
  }

  function pushHistory(docId: string, page: number) {
    if (!activeDoc) return;
    const list = activeDoc.annotations[page] || [];
    const snapshot: Annotation[] = list.map((a) => ({
      ...a,
      points: a.points ? a.points.map((p) => ({ ...p })) : undefined,
    }));

    if (!undoStacks[docId]) undoStacks[docId] = [];
    undoStacks[docId].push({ page, annotations: snapshot });
    if (undoStacks[docId].length > 40) undoStacks[docId].shift();

    redoStacks[docId] = [];
    historyVersion++;
  }

  function eraseObjectAtPoint(pos: { x: number; y: number }) {
    if (!activeDoc) return;
    const currentList = activeDoc.annotations[activeDoc.currentPage] || [];
    if (currentList.length === 0) return;

    let bestItem: Annotation | null = null;
    let minDistance = 16; // 16px outline hit tolerance

    for (let i = currentList.length - 1; i >= 0; i--) {
      const item = currentList[i];
      const dist = getDistanceToAnnotationEdge(item, pos.x, pos.y);
      if (dist <= minDistance) {
        minDistance = dist;
        bestItem = item;
      }
    }

    if (bestItem) {
      if (!erasedAnyInStroke) {
        pushHistory(activeDoc.id, activeDoc.currentPage);
        erasedAnyInStroke = true;
      }
      const remaining = currentList.filter((a) => a.id !== bestItem!.id);
      activeDoc.annotations[activeDoc.currentPage] = remaining;
      setPageAnnotations(activeDoc.id, activeDoc.currentPage, remaining);
      if (selectedAnnotationIds.includes(bestItem.id)) {
        selectedAnnotationIds = selectedAnnotationIds.filter((id) => id !== bestItem.id);
      }
      notifyPageThumbnailsUpdated(activeDoc.currentPage);
      redrawOverlay(remaining);
    }
  }

  function handleContainerWheel(e: WheelEvent) {
    if (!activeDoc || activeDoc.pageCount <= 1) return;

    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      changeZoom(e.deltaY < 0 ? 0.1 : -0.1);
      return;
    }

    if (isDraggingItem && selectedAnnotationIds.length > 0 && activeDoc) {
      e.preventDefault();
      const now = Date.now();
      if (now - lastDragPageTime < 400) return;
      if (e.deltaY > 0 && activeDoc.currentPage < activeDoc.pageCount) {
        lastDragPageTime = now;
        transferSelectedToPage(activeDoc.currentPage + 1, "top");
      } else if (e.deltaY < 0 && activeDoc.currentPage > 1) {
        lastDragPageTime = now;
        transferSelectedToPage(activeDoc.currentPage - 1, "bottom");
      }
      return;
    }

    if (!canvasContainer) return;

    const isAtBottom = canvasContainer.scrollTop + canvasContainer.clientHeight >= canvasContainer.scrollHeight - 25;
    const isAtTop = canvasContainer.scrollTop <= 25;

    const now = Date.now();
    if (now - lastPageChangeTime < 350) return;

    if (e.deltaY > 0 && isAtBottom && activeDoc.currentPage < activeDoc.pageCount) {
      wheelAccumulator += e.deltaY;
      if (wheelAccumulator > 70) {
        e.preventDefault();
        wheelAccumulator = 0;
        lastPageChangeTime = now;
        setPage(activeDoc.currentPage + 1);
        tick().then(() => {
          if (canvasContainer) canvasContainer.scrollTop = 0;
        });
      }
    } else if (e.deltaY < 0 && isAtTop && activeDoc.currentPage > 1) {
      wheelAccumulator += e.deltaY;
      if (wheelAccumulator < -70) {
        e.preventDefault();
        wheelAccumulator = 0;
        lastPageChangeTime = now;
        setPage(activeDoc.currentPage - 1);
        tick().then(() => {
          if (canvasContainer) canvasContainer.scrollTop = canvasContainer.scrollHeight;
        });
      }
    } else {
      wheelAccumulator = 0;
    }
  }

  function onMouseDown(e: MouseEvent) {
    if (e.button === 1) {
      handleMiddlePanMouseDown(e);
      return;
    }
    if (e.button !== 0) return;
    if (!activeDoc || !overlayCanvas) return;
    const pos = getCanvasPos(e);
    isPointerDown = true;
    startX = pos.x;
    startY = pos.y;
    currentX = pos.x;
    currentY = pos.y;

    if (currentTool === "select") {
      const pageList = activeDoc.annotations[activeDoc.currentPage] || [];

      // 1. Check if clicking on one of the 8 resize handles of selected item (only when 1 item selected)
      if (selectedAnnotationIds.length === 1) {
        const selectedItem = pageList.find((a) => a.id === selectedAnnotationIds[0]);
        if (selectedItem) {
          const bounds = getAnnotationBounds(selectedItem);
          if (bounds) {
            const hitHandle = findHitResizeHandle(pos, bounds, activeDoc.scale);
            if (hitHandle) {
              isResizingItem = true;
              activeResizeHandle = hitHandle.type;
              didResizeItem = false;
              resizeStartPos = { x: pos.x, y: pos.y };
              resizeInitialBounds = { ...bounds };
              resizeInitialSnapshot = pageList.map((a) => ({
                ...a,
                points: a.points ? a.points.map((p) => ({ ...p })) : undefined,
              }));
              window.addEventListener("mousemove", onWindowMouseMove, { passive: false });
              window.addEventListener("mouseup", onWindowMouseUp, { passive: false });
              return;
            }
          }
        }
      }

      // 2. Check if clicking on an annotation body / inside bounds
      const clicked = findHitAnnotation(pos, pageList);
      if (clicked) {
        if (e.ctrlKey || e.metaKey) {
          // Toggle selection with CTRL + Left click
          if (selectedAnnotationIds.includes(clicked.id)) {
            selectedAnnotationIds = selectedAnnotationIds.filter((id) => id !== clicked.id);
          } else {
            selectedAnnotationIds = [...selectedAnnotationIds, clicked.id];
          }
        } else {
          // Standard click without CTRL:
          // If clicking an item that's already in the multi-selection group, keep group selected to allow dragging all
          if (!selectedAnnotationIds.includes(clicked.id)) {
            selectedAnnotationIds = [clicked.id];
          }
        }

        if (selectedAnnotationIds.length > 0) {
          syncStyleFromSelected();
          isDraggingItem = true;
          didMoveItem = false;
          dragInitialSnapshot = pageList.map((a) => ({
            ...a,
            points: a.points ? a.points.map((p) => ({ ...p })) : undefined,
          }));
          lastMoveX = pos.x;
          lastMoveY = pos.y;
          window.addEventListener("mousemove", onWindowMouseMove, { passive: false });
          window.addEventListener("mouseup", onWindowMouseUp, { passive: false });
        } else {
          isDraggingItem = false;
        }
      } else {
        if (!e.ctrlKey && !e.metaKey) {
          selectedAnnotationIds = [];
        }
        isDraggingItem = false;
      }
      redrawOverlay(pageList);
      return;
    }

    if (currentTool === "text") {
      if (textInputVisible && textInputVal.trim()) {
        commitTextInput();
      }
      const rect = overlayCanvas.getBoundingClientRect();
      textInputCssX = e.clientX - rect.left;
      textInputCssY = e.clientY - rect.top;
      textInputCanvasX = pos.x;
      textInputCanvasY = pos.y;
      textInputVal = "";
      textInputVisible = true;
      tick().then(() => textInputRef?.focus());
      return;
    }

    if (currentTool === "eraser") {
      if (eraserMode === "object") {
        erasedAnyInStroke = false;
        eraseObjectAtPoint(pos);
      } else {
        currentStrokePoints = [{ x: pos.x, y: pos.y }];
      }
      return;
    }

    if (currentTool === "pen" || currentTool === "highlighter") {
      currentStrokePoints = [{ x: pos.x, y: pos.y }];
    }
  }

  function onMouseMove(e: MouseEvent) {
    if (!activeDoc || !overlayCanvas) return;
    const pos = getCanvasPos(e);

    isMouseOverCanvas = true;
    if (currentTool === "eraser" && (eraserMode === "precision" || eraserMode === "whiteout")) {
      const rect = overlayCanvas.getBoundingClientRect();
      eraserClientPos = {
        x: e.clientX - rect.left,
        y: e.clientY - rect.top,
      };
      const scaleRatio = rect.width / overlayCanvas.width;
      eraserDisplayDiameter = Math.max(6, eraserSize * scaleRatio);
    }

    if (currentTool === "select" && !isPointerDown) {
      const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
      hoverResizeHandle = null;
      if (selectedAnnotationIds.length === 1) {
        const selectedItem = pageList.find((a) => a.id === selectedAnnotationIds[0]);
        if (selectedItem) {
          const bounds = getAnnotationBounds(selectedItem);
          if (bounds) {
            const hitHandle = findHitResizeHandle(pos, bounds, activeDoc.scale);
            if (hitHandle) {
              hoverResizeHandle = hitHandle;
              isHoveringItem = false;
              return;
            }
          }
        }
      }
      const hit = findHitAnnotation(pos, pageList);
      isHoveringItem = !!hit;
      return;
    }

    if (!isPointerDown) return;
    currentX = pos.x;
    currentY = pos.y;

    if (isResizingItem && selectedAnnotationId && activeResizeHandle && resizeInitialBounds) {
      const dx = pos.x - resizeStartPos.x;
      const dy = pos.y - resizeStartPos.y;
      if (dx !== 0 || dy !== 0) didResizeItem = true;

      const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
      const item = pageList.find((a) => a.id === selectedAnnotationId);
      if (item) {
        let newX = resizeInitialBounds.x;
        let newY = resizeInitialBounds.y;
        let newW = resizeInitialBounds.width;
        let newH = resizeInitialBounds.height;
        const minW = 12;
        const minH = 12;

        switch (activeResizeHandle) {
          case "e": {
            newW = Math.max(minW, resizeInitialBounds.width + dx);
            break;
          }
          case "w": {
            const calculatedW = resizeInitialBounds.width - dx;
            newW = Math.max(minW, calculatedW);
            newX = resizeInitialBounds.x + (resizeInitialBounds.width - newW);
            break;
          }
          case "s": {
            newH = Math.max(minH, resizeInitialBounds.height + dy);
            break;
          }
          case "n": {
            const calculatedH = resizeInitialBounds.height - dy;
            newH = Math.max(minH, calculatedH);
            newY = resizeInitialBounds.y + (resizeInitialBounds.height - newH);
            break;
          }
          case "se": {
            newW = Math.max(minW, resizeInitialBounds.width + dx);
            newH = Math.max(minH, resizeInitialBounds.height + dy);
            break;
          }
          case "sw": {
            const calculatedW = resizeInitialBounds.width - dx;
            newW = Math.max(minW, calculatedW);
            newX = resizeInitialBounds.x + (resizeInitialBounds.width - newW);
            newH = Math.max(minH, resizeInitialBounds.height + dy);
            break;
          }
          case "ne": {
            newW = Math.max(minW, resizeInitialBounds.width + dx);
            const calculatedH = resizeInitialBounds.height - dy;
            newH = Math.max(minH, calculatedH);
            newY = resizeInitialBounds.y + (resizeInitialBounds.height - newH);
            break;
          }
          case "nw": {
            const calculatedW = resizeInitialBounds.width - dx;
            newW = Math.max(minW, calculatedW);
            newX = resizeInitialBounds.x + (resizeInitialBounds.width - newW);
            const calculatedH = resizeInitialBounds.height - dy;
            newH = Math.max(minH, calculatedH);
            newY = resizeInitialBounds.y + (resizeInitialBounds.height - newH);
            break;
          }
        }

        if (item.type === "rect" || item.type === "redact" || item.type === "circle" || item.type === "image" || item.type === "text") {
          item.x = newX;
          item.y = newY;
          item.width = newW;
          item.height = newH;
        } else if (item.type === "arrow" || item.type === "line") {
          const initSnap = resizeInitialSnapshot?.find((s) => s.id === item.id);
          if (initSnap && resizeInitialBounds.width > 0 && resizeInitialBounds.height > 0) {
            const sx = newW / resizeInitialBounds.width;
            const sy = newH / resizeInitialBounds.height;
            item.startX = newX + ((initSnap.startX ?? 0) - resizeInitialBounds.x) * sx;
            item.startY = newY + ((initSnap.startY ?? 0) - resizeInitialBounds.y) * sy;
            item.endX = newX + ((initSnap.endX ?? 0) - resizeInitialBounds.x) * sx;
            item.endY = newY + ((initSnap.endY ?? 0) - resizeInitialBounds.y) * sy;
          }
        } else if (item.type === "stroke" || item.type === "highlighter") {
          const initSnap = resizeInitialSnapshot?.find((s) => s.id === item.id);
          if (initSnap && initSnap.points && resizeInitialBounds.width > 0 && resizeInitialBounds.height > 0) {
            const sx = newW / resizeInitialBounds.width;
            const sy = newH / resizeInitialBounds.height;
            item.points = initSnap.points.map((pt) => ({
              x: newX + (pt.x - resizeInitialBounds.x) * sx,
              y: newY + (pt.y - resizeInitialBounds.y) * sy,
            }));
          }
        }

        activeDoc.annotations[activeDoc.currentPage] = pageList;
        redrawOverlay(pageList);
      }
      return;
    }

    if (isDraggingItem && selectedAnnotationIds.length > 0) {
      if (dragOverSidebarPage) {
        // While hovering thumbnail drop target in sidebar, do not shift coordinates on canvas
        return;
      }

      if (didPageChangeDuringDrag) {
        didPageChangeDuringDrag = false;
        lastMoveX = pos.x;
        lastMoveY = pos.y;
      }

      const pageH = (overlayCanvas?.height || 1000) / (activeDoc.scale || 1.0);
      const now = Date.now();

      // If dragged past bottom boundary of current page canvas, move to next page
      if (pos.y > pageH + 15 && activeDoc.currentPage < activeDoc.pageCount && now - lastDragPageTime > 500) {
        lastDragPageTime = now;
        transferSelectedToPage(activeDoc.currentPage + 1, "top");
        lastMoveX = pos.x;
        lastMoveY = pos.y;
        return;
      }

      // If dragged past top boundary of current page canvas, move to previous page
      if (pos.y < -15 && activeDoc.currentPage > 1 && now - lastDragPageTime > 500) {
        lastDragPageTime = now;
        transferSelectedToPage(activeDoc.currentPage - 1, "bottom");
        lastMoveX = pos.x;
        lastMoveY = pos.y;
        return;
      }

      const dx = pos.x - lastMoveX;
      const dy = pos.y - lastMoveY;
      if (dx !== 0 || dy !== 0) didMoveItem = true;
      lastMoveX = pos.x;
      lastMoveY = pos.y;

      const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
      const selectedSet = new Set(selectedAnnotationIds);
      for (const item of pageList) {
        if (!selectedSet.has(item.id)) continue;
        if (item.type === "rect" || item.type === "redact" || item.type === "image" || item.type === "text" || item.type === "circle") {
          item.x = (item.x ?? 0) + dx;
          item.y = (item.y ?? 0) + dy;
        } else if (item.type === "arrow" || item.type === "line") {
          item.startX = (item.startX ?? 0) + dx;
          item.startY = (item.startY ?? 0) + dy;
          item.endX = (item.endX ?? 0) + dx;
          item.endY = (item.endY ?? 0) + dy;
        } else if (item.type === "stroke" || item.type === "highlighter") {
          if (item.points) {
            for (const p of item.points) {
              p.x += dx;
              p.y += dy;
            }
          }
        }
      }
      activeDoc.annotations[activeDoc.currentPage] = pageList;
      redrawOverlay(pageList);
      return;
    }

    if (currentTool === "eraser") {
      if (eraserMode === "object") {
        eraseObjectAtPoint(pos);
      } else {
        currentStrokePoints.push({ x: pos.x, y: pos.y });
        redrawOverlay();
        drawActiveDraft();
      }
      return;
    }

    if (currentTool === "pen" || currentTool === "highlighter") {
      currentStrokePoints.push({ x: pos.x, y: pos.y });
      redrawOverlay();
      drawActiveDraft();
      return;
    }

    if (["rect", "circle", "arrow", "line", "redact"].includes(currentTool)) {
      redrawOverlay();
      drawActiveDraft();
    }
  }

  function onMouseUp(e: MouseEvent) {
    if (!isPointerDown || !activeDoc) return;
    isPointerDown = false;

    if (isResizingItem) {
      isResizingItem = false;
      activeResizeHandle = null;
      if (didResizeItem && resizeInitialSnapshot) {
        if (!undoStacks[activeDoc.id]) undoStacks[activeDoc.id] = [];
        undoStacks[activeDoc.id].push({ page: activeDoc.currentPage, annotations: resizeInitialSnapshot });
        redoStacks[activeDoc.id] = [];
        historyVersion++;
      }
      resizeInitialSnapshot = null;
      resizeInitialBounds = null;
      didResizeItem = false;
      const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
      setPageAnnotations(activeDoc.id, activeDoc.currentPage, pageList);
      notifyPageThumbnailsUpdated(activeDoc.currentPage);
      redrawOverlay(pageList);
      return;
    }

    if (isDraggingItem) {
      isDraggingItem = false;
      if (didMoveItem && dragInitialSnapshot) {
        if (!undoStacks[activeDoc.id]) undoStacks[activeDoc.id] = [];
        undoStacks[activeDoc.id].push({ page: activeDoc.currentPage, annotations: dragInitialSnapshot });
        redoStacks[activeDoc.id] = [];
        historyVersion++;
      }
      dragInitialSnapshot = null;
      didMoveItem = false;
      const pageList = activeDoc.annotations[activeDoc.currentPage] || [];
      setPageAnnotations(activeDoc.id, activeDoc.currentPage, pageList);
      notifyPageThumbnailsUpdated(activeDoc.currentPage);
      redrawOverlay(pageList);
      return;
    }

    const pos = getCanvasPos(e);
    const currentList = [...(activeDoc.annotations[activeDoc.currentPage] || [])];

    if (currentTool === "eraser") {
      if (eraserMode === "precision") {
        if (currentStrokePoints.length > 0) {
          if (currentStrokePoints.length === 1) {
            currentStrokePoints.push({ x: currentStrokePoints[0].x + 0.1, y: currentStrokePoints[0].y + 0.1 });
          }
          pushHistory(activeDoc.id, activeDoc.currentPage);
          currentList.push({
            id: Math.random().toString(36).slice(2),
            type: "stroke",
            points: [...currentStrokePoints],
            strokeWidth: eraserSize,
            blendMode: "destination-out",
          });
          activeDoc.annotations[activeDoc.currentPage] = currentList;
          setPageAnnotations(activeDoc.id, activeDoc.currentPage, currentList);
        }
      } else if (eraserMode === "whiteout") {
        if (currentStrokePoints.length > 0) {
          if (currentStrokePoints.length === 1) {
            currentStrokePoints.push({ x: currentStrokePoints[0].x + 0.1, y: currentStrokePoints[0].y + 0.1 });
          }
          pushHistory(activeDoc.id, activeDoc.currentPage);
          currentList.push({
            id: Math.random().toString(36).slice(2),
            type: "stroke",
            points: [...currentStrokePoints],
            color: "#ffffff",
            strokeWidth: eraserSize,
            opacity: 1.0,
          });
          activeDoc.annotations[activeDoc.currentPage] = currentList;
          setPageAnnotations(activeDoc.id, activeDoc.currentPage, currentList);
        }
      }
      currentStrokePoints = [];
      erasedAnyInStroke = false;
      notifyPageThumbnailsUpdated(activeDoc.currentPage);
      redrawOverlay(currentList);
      return;
    }

    if (currentTool === "pen" || currentTool === "highlighter") {
      if (currentStrokePoints.length > 1) {
        pushHistory(activeDoc.id, activeDoc.currentPage);
        currentList.push({
          id: Math.random().toString(36).slice(2),
          type: currentTool === "pen" ? "stroke" : "highlighter",
          points: [...currentStrokePoints],
          color: currentTool === "pen" ? currentColor : highlighterColors[0],
          strokeWidth: currentTool === "pen" ? currentStrokeWidth : 18,
          opacity: currentTool === "pen" ? (currentOpacity < 100 ? currentOpacity / 100 : 1.0) : 0.45,
        });
        activeDoc.annotations[activeDoc.currentPage] = currentList;
        setPageAnnotations(activeDoc.id, activeDoc.currentPage, currentList);
        notifyPageThumbnailsUpdated(activeDoc.currentPage);
      }
      currentStrokePoints = [];
    } else if (currentTool === "arrow" || currentTool === "line") {
      const dist = Math.hypot(pos.x - startX, pos.y - startY);
      if (dist > 5) {
        pushHistory(activeDoc.id, activeDoc.currentPage);
        currentList.push({
          id: Math.random().toString(36).slice(2),
          type: currentTool,
          startX,
          startY,
          endX: pos.x,
          endY: pos.y,
          color: currentColor,
          strokeWidth: currentStrokeWidth,
          opacity: currentOpacity < 100 ? currentOpacity / 100 : 1.0,
        });
        activeDoc.annotations[activeDoc.currentPage] = currentList;
        setPageAnnotations(activeDoc.id, activeDoc.currentPage, currentList);
        notifyPageThumbnailsUpdated(activeDoc.currentPage);
      }
    } else if (["rect", "circle", "redact"].includes(currentTool)) {
      const w = pos.x - startX;
      const h = pos.y - startY;
      if (Math.abs(w) > 5 || Math.abs(h) > 5) {
        pushHistory(activeDoc.id, activeDoc.currentPage);
        currentList.push({
          id: Math.random().toString(36).slice(2),
          type: currentTool as any,
          x: Math.min(startX, pos.x),
          y: Math.min(startY, pos.y),
          width: Math.abs(w),
          height: Math.abs(h),
          color: currentTool === "redact" ? "#000000" : currentColor,
          fillColor: currentTool === "redact" ? "#000000" : (currentFillColor === "transparent" ? undefined : currentFillColor),
          strokeWidth: currentStrokeWidth,
          opacity: currentTool === "redact" ? 1.0 : (currentOpacity < 100 ? currentOpacity / 100 : 1.0),
        });
        activeDoc.annotations[activeDoc.currentPage] = currentList;
        setPageAnnotations(activeDoc.id, activeDoc.currentPage, currentList);
        notifyPageThumbnailsUpdated(activeDoc.currentPage);
      }
    }

    redrawOverlay(currentList);
  }

  function onMouseLeave(e: MouseEvent) {
    isMouseOverCanvas = false;
    if (isPointerDown && !isDraggingItem && !isResizingItem) {
      onMouseUp(e);
    }
  }

  function onWindowMouseMove(e: MouseEvent) {
    if (!isPointerDown || !activeDoc) return;

    if (isResizingItem) {
      onMouseMove(e);
      return;
    }

    if (isDraggingItem && selectedAnnotationIds.length > 0) {
      // Check if cursor is hovering over a sidebar thumbnail
      const thumbEl = (document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null)?.closest("[data-page-thumb]");
      if (thumbEl) {
        const pNum = Number(thumbEl.getAttribute("data-page-thumb"));
        if (pNum && pNum >= 1 && pNum <= activeDoc.pageCount) {
          dragOverSidebarPage = pNum;
        } else {
          dragOverSidebarPage = null;
        }
      } else {
        dragOverSidebarPage = null;
      }

      onMouseMove(e);
    }
  }

  function onWindowMouseUp(e: MouseEvent) {
    window.removeEventListener("mousemove", onWindowMouseMove);
    window.removeEventListener("mouseup", onWindowMouseUp);

    if (isDraggingItem && selectedAnnotationIds.length > 0 && activeDoc) {
      if (dragOverSidebarPage && dragOverSidebarPage !== activeDoc.currentPage) {
        const targetPage = dragOverSidebarPage;
        dragOverSidebarPage = null;
        transferSelectedToPage(targetPage, "current");
        pushToast(`Nesne Sayfa ${targetPage}'e aktarıldı`, "success");
        return;
      }
      dragOverSidebarPage = null;
    }

    onMouseUp(e);
  }

  function drawActiveDraft() {
    if (!overlayCtx || !activeDoc) return;
    overlayCtx.save();
    overlayCtx.scale(activeDoc.scale, activeDoc.scale);

    if (currentTool === "eraser" && eraserMode === "precision" && currentStrokePoints.length > 1) {
      overlayCtx.save();
      overlayCtx.globalCompositeOperation = "destination-out";
      overlayCtx.strokeStyle = "rgba(0,0,0,1)";
      overlayCtx.lineWidth = eraserSize / activeDoc.scale;
      overlayCtx.lineCap = "round";
      overlayCtx.lineJoin = "round";
      overlayCtx.beginPath();
      overlayCtx.moveTo(currentStrokePoints[0].x, currentStrokePoints[0].y);
      for (let i = 1; i < currentStrokePoints.length; i++) {
        overlayCtx.lineTo(currentStrokePoints[i].x, currentStrokePoints[i].y);
      }
      overlayCtx.stroke();
      overlayCtx.restore();
    } else if (currentTool === "eraser" && eraserMode === "whiteout" && currentStrokePoints.length > 1) {
      overlayCtx.save();
      overlayCtx.strokeStyle = "#ffffff";
      overlayCtx.lineWidth = eraserSize / activeDoc.scale;
      overlayCtx.lineCap = "round";
      overlayCtx.lineJoin = "round";
      overlayCtx.beginPath();
      overlayCtx.moveTo(currentStrokePoints[0].x, currentStrokePoints[0].y);
      for (let i = 1; i < currentStrokePoints.length; i++) {
        overlayCtx.lineTo(currentStrokePoints[i].x, currentStrokePoints[i].y);
      }
      overlayCtx.stroke();
      overlayCtx.restore();
    } else if (currentTool === "pen" && currentStrokePoints.length > 1) {
      overlayCtx.strokeStyle = currentColor;
      overlayCtx.lineWidth = currentStrokeWidth;
      overlayCtx.lineCap = "round";
      overlayCtx.lineJoin = "round";
      overlayCtx.beginPath();
      overlayCtx.moveTo(currentStrokePoints[0].x, currentStrokePoints[0].y);
      for (let i = 1; i < currentStrokePoints.length; i++) {
        overlayCtx.lineTo(currentStrokePoints[i].x, currentStrokePoints[i].y);
      }
      overlayCtx.stroke();
    } else if (currentTool === "highlighter" && currentStrokePoints.length > 1) {
      overlayCtx.strokeStyle = highlighterColors[0];
      overlayCtx.lineWidth = 18;
      overlayCtx.lineCap = "round";
      overlayCtx.lineJoin = "round";
      overlayCtx.beginPath();
      overlayCtx.moveTo(currentStrokePoints[0].x, currentStrokePoints[0].y);
      for (let i = 1; i < currentStrokePoints.length; i++) {
        overlayCtx.lineTo(currentStrokePoints[i].x, currentStrokePoints[i].y);
      }
      overlayCtx.stroke();
    } else if (currentTool === "rect") {
      if (currentOpacity < 100) overlayCtx.globalAlpha = currentOpacity / 100;
      if (currentFillColor && currentFillColor !== "transparent") {
        overlayCtx.fillStyle = currentFillColor;
        overlayCtx.fillRect(
          Math.min(startX, currentX),
          Math.min(startY, currentY),
          Math.abs(currentX - startX),
          Math.abs(currentY - startY)
        );
      }
      overlayCtx.strokeStyle = currentColor;
      overlayCtx.lineWidth = currentStrokeWidth;
      overlayCtx.strokeRect(
        Math.min(startX, currentX),
        Math.min(startY, currentY),
        Math.abs(currentX - startX),
        Math.abs(currentY - startY)
      );
    } else if (currentTool === "redact") {
      overlayCtx.fillStyle = "#000000";
      overlayCtx.fillRect(
        Math.min(startX, currentX),
        Math.min(startY, currentY),
        Math.abs(currentX - startX),
        Math.abs(currentY - startY)
      );
    } else if (currentTool === "circle") {
      if (currentOpacity < 100) overlayCtx.globalAlpha = currentOpacity / 100;
      const rx = Math.abs(currentX - startX) / 2;
      const ry = Math.abs(currentY - startY) / 2;
      const cx = Math.min(startX, currentX) + rx;
      const cy = Math.min(startY, currentY) + ry;
      overlayCtx.beginPath();
      overlayCtx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
      if (currentFillColor && currentFillColor !== "transparent") {
        overlayCtx.fillStyle = currentFillColor;
        overlayCtx.fill();
      }
      overlayCtx.strokeStyle = currentColor;
      overlayCtx.lineWidth = currentStrokeWidth;
      overlayCtx.stroke();
    } else if (currentTool === "line") {
      overlayCtx.strokeStyle = currentColor;
      overlayCtx.lineWidth = currentStrokeWidth;
      overlayCtx.beginPath();
      overlayCtx.moveTo(startX, startY);
      overlayCtx.lineTo(currentX, currentY);
      overlayCtx.stroke();
    } else if (currentTool === "arrow") {
      renderArrow(
        overlayCtx,
        startX,
        startY,
        currentX,
        currentY,
        currentColor,
        currentStrokeWidth,
        currentOpacity < 100 ? currentOpacity / 100 : 1.0
      );
    }

    overlayCtx.restore();
  }

  function commitTextInput() {
    if (!textInputVisible || !textInputVal.trim() || !activeDoc) {
      textInputVisible = false;
      textInputVal = "";
      return;
    }
    pushHistory(activeDoc.id, activeDoc.currentPage);
    const currentList = [...(activeDoc.annotations[activeDoc.currentPage] || [])];

    let textWidth = Math.max(40, textInputVal.trim().length * currentFontSize * 0.65 + 10);
    let textHeight = currentFontSize * 1.35;
    if (overlayCtx) {
      overlayCtx.save();
      overlayCtx.font = `${currentFontSize}px sans-serif`;
      const m = overlayCtx.measureText(textInputVal.trim());
      if (m && m.width) textWidth = Math.ceil(m.width);
      overlayCtx.restore();
    }

    currentList.push({
      id: Math.random().toString(36).slice(2),
      type: "text",
      x: textInputCanvasX,
      y: textInputCanvasY,
      width: textWidth,
      height: textHeight,
      text: textInputVal.trim(),
      fontSize: currentFontSize,
      color: currentColor,
      opacity: currentOpacity < 100 ? currentOpacity / 100 : 1.0,
    });
    activeDoc.annotations[activeDoc.currentPage] = currentList;
    setPageAnnotations(activeDoc.id, activeDoc.currentPage, currentList);
    textInputVisible = false;
    textInputVal = "";
    notifyPageThumbnailsUpdated(activeDoc.currentPage);
    redrawOverlay(currentList);
  }

  function redrawOverlay(listOverride?: Annotation[]) {
    if (!overlayCtx || !overlayCanvas || !activeDoc) return;
    overlayCtx.clearRect(0, 0, overlayCanvas.width, overlayCanvas.height);
    const list = listOverride || (activeDoc.annotations && activeDoc.annotations[activeDoc.currentPage]) || [];

    overlayCtx.save();
    overlayCtx.scale(activeDoc.scale, activeDoc.scale);

    for (const item of list) {
      overlayCtx.save();
      if (item.blendMode === "destination-out") {
        if (item.points && item.points.length > 1) {
          overlayCtx.globalCompositeOperation = "destination-out";
          overlayCtx.strokeStyle = "rgba(0,0,0,1)";
          overlayCtx.lineWidth = (item.strokeWidth || 20) / activeDoc.scale;
          overlayCtx.lineCap = "round";
          overlayCtx.lineJoin = "round";
          overlayCtx.beginPath();
          overlayCtx.moveTo(item.points[0].x, item.points[0].y);
          for (let i = 1; i < item.points.length; i++) {
            overlayCtx.lineTo(item.points[i].x, item.points[i].y);
          }
          overlayCtx.stroke();
          overlayCtx.globalCompositeOperation = "source-over";
        }
      } else if (item.type === "stroke" && item.points && item.points.length > 1) {
        overlayCtx.strokeStyle = item.color || "#000";
        overlayCtx.lineWidth = item.strokeWidth || 3;
        overlayCtx.lineCap = "round";
        overlayCtx.lineJoin = "round";
        overlayCtx.beginPath();
        overlayCtx.moveTo(item.points[0].x, item.points[0].y);
        for (let i = 1; i < item.points.length; i++) {
          overlayCtx.lineTo(item.points[i].x, item.points[i].y);
        }
        overlayCtx.stroke();
      } else if (item.type === "highlighter" && item.points && item.points.length > 1) {
        overlayCtx.strokeStyle = item.color || highlighterColors[0];
        overlayCtx.lineWidth = item.strokeWidth || 18;
        overlayCtx.lineCap = "round";
        overlayCtx.lineJoin = "round";
        overlayCtx.beginPath();
        overlayCtx.moveTo(item.points[0].x, item.points[0].y);
        for (let i = 1; i < item.points.length; i++) {
          overlayCtx.lineTo(item.points[i].x, item.points[i].y);
        }
        overlayCtx.stroke();
      } else if (item.type === "text" && item.text && item.x !== undefined && item.y !== undefined) {
        if (item.opacity !== undefined) overlayCtx.globalAlpha = item.opacity;
        overlayCtx.font = `${item.fontSize || 18}px sans-serif`;
        overlayCtx.fillStyle = item.color || "#9333ea";
        overlayCtx.textBaseline = "top";
        overlayCtx.fillText(item.text, item.x, item.y);
      } else if (item.type === "rect" && item.x !== undefined && item.y !== undefined) {
        if (item.opacity !== undefined) overlayCtx.globalAlpha = item.opacity;
        if (item.fillColor && item.fillColor !== "transparent") {
          overlayCtx.fillStyle = item.fillColor;
          overlayCtx.fillRect(item.x, item.y, item.width || 0, item.height || 0);
        }
        overlayCtx.strokeStyle = item.color || "#000";
        overlayCtx.lineWidth = item.strokeWidth || 3;
        overlayCtx.strokeRect(item.x, item.y, item.width || 0, item.height || 0);
      } else if (item.type === "redact" && item.x !== undefined && item.y !== undefined) {
        overlayCtx.fillStyle = item.fillColor || "#000000";
        overlayCtx.fillRect(item.x, item.y, item.width || 0, item.height || 0);
      } else if (item.type === "circle" && item.x !== undefined && item.y !== undefined) {
        if (item.opacity !== undefined) overlayCtx.globalAlpha = item.opacity;
        const rx = (item.width || 0) / 2;
        const ry = (item.height || 0) / 2;
        overlayCtx.beginPath();
        overlayCtx.ellipse(item.x + rx, item.y + ry, rx, ry, 0, 0, Math.PI * 2);
        if (item.fillColor && item.fillColor !== "transparent") {
          overlayCtx.fillStyle = item.fillColor;
          overlayCtx.fill();
        }
        overlayCtx.strokeStyle = item.color || "#000";
        overlayCtx.lineWidth = item.strokeWidth || 3;
        overlayCtx.stroke();
      } else if (item.type === "line" && item.startX !== undefined && item.startY !== undefined && item.endX !== undefined && item.endY !== undefined) {
        if (item.opacity !== undefined) overlayCtx.globalAlpha = item.opacity;
        overlayCtx.strokeStyle = item.color || "#000";
        overlayCtx.lineWidth = item.strokeWidth || 3;
        overlayCtx.beginPath();
        overlayCtx.moveTo(item.startX, item.startY);
        overlayCtx.lineTo(item.endX, item.endY);
        overlayCtx.stroke();
      } else if (item.type === "arrow" && item.startX !== undefined && item.startY !== undefined && item.endX !== undefined && item.endY !== undefined) {
        renderArrow(
          overlayCtx,
          item.startX,
          item.startY,
          item.endX,
          item.endY,
          item.color || "#000",
          item.strokeWidth || 3,
          item.opacity
        );
      } else if (item.type === "image" && item.x !== undefined && item.y !== undefined) {
        if (item.imageEl instanceof HTMLImageElement && item.imageEl.complete && item.imageEl.naturalWidth > 0) {
          overlayCtx.drawImage(item.imageEl, item.x, item.y, item.width || 120, item.height || 60);
        } else if (item.imageData && typeof item.imageData === "string" && !item.imageEl) {
          const tempImg = new Image();
          tempImg.onload = () => {
            item.imageEl = tempImg;
            redrawOverlay();
          };
          tempImg.src = item.imageData;
          item.imageEl = tempImg;
        }
      }

      overlayCtx.restore();
    }

    // Active Search Match Highlight (Yellow glowing box)
    if (activeSearchMatch && activeSearchMatch.page === activeDoc.currentPage) {
      overlayCtx.save();
      overlayCtx.fillStyle = "rgba(250, 204, 21, 0.4)";
      overlayCtx.strokeStyle = "#eab308";
      overlayCtx.lineWidth = 2 / activeDoc.scale;
      overlayCtx.fillRect(
        activeSearchMatch.x - 2,
        activeSearchMatch.y - 1,
        activeSearchMatch.width + 4,
        activeSearchMatch.height + 2
      );
      overlayCtx.strokeRect(
        activeSearchMatch.x - 2,
        activeSearchMatch.y - 1,
        activeSearchMatch.width + 4,
        activeSearchMatch.height + 2
      );
      overlayCtx.restore();
    }

    // Selected item bounding box and 8 circular resize handles (matching user reference UI)
    if (currentTool === "select" && selectedAnnotationIds.length > 0) {
      if (selectedAnnotationIds.length === 1) {
        const selectedItem = list.find((a) => a.id === selectedAnnotationIds[0]);
        if (selectedItem) {
          const bounds = getAnnotationBounds(selectedItem);
          if (bounds) {
            overlayCtx.save();
            // Vibrant solid blue bounding rectangle outline
            overlayCtx.strokeStyle = "#2563eb";
            overlayCtx.lineWidth = 1.5 / activeDoc.scale;
            overlayCtx.strokeRect(bounds.x, bounds.y, bounds.width, bounds.height);

            // 8 circular resize handles matching user reference screenshot
            const handles = getResizeHandles(bounds);
            const handleRadius = 4.5 / activeDoc.scale;
            const handleLineWidth = 1.5 / activeDoc.scale;

            for (const h of handles) {
              overlayCtx.beginPath();
              overlayCtx.arc(h.x, h.y, handleRadius, 0, Math.PI * 2);
              overlayCtx.fillStyle = "#ffffff";
              overlayCtx.fill();
              overlayCtx.strokeStyle = "#2563eb";
              overlayCtx.lineWidth = handleLineWidth;
              overlayCtx.stroke();
            }
            overlayCtx.restore();
          }
        }
      } else {
        // Multi-selection: show selection bounding box and corner indicators on each selected item
        overlayCtx.save();
        overlayCtx.strokeStyle = "#2563eb";
        overlayCtx.lineWidth = 1.5 / activeDoc.scale;
        overlayCtx.setLineDash([4 / activeDoc.scale, 3 / activeDoc.scale]);
        const selSet = new Set(selectedAnnotationIds);
        for (const item of list) {
          if (selSet.has(item.id)) {
            const bounds = getAnnotationBounds(item);
            if (bounds) {
              overlayCtx.strokeRect(bounds.x - 2, bounds.y - 2, bounds.width + 4, bounds.height + 4);
              const dotSize = 4 / activeDoc.scale;
              overlayCtx.fillStyle = "#2563eb";
              overlayCtx.fillRect(bounds.x - 3, bounds.y - 3, dotSize, dotSize);
              overlayCtx.fillRect(bounds.x + bounds.width - 1, bounds.y - 3, dotSize, dotSize);
              overlayCtx.fillRect(bounds.x - 3, bounds.y + bounds.height - 1, dotSize, dotSize);
              overlayCtx.fillRect(bounds.x + bounds.width - 1, bounds.y + bounds.height - 1, dotSize, dotSize);
            }
          }
        }
        overlayCtx.restore();
      }
    }

    overlayCtx.restore(); // restores the overlayCtx.scale(activeDoc.scale, activeDoc.scale)
  }

  function handleSaveSignature(dataUrl: string) {
    if (!activeDoc) return;
    const img = new Image();
    img.onload = () => {
      pushHistory(activeDoc.id, activeDoc.currentPage);
      const currentList = [...(activeDoc.annotations[activeDoc.currentPage] || [])];
      const aspect = img.width / img.height;
      const w = 150;
      const h = w / aspect;
      const baseW = (overlayCanvas?.width || 600) / activeDoc.scale;
      const baseH = (overlayCanvas?.height || 800) / activeDoc.scale;
      const newAnn: Annotation = {
        id: Math.random().toString(36).slice(2),
        type: "image",
        x: baseW / 2 - w / 2,
        y: baseH / 2 - h / 2,
        width: w,
        height: h,
        imageData: dataUrl,
        imageEl: img,
      };
      currentList.push(newAnn);
      activeDoc.annotations[activeDoc.currentPage] = currentList;
      setPageAnnotations(activeDoc.id, activeDoc.currentPage, currentList);
      selectedAnnotationIds = [newAnn.id];
      currentTool = "select";
      notifyPageThumbnailsUpdated(activeDoc.currentPage);
      redrawOverlay(currentList);
      pushToast("İmza eklendi. Sürükleyerek dilediğiniz yere taşıyabilirsiniz.", "info");
    };
    img.src = dataUrl;
  }

  async function handleRemoveWatermarkApplyDirectly() {
    if (!activeDoc) return;
    if (activeDoc.cleanPdfBytes) {
      pushToast("Bu belgedeki filigran zaten temizlendi.", "info");
      return;
    }
    isRemovingWatermark = true;
    try {
      // Snapshot state for Undo before applying watermark removal
      if (!undoStacks[activeDoc.id]) undoStacks[activeDoc.id] = [];
      undoStacks[activeDoc.id].push({
        page: activeDoc.currentPage,
        annotations: (activeDoc.annotations[activeDoc.currentPage] || []).map((a) => ({ ...a })),
        type: "watermark",
        cleanPdfBytes: activeDoc.cleanPdfBytes ? activeDoc.cleanPdfBytes.slice(0) : null,
        wasUnsaved: activeDoc.hasUnsavedChanges,
      });
      redoStacks[activeDoc.id] = [];
      historyVersion++;

      const cleanBytes = await cleanWatermarkBytes(activeDoc.path, activeDoc.cleanPdfBytes ?? null);
      const safeCleanBytes = cleanBytes.slice(0);
      const refreshedHandle = await openPdf(activeDoc.path, safeCleanBytes.slice(0));
      await activeDoc.handle.destroy();
      activeDoc.handle = refreshedHandle;
      activeDoc.cleanPdfBytes = safeCleanBytes;
      activeDoc.hasUnsavedChanges = true;
      markDocumentModified(activeDoc.id, safeCleanBytes);
      await renderCurrentPage();
      notifyDocThumbnailsUpdated();
      showRemoveWatermarkModal = false;
      pushToast("Filigran kaldırıldı! Gerekirse 'Geri Al' (Ctrl+Z) ile eski haline döndürebilirsiniz.", "success");
    } catch (e) {
      pushToast(`Filigran kaldırılamadı: ${e}`, "error");
    } finally {
      isRemovingWatermark = false;
    }
  }

  function undoAction() {
    if (!activeDoc) return;
    const docId = activeDoc.id;
    if (!undoStacks[docId] || undoStacks[docId].length === 0) return;

    const prev = undoStacks[docId].pop()!;
    if (!redoStacks[docId]) redoStacks[docId] = [];

    if (prev.type === "watermark") {
      const currentCleanBytes = activeDoc.cleanPdfBytes ? activeDoc.cleanPdfBytes.slice(0) : null;
      redoStacks[docId].push({
        page: activeDoc.currentPage,
        annotations: (activeDoc.annotations[activeDoc.currentPage] || []).map((a) => ({ ...a })),
        type: "watermark",
        cleanPdfBytes: currentCleanBytes,
        wasUnsaved: activeDoc.hasUnsavedChanges,
      });

      (async () => {
        try {
          let refreshedHandle: any;
          if (prev.cleanPdfBytes && prev.cleanPdfBytes.length > 0) {
            const safeBytes = prev.cleanPdfBytes.slice(0);
            refreshedHandle = await openPdf(activeDoc.path, safeBytes.slice(0));
            activeDoc.cleanPdfBytes = safeBytes;
          } else {
            refreshedHandle = await openPdf(activeDoc.path);
            activeDoc.cleanPdfBytes = undefined;
          }
          await activeDoc.handle.destroy();
          activeDoc.handle = refreshedHandle;
          activeDoc.hasUnsavedChanges = prev.wasUnsaved ?? false;
          openDocs.update((docs) =>
            docs.map((d) =>
              d.id === activeDoc.id
                ? { ...d, hasUnsavedChanges: activeDoc.hasUnsavedChanges, cleanPdfBytes: activeDoc.cleanPdfBytes }
                : d
            )
          );
          await renderCurrentPage();
          notifyDocThumbnailsUpdated();
          pushToast("Filigran kaldırma işlemi geri alındı (orijinal filigran geri yüklendi).", "info");
        } catch (e) {
          pushToast(`Geri alınamadı: ${e}`, "error");
        }
      })();
      historyVersion++;
      return;
    }

    const currentList = (activeDoc.annotations[prev.page] || []).map((a) => ({
      ...a,
      points: a.points ? a.points.map((p) => ({ ...p })) : undefined,
    }));
    redoStacks[docId].push({ page: prev.page, annotations: currentList, type: "annotations" });

    activeDoc.annotations[prev.page] = prev.annotations;
    setPageAnnotations(docId, prev.page, prev.annotations);
    selectedAnnotationIds = [];
    historyVersion++;
    notifyPageThumbnailsUpdated(prev.page);

    if (activeDoc.currentPage !== prev.page) {
      updateDocPage(docId, prev.page);
    } else {
      redrawOverlay(prev.annotations);
    }
  }

  function redoAction() {
    if (!activeDoc) return;
    const docId = activeDoc.id;
    if (!redoStacks[docId] || redoStacks[docId].length === 0) return;

    const next = redoStacks[docId].pop()!;
    if (!undoStacks[docId]) undoStacks[docId] = [];

    if (next.type === "watermark") {
      const currentCleanBytes = activeDoc.cleanPdfBytes ? activeDoc.cleanPdfBytes.slice(0) : null;
      undoStacks[docId].push({
        page: activeDoc.currentPage,
        annotations: (activeDoc.annotations[activeDoc.currentPage] || []).map((a) => ({ ...a })),
        type: "watermark",
        cleanPdfBytes: currentCleanBytes,
        wasUnsaved: activeDoc.hasUnsavedChanges,
      });

      (async () => {
        try {
          let refreshedHandle: any;
          if (next.cleanPdfBytes && next.cleanPdfBytes.length > 0) {
            const safeBytes = next.cleanPdfBytes.slice(0);
            refreshedHandle = await openPdf(activeDoc.path, safeBytes.slice(0));
            activeDoc.cleanPdfBytes = safeBytes;
          } else {
            refreshedHandle = await openPdf(activeDoc.path);
            activeDoc.cleanPdfBytes = undefined;
          }
          await activeDoc.handle.destroy();
          activeDoc.handle = refreshedHandle;
          activeDoc.hasUnsavedChanges = true;
          openDocs.update((docs) =>
            docs.map((d) =>
              d.id === activeDoc.id
                ? { ...d, hasUnsavedChanges: true, cleanPdfBytes: activeDoc.cleanPdfBytes }
                : d
            )
          );
          await renderCurrentPage();
          notifyDocThumbnailsUpdated();
          pushToast("Filigran kaldırma işlemi yinelendi.", "info");
        } catch (e) {
          pushToast(`Yinelenemedi: ${e}`, "error");
        }
      })();
      historyVersion++;
      return;
    }

    const currentList = (activeDoc.annotations[next.page] || []).map((a) => ({
      ...a,
      points: a.points ? a.points.map((p) => ({ ...p })) : undefined,
    }));
    undoStacks[docId].push({ page: next.page, annotations: currentList, type: "annotations" });

    activeDoc.annotations[next.page] = next.annotations;
    setPageAnnotations(docId, next.page, next.annotations);
    selectedAnnotationIds = [];
    historyVersion++;
    notifyPageThumbnailsUpdated(next.page);

    if (activeDoc.currentPage !== next.page) {
      updateDocPage(docId, next.page);
    } else {
      redrawOverlay(next.annotations);
    }
  }

  function deleteSelectedAnnotation() {
    if (!activeDoc || selectedAnnotationIds.length === 0) return;
    pushHistory(activeDoc.id, activeDoc.currentPage);
    const toDelete = new Set(selectedAnnotationIds);
    const currentList = (activeDoc.annotations[activeDoc.currentPage] || []).filter(
      (a) => !toDelete.has(a.id)
    );
    activeDoc.annotations[activeDoc.currentPage] = currentList;
    setPageAnnotations(activeDoc.id, activeDoc.currentPage, currentList);
    selectedAnnotationIds = [];
    notifyPageThumbnailsUpdated(activeDoc.currentPage);
    redrawOverlay(currentList);
  }

  function clearPage() {
    if (!activeDoc) return;
    const current = activeDoc.annotations[activeDoc.currentPage] || [];
    if (current.length === 0) return;
    pushHistory(activeDoc.id, activeDoc.currentPage);
    activeDoc.annotations[activeDoc.currentPage] = [];
    setPageAnnotations(activeDoc.id, activeDoc.currentPage, []);
    selectedAnnotationIds = [];
    notifyPageThumbnailsUpdated(activeDoc.currentPage);
    redrawOverlay([]);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (textInputVisible) return;

    const isCtrl = e.ctrlKey || e.metaKey;

    // Permanently trap Ctrl + F globally with e.preventDefault() so WebView2 native browser search bar NEVER opens
    if (isCtrl && (e.key === "f" || e.key === "F")) {
      e.preventDefault();
      if (activeDoc) {
        showSearchBar = !showSearchBar;
      }
      return;
    }

    // Ctrl + S: Kaydet / Ctrl + Shift + S: Farklı Kaydet
    if (isCtrl && (e.key === "s" || e.key === "S")) {
      e.preventDefault();
      handleSaveTrigger(e.shiftKey);
      return;
    }

    // Ctrl + P: Yazdır
    if (isCtrl && (e.key === "p" || e.key === "P")) {
      e.preventDefault();
      if (activeDoc) showPrintModal = true;
      return;
    }

    // Do not intercept non-global shortcuts if user is typing inside an input/textarea/select
    const target = e.target as HTMLElement;
    if (target && (target.tagName === "INPUT" || target.tagName === "SELECT" || target.tagName === "TEXTAREA")) {
      return;
    }

    if (isCtrl) {
      // Ctrl + C: Seçili Nesneleri Kopyala
      if (e.key === "c" || e.key === "C") {
        if (selectedAnnotationIds.length > 0) {
          e.preventDefault();
          copySelectedAnnotations();
          return;
        }
      }

      // Ctrl + V: Nesneleri Yapıştır (Öteleyerek çoğalt)
      if (e.key === "v" || e.key === "V") {
        if (annotationClipboard.length > 0) {
          e.preventDefault();
          pasteAnnotations();
          return;
        }
      }

      // Ctrl + D: Doğrudan Çoğalt (Duplicate)
      if (e.key === "d" || e.key === "D") {
        if (selectedAnnotationIds.length > 0) {
          e.preventDefault();
          copySelectedAnnotations();
          pasteAnnotations();
          return;
        }
      }

      // Ctrl + O: Dosya Aç
      if (e.key === "o" || e.key === "O") {
        e.preventDefault();
        openNewFileDialog();
        return;
      }

      // Ctrl + W: PDF Sekmesini Kapat
      if (e.key === "w" || e.key === "W") {
        e.preventDefault();
        if (activeDoc) handleCloseTab(activeDoc.id);
        return;
      }

      // Ctrl + Z: Geri Al
      if ((e.key === "z" || e.key === "Z") && !e.shiftKey) {
        e.preventDefault();
        undoAction();
        return;
      }

      // Ctrl + Y / Ctrl + Shift + Z: Yinele
      if ((e.key === "y" || e.key === "Y") || (e.shiftKey && (e.key === "z" || e.key === "Z"))) {
        e.preventDefault();
        redoAction();
        return;
      }

      // Ctrl + + / Ctrl + = : Yakınlaştır
      if (e.key === "+" || e.key === "=") {
        e.preventDefault();
        changeZoom(0.2);
        return;
      }

      // Ctrl + - : Uzaklaştır
      if (e.key === "-") {
        e.preventDefault();
        changeZoom(-0.2);
        return;
      }

      // Ctrl + 0 / Ctrl + 1 : Sayfayı ekrana sığdır / %100
      if (e.key === "0" || e.key === "1") {
        e.preventDefault();
        if (activeDoc) updateDocScale(activeDoc.id, 1.0);
        return;
      }

      // Ctrl + Home : İlk Sayfa
      if (e.key === "Home") {
        e.preventDefault();
        setPage(1);
        return;
      }

      // Ctrl + End : Son Sayfa
      if (e.key === "End") {
        e.preventDefault();
        if (activeDoc) setPage(activeDoc.pageCount);
        return;
      }
    } else {
      // Non-Ctrl shortcuts:
      if (e.key === "PageUp" || e.key === "ArrowLeft") {
        if (activeDoc && activeDoc.currentPage > 1) {
          e.preventDefault();
          setPage(activeDoc.currentPage - 1);
        }
        return;
      }

      if (e.key === "PageDown" || e.key === "ArrowRight") {
        if (activeDoc && activeDoc.currentPage < activeDoc.pageCount) {
          e.preventDefault();
          setPage(activeDoc.currentPage + 1);
        }
        return;
      }

      if (e.key === "Home") {
        e.preventDefault();
        setPage(1);
        return;
      }

      if (e.key === "End") {
        e.preventDefault();
        if (activeDoc) setPage(activeDoc.pageCount);
        return;
      }

      if (e.key === "F11") {
        e.preventDefault();
        toggleReadingMode();
        return;
      }

      if (e.key === "Escape") {
        if (isReadingMode) {
          e.preventDefault();
          toggleReadingMode();
          return;
        }
      }

      if ((e.key === "Delete" || e.key === "Backspace") && selectedAnnotationIds.length > 0 && activeDoc) {
        e.preventDefault();
        deleteSelectedAnnotation();
        return;
      }

      if (e.key === "Escape") {
        e.preventDefault();
        if (showSearchBar) {
          showSearchBar = false;
          activeSearchMatch = null;
          redrawOverlay();
        } else if (selectedAnnotationIds.length > 0) {
          selectedAnnotationIds = [];
          redrawOverlay();
        } else {
          currentTool = "select";
        }
        return;
      }
    }
  }

  function hasAnyRedactions(doc: OpenDocument): boolean {
    for (const pageNum in doc.annotations) {
      const list = doc.annotations[pageNum] || [];
      for (const item of list) {
        if (item.type === "redact") return true;
      }
    }
    return false;
  }

  function handleSaveTrigger(isSaveAs = false) {
    if (!activeDoc) return;
    commitTextInput();
    pendingSaveAs = isSaveAs;

    if (hasAnyRedactions(activeDoc)) {
      showRedactionConfirmModal = true;
    } else {
      executeSave(pendingSaveAs);
    }
  }

  function confirmRedactionAndSave() {
    showRedactionConfirmModal = false;
    executeSave(pendingSaveAs);
  }

  async function executeSave(forceSaveAs = false) {

    if (!activeDoc) return;
    commitTextInput();

    const editedPages = Object.keys(activeDoc.annotations)
      .map(Number)
      .filter((p) => activeDoc.annotations[p] && activeDoc.annotations[p].length > 0);
    const hasWatermarkChanges = !!activeDoc.cleanPdfBytes;
    const hasModifications = editedPages.length > 0 || hasWatermarkChanges || activeDoc.hasUnsavedChanges;

    if (!hasModifications && !forceSaveAs) {
      pushToast("Henüz bu belgede bir düzenleme yapmadınız.", "info");
      return;
    }

    const isRedactedDoc = hasAnyRedactions(activeDoc);
    let outputPath = activeDoc.path;

    if (isRedactedDoc) {
      // User specified requirement: If document contains redactions, do NOT auto-overwrite!
      // Suggest filename with _cencored suffix (e.g. maliDosyaOcak_cencored.pdf) and prompt save dialog.
      const originalPath = activeDoc.path;
      const dir = originalPath.replace(/[\\/][^\\/]+$/, "");
      const fileName = baseName(originalPath);
      const dotIdx = fileName.lastIndexOf(".");
      const stem = dotIdx > 0 ? fileName.slice(0, dotIdx) : fileName;
      const ext = dotIdx > 0 ? fileName.slice(dotIdx) : ".pdf";
      const censoredStem = stem.endsWith("_cencored") ? stem : `${stem}_cencored`;
      const defaultCensoredPath = dir ? `${dir}\\${censoredStem}${ext}` : `${censoredStem}${ext}`;

      const selected = await save({
        filters: [{ name: "PDF", extensions: ["pdf"] }],
        defaultPath: defaultCensoredPath,
      });
      if (!selected) return;
      outputPath = selected;
    } else if (forceSaveAs || !outputPath) {
      const selected = await save({
        filters: [{ name: "PDF", extensions: ["pdf"] }],
        defaultPath: activeDoc.path,
      });
      if (!selected) return;
      outputPath = selected;
    }

    busy = true;
    try {
      if (editedPages.length === 0) {
        if (activeDoc.cleanPdfBytes) {
          await writePdfBytes(outputPath, activeDoc.cleanPdfBytes);
        }
        if (outputPath !== activeDoc.path) {
          updateDocPath(activeDoc.id, outputPath);
        }
        activeDoc.cleanPdfBytes = undefined;
        markDocumentSaved(activeDoc.id);
        logActivity([baseName(activeDoc.path)], "Düzenlendi & Kaydedildi", outputPath);
        pushToast(`Başarıyla kaydedildi: ${baseName(outputPath)}`, "success", {
          label: "Klasöre Git",
          path: outputPath,
        });
        return;
      }

      const overlays: PageOverlayPayload[] = [];
      const saveScale = 2.5; // High resolution 180+ DPI crisp rasterization

      for (const p of editedPages) {
        const rendered = await activeDoc.handle.renderPage(p, saveScale);
        const list = activeDoc.annotations[p] || [];
        const redactItems = list.filter((item) => item.type === "redact");
        const nonRedactItems = list.filter((item) => item.type !== "redact");

        // 1. If page has redactions: Burn ONLY the redactions into the base page pixels (physical destruction of vector text)
        if (redactItems.length > 0) {
          const rCanvas = document.createElement("canvas");
          rCanvas.width = rendered.width;
          rCanvas.height = rendered.height;
          const rCtx = rCanvas.getContext("2d")!;
          // Draw base page
          rCtx.drawImage(rendered, 0, 0);
          rCtx.save();
          rCtx.scale(saveScale, saveScale);
          for (const item of redactItems) {
            drawSingleAnnotation(rCtx, item, saveScale);
          }
          rCtx.restore();

          const rBlob = await new Promise<Blob | null>((resolve) => rCanvas.toBlob(resolve, "image/png"));
          if (!rBlob) throw new Error(`Sayfa ${p} sansür katmanı işlenemedi`);
          const rBytes = new Uint8Array(await rBlob.arrayBuffer());
          overlays.push({
            page: p,
            png_data: rBytes,
            replace_page_content: true,
          });
        }

        // 2. All non-redact items (highlight, text, rect, circle, pen, signature, whiteout) go into a transparent overlay
        if (nonRedactItems.length > 0) {
          const unscaledW = rendered.width / saveScale;
          const unscaledH = rendered.height / saveScale;
          const bbox = calculateAnnotationsBoundingBox(nonRedactItems, unscaledW, unscaledH);

          const useCrop = bbox !== null && (bbox.width * bbox.height < unscaledW * unscaledH * 0.85);
          const offCanvas = document.createElement("canvas");
          const offCtx = offCanvas.getContext("2d")!;
          let overlayBbox: [number, number, number, number] | null = null;

          if (useCrop && bbox) {
            offCanvas.width = Math.ceil(bbox.width * saveScale);
            offCanvas.height = Math.ceil(bbox.height * saveScale);
            offCtx.save();
            offCtx.scale(saveScale, saveScale);
            offCtx.translate(-bbox.minX, -bbox.minY);
            for (const item of nonRedactItems) {
              drawSingleAnnotation(offCtx, item, saveScale);
            }
            offCtx.restore();

            overlayBbox = [bbox.minX, unscaledH - bbox.maxY, bbox.width, bbox.height];
          } else {
            offCanvas.width = rendered.width;
            offCanvas.height = rendered.height;
            offCtx.save();
            offCtx.scale(saveScale, saveScale);
            for (const item of nonRedactItems) {
              drawSingleAnnotation(offCtx, item, saveScale);
            }
            offCtx.restore();
          }

          const blob = await new Promise<Blob | null>((resolve) => offCanvas.toBlob(resolve, "image/png"));

          if (!blob) throw new Error(`Sayfa ${p} çizim katmanı işlenemedi`);
          const pngBytes = new Uint8Array(await blob.arrayBuffer());
          overlays.push({
            page: p,
            png_data: pngBytes,
            replace_page_content: false,
            bbox: overlayBbox,
          });
        }
      }


      // Filter out all 'redact' items from editor metadata so they cannot be selected,
      // moved, or deleted when reopening or clearing in PDFKit. They are now baked into the page pixels!
      const sanitizedAnnotations: Record<number, Annotation[]> = {};
      for (const [pageNumStr, list] of Object.entries(activeDoc.annotations)) {
        const pageNum = Number(pageNumStr);
        const nonRedact = list.filter((a) => a.type !== "redact");
        if (nonRedact.length > 0) {
          sanitizedAnnotations[pageNum] = nonRedact;
        }
      }

      let sourcePdfForOverlays = activeDoc.path;
      if (Object.keys(docFormValues).length > 0) {
        try {
          await fillForm(activeDoc.path, outputPath, docFormValues);
          sourcePdfForOverlays = outputPath;
        } catch (fe) {
          console.warn("fillForm save error:", fe);
        }
      }

      const editorData = JSON.stringify(sanitizedAnnotations);
      const res = await stampPageOverlays(
        sourcePdfForOverlays,
        outputPath,
        overlays,
        editorData,
        activeDoc.cleanPdfBytes ?? null
      );

      activeDoc.cleanPdfBytes = undefined;
      try {
        localStorage.setItem("pdfkit_ann_" + outputPath, editorData);
      } catch {}

      if (outputPath !== activeDoc.path) {
        updateDocPath(activeDoc.id, outputPath);
      }

      // Always reload doc handle with clean_pdf_bytes so in-memory handle reflects
      // permanent redactions in base pixels while keeping non-redact tools editable!
      try {
        const loadRes = await loadEditorDocument(outputPath);
        let cleanData: Uint8Array | undefined = undefined;
        if (loadRes.clean_pdf_bytes && loadRes.clean_pdf_bytes.length > 0) {
          cleanData = loadRes.clean_pdf_bytes instanceof Uint8Array
            ? loadRes.clean_pdf_bytes
            : new Uint8Array(loadRes.clean_pdf_bytes);
        }
        let refreshedHandle: any;
        if (cleanData && cleanData.length > 0) {
          try {
            refreshedHandle = await openPdf(outputPath, cleanData);
          } catch (e) {
            console.warn("Clean handle reload failed, falling back to disk path:", e);
            refreshedHandle = await openPdf(outputPath);
          }
        } else {
          refreshedHandle = await openPdf(outputPath);
        }
        await activeDoc.handle.destroy();
        activeDoc.handle = refreshedHandle;
        activeDoc.annotations = sanitizedAnnotations;
        await renderCurrentPage();
      } catch (reloadErr) {
        console.warn("Handle reload after save:", reloadErr);
      }

      markDocumentSaved(activeDoc.id);

      logActivity([baseName(activeDoc.path)], "Düzenlendi & Kaydedildi", outputPath);
      pushToast(`Başarıyla kaydedildi: ${baseName(res.outputs[0])} (${res.total_pages} sayfa)`, "success", {
        label: "Klasöre Git",
        path: outputPath,
      });
    } catch (e: any) {
      const errMsg = String(e?.message || e || "");
      if (
        errMsg.includes("açık tutuluyor") ||
        errMsg.includes("Erişim engellendi") ||
        errMsg.includes("os error 5") ||
        errMsg.includes("PermissionDenied")
      ) {
        pushToast(
          "Dosya başka bir programda açık olduğu için üzerine kaydedilemedi. Farklı kaydetme penceresi açılıyor...",
          "warning"
        );
        busy = false;
        await executeSave(true);
        return;
      }
      pushToast(`Kaydetme hatası: ${e}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window on:keydown|capture={handleKeyDown} />

<div class="flex flex-1 flex-col h-full overflow-hidden">
  {#if !isReadingMode}
    <!-- Adobe Acrobat Style Top Tab Bar -->
    <div class="flex items-center gap-1 border-b border-zinc-200 bg-zinc-200/70 px-2 pt-1.5 dark:border-zinc-800 dark:bg-zinc-900/90 shrink-0">
      <div class="flex flex-1 items-center gap-1 overflow-x-auto no-scrollbar">
        {#each $openDocs as d (d.id)}
          <div
            class="group flex max-w-[200px] items-center gap-1.5 rounded-t-lg border-t border-x px-3 py-1.5 text-xs font-medium transition-all cursor-pointer select-none
              {$activeDocId === d.id
                ? 'border-zinc-300 bg-white text-zinc-900 shadow-sm dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-100'
                : 'border-transparent bg-transparent text-zinc-500 hover:bg-zinc-300/50 hover:text-zinc-800 dark:text-zinc-400 dark:hover:bg-zinc-800/50'}"
            on:click={() => activeDocId.set(d.id)}
            role="button"
            tabindex="0"
            on:keydown={(e) => e.key === "Enter" && activeDocId.set(d.id)}
          >
            <FileText size={13} class={$activeDocId === d.id ? "text-indigo-600 dark:text-indigo-400" : "text-zinc-400"} />
            <span class="truncate">{d.name}</span>
            {#if d.hasUnsavedChanges}
              <span class="h-1.5 w-1.5 rounded-full bg-amber-500 shrink-0" title="Kaydedilmemiş değişiklikler var"></span>
            {/if}
            <button
              class="rounded p-0.5 opacity-60 hover:bg-zinc-200 hover:opacity-100 dark:hover:bg-zinc-700"
              title="Sekmeyi Kapat"
              on:click={(e) => handleCloseTab(d.id, e)}
            >
              <X size={12} />
            </button>
          </div>
        {/each}
      </div>

      <button
        class="flex items-center gap-1 rounded-lg px-2.5 py-1 text-xs font-semibold text-zinc-600 hover:bg-zinc-300/60 dark:text-zinc-400 dark:hover:bg-zinc-800"
        title="Yeni PDF Aç"
        on:click={openNewFileDialog}
      >
        <Plus size={14} /> Yeni PDF Aç
      </button>
    </div>
  {/if}

  {#if !activeDoc}
    <!-- Empty state when no PDF is loaded -->
    <div class="flex flex-1 flex-col items-center justify-center p-8">
      <div class="w-full max-w-md">
        <FileDropzone extensions={["pdf"]} label="Düzenlemek istediğiniz PDF'i buraya sürükleyin veya seçin" onFiles={handleFileDrop} />
      </div>
    </div>
  {:else}
    {#if !isReadingMode}
      <!-- Compact Professional Toolbar (Directly under tabs, NO giant title section) -->
      <div class="flex flex-wrap items-center justify-between gap-2 border-b border-zinc-200 bg-white px-3 py-1.5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900 shrink-0">
      <div class="flex items-center gap-1">
        <!-- Toggle sidebar button -->
        <button
          class="flex items-center gap-1 rounded-lg p-1.5 text-zinc-500 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800 mr-1"
          title={showSidebar ? "Kenar Çubuğunu Gizle" : "Kenar Çubuğunu Göster"}
          on:click={() => (showSidebar = !showSidebar)}
        >
          {#if showSidebar}<PanelLeftClose size={16} />{:else}<PanelLeftOpen size={16} />{/if}
        </button>

        <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-800 mr-1"></div>

        <!-- SEÇ -->
        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-all {currentTool === 'select' ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
          title="Seç / Taşı (Çoklu seçim için CTRL + Tık)"
          on:click={() => (currentTool = "select")}
        >
          <MousePointer size={14} /> Seç
        </button>

        <!-- METİN -->
        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-all {currentTool === 'text' ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
          title="Metin Ekle"
          on:click={() => { currentTool = "text"; }}
        >
          <Type size={14} /> Metin
        </button>

        <!-- KALEM -->
        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-all {currentTool === 'pen' ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
          title="Serbest Kalem"
          on:click={() => (currentTool = "pen")}
        >
          <PenLine size={14} /> Kalem
        </button>

        <!-- VURGULA SPLIT DROPDOWN -->
        <div id="highlighter-dropdown-container" class="relative inline-flex items-center">
          <button
            class="flex items-center gap-1 rounded-l-lg px-2 py-1 text-xs font-medium transition-all {currentTool === 'highlighter' ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
            title={selectedText ? "Seçili metni sarı ile vurgula" : "Fosforlu Vurgulayıcı"}
            on:click={() => {
              if (selectedText) {
                applyTextAction("highlight");
              } else {
                currentTool = "highlighter";
              }
            }}
          >
            <Highlighter size={14} /> Vurgula
          </button>
          <button
            class="flex items-center justify-center rounded-r-lg px-1 py-1 text-xs font-medium border-l border-zinc-200 dark:border-zinc-700 transition-all {currentTool === 'highlighter' ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
            title="Vurgulama ve Metin Biçimlendirme Seçenekleri"
            on:click|stopPropagation={() => (showHighlighterMenu = !showHighlighterMenu)}
          >
            <ChevronDown size={11} class="transition-transform {showHighlighterMenu ? 'rotate-180' : ''}" />
          </button>

          {#if showHighlighterMenu}
            <div
              class="absolute left-0 top-full mt-1.5 w-48 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 p-1.5 shadow-xl z-50 text-xs"
            >
              <button
                class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700 {currentTool === 'highlighter' ? 'font-semibold text-indigo-600 dark:text-indigo-400' : ''}"
                on:click={() => { currentTool = 'highlighter'; showHighlighterMenu = false; }}
              >
                <Highlighter size={14} />
                <span>Fosforlu Kalem</span>
              </button>
              <div class="h-px bg-zinc-100 dark:bg-zinc-700 my-1"></div>
              <div class="text-[10px] font-semibold text-zinc-400 dark:text-zinc-500 uppercase px-2 py-0.5">Seçili Metin İçin</div>
              <button
                class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700"
                on:click={() => { applyTextAction('highlight'); showHighlighterMenu = false; }}
              >
                <Highlighter size={14} class="text-amber-500" />
                <span>Metni Vurgula</span>
              </button>
              <button
                class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700"
                on:click={() => { applyTextAction('underline'); showHighlighterMenu = false; }}
              >
                <Underline size={14} class="text-rose-500" />
                <span>Altını Çiz</span>
              </button>
              <button
                class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700"
                on:click={() => { applyTextAction('strikethrough'); showHighlighterMenu = false; }}
              >
                <Strikethrough size={14} class="text-zinc-500" />
                <span>Üstünü Çiz</span>
              </button>
            </div>
          {/if}
        </div>

        <!-- SİLGİ DROPDOWN -->
        <div id="eraser-dropdown-container" class="relative inline-flex items-center">
          <button
            class="flex items-center gap-1 rounded-l-lg px-2 py-1 text-xs font-medium transition-all {currentTool === 'eraser' ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
            title="Silgi"
            on:click={() => { currentTool = "eraser"; showEraserMenu = false; }}
          >
            <Eraser size={14} /> Silgi
          </button>
          <button
            class="flex items-center justify-center rounded-r-lg px-1 py-1 text-xs font-medium border-l border-zinc-200 dark:border-zinc-700 transition-all {currentTool === 'eraser' ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
            title="Silgi Modu ve Boyut Ayarları"
            on:click|stopPropagation={() => (showEraserMenu = !showEraserMenu)}
          >
            <ChevronDown size={11} class="transition-transform {showEraserMenu ? 'rotate-180' : ''}" />
          </button>

          {#if showEraserMenu}
            <div
              class="absolute left-0 top-full mt-1.5 w-52 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 p-2 shadow-xl z-50 text-xs"
            >
              <div class="text-[11px] font-semibold text-zinc-500 uppercase tracking-wider mb-1 px-1">Silgi Modu</div>
              <div class="space-y-1 mb-2">
                <button
                  class="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-left text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700/60 {eraserMode === 'object' ? 'bg-indigo-50 font-semibold text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300' : ''}"
                  on:click={() => { eraserMode = 'object'; currentTool = 'eraser'; showEraserMenu = false; }}
                >
                  <div>
                    <div class="font-medium">Bölgesel</div>
                    <div class="text-[10px] text-zinc-400 font-normal">Çizilen objeyi tek tıkla siler</div>
                  </div>
                  {#if eraserMode === 'object'}<Check size={13} class="text-indigo-600 dark:text-indigo-400" />{/if}
                </button>

                <button
                  class="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-left text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700/60 {eraserMode === 'precision' ? 'bg-indigo-50 font-semibold text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300' : ''}"
                  on:click={() => { eraserMode = 'precision'; currentTool = 'eraser'; }}
                >
                  <div>
                    <div class="font-medium">Hassas</div>
                    <div class="text-[10px] text-zinc-400 font-normal">Kendi çizimlerinizi fırçayla siler</div>
                  </div>
                  {#if eraserMode === 'precision'}<Check size={13} class="text-indigo-600 dark:text-indigo-400" />{/if}
                </button>

                <button
                  class="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-left text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700/60 {eraserMode === 'whiteout' ? 'bg-indigo-50 font-semibold text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300' : ''}"
                  on:click={() => { eraserMode = 'whiteout'; currentTool = 'eraser'; }}
                >
                  <div>
                    <div class="font-medium">Daksil</div>
                    <div class="text-[10px] text-zinc-400 font-normal">Alanı beyazlatır (görsel gizler, metin altta kalır)</div>
                  </div>
                  {#if eraserMode === 'whiteout'}<Check size={13} class="text-indigo-600 dark:text-indigo-400" />{/if}
                </button>
              </div>

              {#if eraserMode === 'precision' || eraserMode === 'whiteout'}
                <div class="pt-2 border-t border-zinc-100 dark:border-zinc-700">
                  <div class="flex items-center justify-between text-xs text-zinc-600 dark:text-zinc-400 mb-1">
                    <span class="text-[11px] font-medium">Silgi Boyutu:</span>
                    <span class="font-mono text-[11px]">{eraserSize}px</span>
                  </div>
                  <input
                    type="range"
                    min="8"
                    max="80"
                    step="2"
                    bind:value={eraserSize}
                    class="h-1.5 w-full accent-indigo-600 cursor-pointer"
                  />
                </div>
              {/if}
            </div>
          {/if}
        </div>

        <!-- ŞEKİLLER SPLIT DROPDOWN -->
        <div id="shape-dropdown-container" class="relative inline-flex items-center">
          <button
            class="flex items-center gap-1 rounded-l-lg px-2 py-1 text-xs font-medium transition-all {['rect', 'circle', 'arrow', 'line'].includes(currentTool) ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
            title={activeShapeTool === 'rect' ? 'Kutu (Dörtgen) Çiz' : activeShapeTool === 'circle' ? 'Daire Çiz' : activeShapeTool === 'arrow' ? 'Ok Çiz' : 'Çizgi Çiz'}
            on:click={() => { currentTool = activeShapeTool; showShapeMenu = false; }}
          >
            {#if activeShapeTool === 'rect'}
              <Square size={14} /> <span>Kutu</span>
            {:else if activeShapeTool === 'circle'}
              <Circle size={14} /> <span>Daire</span>
            {:else if activeShapeTool === 'arrow'}
              <ArrowUpRight size={14} /> <span>Ok</span>
            {:else}
              <Minus size={14} /> <span>Çizgi</span>
            {/if}
          </button>
          <button
            class="flex items-center justify-center rounded-r-lg px-1 py-1 text-xs font-medium border-l border-zinc-200 dark:border-zinc-700 transition-all {['rect', 'circle', 'arrow', 'line'].includes(currentTool) ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
            title="Şekil Seçenekleri (Kutu, Daire, Ok, Çizgi)"
            on:click|stopPropagation={() => (showShapeMenu = !showShapeMenu)}
          >
            <ChevronDown size={11} class="transition-transform {showShapeMenu ? 'rotate-180' : ''}" />
          </button>

          {#if showShapeMenu}
            <div
              class="absolute left-0 top-full mt-1.5 w-36 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 p-1 shadow-xl z-50 text-xs"
            >
              <button
                class="flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700/60 {activeShapeTool === 'rect' ? 'font-semibold text-indigo-600 dark:text-indigo-400 bg-indigo-50/50 dark:bg-indigo-950/40' : ''}"
                on:click={() => selectShapeTool('rect')}
              >
                <span class="flex items-center gap-2"><Square size={14} /> Kutu</span>
                {#if activeShapeTool === 'rect'}<Check size={13} class="text-indigo-600 dark:text-indigo-400" />{/if}
              </button>

              <button
                class="flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700/60 {activeShapeTool === 'circle' ? 'font-semibold text-indigo-600 dark:text-indigo-400 bg-indigo-50/50 dark:bg-indigo-950/40' : ''}"
                on:click={() => selectShapeTool('circle')}
              >
                <span class="flex items-center gap-2"><Circle size={14} /> Daire</span>
                {#if activeShapeTool === 'circle'}<Check size={13} class="text-indigo-600 dark:text-indigo-400" />{/if}
              </button>

              <button
                class="flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700/60 {activeShapeTool === 'arrow' ? 'font-semibold text-indigo-600 dark:text-indigo-400 bg-indigo-50/50 dark:bg-indigo-950/40' : ''}"
                on:click={() => selectShapeTool('arrow')}
              >
                <span class="flex items-center gap-2"><ArrowUpRight size={14} /> Ok</span>
                {#if activeShapeTool === 'arrow'}<Check size={13} class="text-indigo-600 dark:text-indigo-400" />{/if}
              </button>

              <button
                class="flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700/60 {activeShapeTool === 'line' ? 'font-semibold text-indigo-600 dark:text-indigo-400 bg-indigo-50/50 dark:bg-indigo-950/40' : ''}"
                on:click={() => selectShapeTool('line')}
              >
                <span class="flex items-center gap-2"><Minus size={14} /> Çizgi</span>
                {#if activeShapeTool === 'line'}<Check size={13} class="text-indigo-600 dark:text-indigo-400" />{/if}
              </button>
            </div>
          {/if}
        </div>

        <!-- BELGE EYLEMLERİ (Sansür, İmza, Filigran Kaldır) -->
        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-all {currentTool === 'redact' ? 'bg-indigo-600 text-white shadow' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
          title="Sansürle / Karart (Özel verileri gizle)"
          on:click={() => (currentTool = "redact")}
        >
          <ShieldAlert size={14} class="text-rose-500" /> Sansür
        </button>

        <button
          class="flex items-center gap-1 rounded-lg bg-indigo-50 px-2 py-1 text-xs font-semibold text-indigo-700 hover:bg-indigo-100 dark:bg-indigo-950 dark:text-indigo-300 dark:hover:bg-indigo-900"
          title="İmza Ekle"
          on:click={() => (showSignatureModal = true)}
        >
          <FileSignature size={14} /> İmza
        </button>

        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-semibold transition-all {activeDoc?.cleanPdfBytes ? 'bg-zinc-100 text-zinc-400 dark:bg-zinc-800/80 dark:text-zinc-500 cursor-not-allowed opacity-60' : 'bg-rose-50 text-rose-700 hover:bg-rose-100 dark:bg-rose-950 dark:text-rose-300 dark:hover:bg-rose-900'}"
          title={activeDoc?.cleanPdfBytes ? "Bu belgedeki filigran zaten temizlendi (Eski haline dönmek için Ctrl+Z yapabilirsiniz)" : "Filigranı Kaldır"}
          disabled={!activeDoc || !!activeDoc.cleanPdfBytes}
          on:click={() => {
            if (activeDoc?.cleanPdfBytes) {
              pushToast("Bu belgedeki filigran zaten temizlendi. Eski haline dönmek için 'Geri Al' (Ctrl+Z) yapabilirsiniz.", "info");
              return;
            }
            showRemoveWatermarkModal = true;
          }}
        >
          {#if activeDoc?.cleanPdfBytes}
            <Check size={14} class="text-emerald-500" /> Filigran Kaldırıldı
          {:else}
            <Stamp size={14} /> Filigranı Kaldır
          {/if}
        </button>
      </div>

      <!-- Controls on right -->
      <div class="flex items-center gap-2">
        <!-- STİL & RENK AÇILIR PANELİ ("dörtgen.png" gibi) -->
        {#if ['pen', 'rect', 'circle', 'arrow', 'line', 'text'].includes(currentTool) || selectedAnnotationIds.length > 0}
          <div id="style-popover-container" class="relative">
            <button
              class="flex items-center gap-1.5 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 px-2.5 py-1 text-xs font-medium text-zinc-700 dark:text-zinc-200 hover:bg-zinc-50 dark:hover:bg-zinc-700/60 shadow-xs transition-all {showStylePopover ? 'ring-2 ring-indigo-500' : ''}"
              title="Renk ve Çizim Stili Paneli"
              on:click|stopPropagation={() => (showStylePopover = !showStylePopover)}
            >
              <!-- Dual swatch indicator (fill inside, border outside) -->
              <span
                class="relative flex h-3.5 w-3.5 items-center justify-center rounded-full border border-zinc-300 dark:border-zinc-600 shadow-inner overflow-hidden"
                style="background-color: {currentFillColor !== 'transparent' ? currentFillColor : '#ffffff'}"
              >
                {#if currentFillColor === 'transparent'}
                  <span class="absolute h-0.5 w-4 rotate-45 bg-rose-500"></span>
                {/if}
                <span class="h-1.5 w-1.5 rounded-full border border-black/20" style="background-color: {currentColor}"></span>
              </span>
              <span>Stil</span>
              <ChevronDown size={11} class="text-zinc-400 transition-transform {showStylePopover ? 'rotate-180' : ''}" />
            </button>

            {#if showStylePopover}
              <div
                class="absolute right-0 top-full mt-2 w-64 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 p-3 shadow-2xl z-50 text-xs"
              >
                <!-- Title -->
                <div class="text-[13px] font-semibold text-zinc-800 dark:text-zinc-100 mb-2">
                  {stylePopoverTitle}
                </div>

                <!-- Tabs: Doldur / Kenarlık -->
                <div class="flex items-center gap-4 text-xs mb-3 border-b border-zinc-100 dark:border-zinc-800 pb-1">
                  <button
                    type="button"
                    class="transition-colors pb-1 {activeStyleTab === 'fill' ? 'font-semibold text-zinc-900 dark:text-white border-b-2 border-zinc-900 dark:border-white' : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400'}"
                    on:click={() => (activeStyleTab = 'fill')}
                  >
                    Doldur
                  </button>
                  <button
                    type="button"
                    class="transition-colors pb-1 {activeStyleTab === 'stroke' ? 'font-semibold text-zinc-900 dark:text-white border-b-2 border-zinc-900 dark:border-white' : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400'}"
                    on:click={() => (activeStyleTab = 'stroke')}
                  >
                    Kenarlık
                  </button>
                </div>

                <!-- Doldur Tab Content -->
                {#if activeStyleTab === 'fill'}
                  <button
                    type="button"
                    class="w-full mb-2.5 flex items-center justify-between px-2 py-1 rounded-md border text-[11px] transition-all {currentFillColor === 'transparent' ? 'border-indigo-500 bg-indigo-50 text-indigo-700 font-medium dark:bg-indigo-950/60 dark:text-indigo-300' : 'border-zinc-200 dark:border-zinc-700 hover:bg-zinc-50 dark:hover:bg-zinc-800 text-zinc-600 dark:text-zinc-300'}"
                    on:click={() => setFillColor('transparent')}
                  >
                    <span class="flex items-center gap-1.5">
                      <span class="h-3.5 w-3.5 rounded-full border border-dashed border-zinc-400 flex items-center justify-center text-[10px] text-zinc-400 font-bold">∅</span>
                      Saydam (İçini Doldurma)
                    </span>
                    {#if currentFillColor === 'transparent'}
                      <Check size={13} class="text-indigo-600 dark:text-indigo-400" />
                    {/if}
                  </button>

                  <div class="grid grid-cols-7 gap-1.5 mb-3">
                    {#each adobeSwatches as row}
                      {#each row as color}
                        <button
                          type="button"
                          class="relative h-6 w-6 rounded border border-black/15 flex items-center justify-center transition-transform hover:scale-110 active:scale-95 shadow-xs"
                          style="background-color: {color};"
                          title={color}
                          on:click={() => setFillColor(color)}
                        >
                          {#if currentFillColor.toLowerCase() === color.toLowerCase()}
                            <Check size={14} class="{isDarkColor(color) ? 'text-white' : 'text-zinc-900'} stroke-[3]" />
                          {/if}
                        </button>
                      {/each}
                    {/each}
                  </div>
                {:else}
                  <!-- Kenarlık Tab Content -->
                  <div class="grid grid-cols-7 gap-1.5 mb-3">
                    {#each adobeSwatches as row}
                      {#each row as color}
                        <button
                          type="button"
                          class="relative h-6 w-6 rounded border border-black/15 flex items-center justify-center transition-transform hover:scale-110 active:scale-95 shadow-xs"
                          style="background-color: {color};"
                          title={color}
                          on:click={() => setStrokeColor(color)}
                        >
                          {#if currentColor.toLowerCase() === color.toLowerCase()}
                            <Check size={14} class="{isDarkColor(color) ? 'text-white' : 'text-zinc-900'} stroke-[3]" />
                          {/if}
                        </button>
                      {/each}
                    {/each}
                  </div>

                  <!-- Çizgi Kalınlığı Presets -->
                  <div class="mb-3 pt-1 border-t border-zinc-100 dark:border-zinc-800">
                    <div class="flex items-center justify-between text-[11px] font-medium text-zinc-600 dark:text-zinc-400 mb-1.5">
                      <span>Çizgi Kalınlığı</span>
                      <span class="font-mono text-zinc-500">{currentStrokeWidth}px</span>
                    </div>
                    <div class="grid grid-cols-5 gap-1">
                      {#each [1, 2, 3, 5, 8] as w}
                        <button
                          type="button"
                          class="py-0.5 rounded text-[11px] font-medium border text-center transition-all {currentStrokeWidth === w ? 'border-indigo-600 bg-indigo-50 text-indigo-700 font-semibold dark:bg-indigo-950 dark:text-indigo-300' : 'border-zinc-200 dark:border-zinc-700 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-50 dark:hover:bg-zinc-800'}"
                          on:click={() => setStrokeWidth(w)}
                        >
                          {w}px
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}

                <!-- Opaklık Slider (Adobe "dörtgen.png" birebir) -->
                <div class="pt-2 border-t border-zinc-100 dark:border-zinc-800">
                  <div class="flex items-center justify-between text-xs text-zinc-700 dark:text-zinc-300 mb-1.5">
                    <span class="text-[11px] font-medium">Opaklık</span>
                    <span class="font-mono text-[11px] text-zinc-500 dark:text-zinc-400">{currentOpacity}%</span>
                  </div>
                  <input
                    type="range"
                    min="10"
                    max="100"
                    step="1"
                    bind:value={currentOpacity}
                    on:input={() => setOpacity(currentOpacity)}
                    class="h-1.5 w-full accent-zinc-800 dark:accent-indigo-500 cursor-pointer"
                  />
                </div>
              </div>
            {/if}
          </div>
        {/if}

        {#if currentTool === "text" || (selectedAnnotationIds.length > 0 && activeDoc?.annotations[activeDoc.currentPage]?.some(a => selectedAnnotationIds.includes(a.id) && a.type === 'text'))}
          <select
            bind:value={currentFontSize}
            on:change={() => { if (selectedAnnotationIds.length > 0) updateSelectedAnnotationsStyle({ fontSize: currentFontSize }); }}
            class="rounded border border-zinc-200 bg-white px-1.5 py-0.5 text-xs dark:border-zinc-700 dark:bg-zinc-800 dark:text-white"
          >
            <option value={12}>12 pt</option>
            <option value={14}>14 pt</option>
            <option value={18}>18 pt</option>
            <option value={24}>24 pt</option>
            <option value={32}>32 pt</option>
          </select>
        {/if}

        <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-800"></div>

        <!-- İşlem Geçmişi & Temizleme Grubu -->
        <div class="flex items-center gap-0.5">
          <button
            class="rounded p-1 text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800 disabled:opacity-30 disabled:pointer-events-none transition-colors"
            title="Geri Al (Ctrl+Z)"
            disabled={!hasUndo}
            on:click={undoAction}
          >
            <Undo2 size={15} />
          </button>

          <button
            class="rounded p-1 text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800 disabled:opacity-30 disabled:pointer-events-none transition-colors"
            title="Yinele (Ctrl+Y)"
            disabled={!hasRedo}
            on:click={redoAction}
          >
            <Redo2 size={15} />
          </button>

          <button
            class="rounded p-1 transition-colors {selectedAnnotationIds.length > 0 ? 'text-rose-600 hover:bg-rose-50 dark:hover:bg-rose-950/40 cursor-pointer' : 'text-zinc-400 opacity-40 cursor-not-allowed'}"
            title={selectedAnnotationIds.length > 1 ? `${selectedAnnotationIds.length} nesneyi sil (Delete)` : "Seçili objeyi sil (Delete)"}
            disabled={selectedAnnotationIds.length === 0}
            on:click={deleteSelectedAnnotation}
          >
            <Trash2 size={15} />
          </button>

          <button
            class="flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium text-zinc-600 hover:bg-rose-50 hover:text-rose-600 dark:text-zinc-400 dark:hover:bg-rose-950/30 dark:hover:text-rose-400 transition-colors ml-0.5"
            title="Bu sayfadaki çizimleri temizle"
            on:click={clearPage}
          >
            <Sparkles size={13} />
            <span>Sayfayı Temizle</span>
          </button>
        </div>

        <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-800"></div>

        <!-- Arama ve Yazdırma Grubu -->
        <div class="flex items-center gap-0.5">
          <button
            class="rounded p-1 transition-colors {showSearchBar ? 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/60 dark:text-indigo-300' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'}"
            title="Belge İçi Arama (Ctrl+F)"
            on:click={() => (showSearchBar = !showSearchBar)}
          >
            <Search size={15} />
          </button>

          <button
            class="rounded p-1 text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors"
            title="Yazdır (Ctrl+P)"
            on:click={() => (showPrintModal = true)}
          >
            <Printer size={15} />
          </button>
        </div>

        <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-800"></div>

        <!-- Yakınlaştırma (Zoom) Grubu -->
        <div id="zoom-dropdown-container" class="relative flex items-center gap-0.5">
          <button class="rounded p-1 text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800" title="Uzaklaştır (Ctrl+-)" on:click={() => changeZoom(-0.2)}>
            <ZoomOut size={15} />
          </button>
          <button
            type="button"
            class="flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-zinc-600 hover:text-indigo-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:text-indigo-400 dark:hover:bg-zinc-800 font-mono cursor-pointer transition-colors active:scale-95"
            title="Yakınlaştırma ve Sığdırma Seçenekleri"
            on:click|stopPropagation={() => (showZoomMenu = !showZoomMenu)}
          >
            <span>%{Math.round(activeDoc.scale * 100)}</span>
            <ChevronDown size={10} class="text-zinc-400" />
          </button>
          <button class="rounded p-1 text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800" title="Yakınlaştır (Ctrl++)" on:click={() => changeZoom(0.2)}>
            <ZoomIn size={15} />
          </button>

          {#if showZoomMenu}
            <div
              class="absolute left-1/2 -translate-x-1/2 top-full mt-1.5 w-44 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 p-1.5 shadow-xl z-50 text-xs"
            >
              <div class="text-[10px] font-semibold text-zinc-400 dark:text-zinc-500 uppercase px-2 py-1">Sığdırma</div>
              <button
                class="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700"
                on:click={fitToWidth}
              >
                <span>Genişliğe Sığdır</span>
              </button>
              <button
                class="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700"
                on:click={fitToPage}
              >
                <span>Sayfaya Sığdır</span>
              </button>
              <div class="h-px bg-zinc-100 dark:bg-zinc-700 my-1"></div>
              <div class="text-[10px] font-semibold text-zinc-400 dark:text-zinc-500 uppercase px-2 py-1">Ön Ayarlar</div>
              {#each [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] as preset}
                <button
                  class="flex w-full items-center justify-between rounded-lg px-2 py-1 text-zinc-700 hover:bg-zinc-100 dark:text-zinc-200 dark:hover:bg-zinc-700 {Math.abs(activeDoc.scale - preset) < 0.05 ? 'font-bold text-indigo-600 dark:text-indigo-400' : ''}"
                  on:click={() => setZoomPreset(preset)}
                >
                  <span>%{Math.round(preset * 100)}</span>
                  {#if Math.abs(activeDoc.scale - preset) < 0.05}
                    <Check size={12} />
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>

        <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-800"></div>

        <!-- Görünüm Modu: Tek Sayfa / Sürekli Kaydırma -->
        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-all {viewLayout === 'continuous' ? 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/60 dark:text-indigo-300 font-semibold' : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
          title={viewLayout === 'continuous' ? 'Tek Sayfa Görünümüne Geç' : 'Sürekli Kaydırma Görünümüne Geç'}
          on:click={() => {
            viewLayout = viewLayout === 'single' ? 'continuous' : 'single';
            if (viewLayout === 'single') {
              tick().then(renderCurrentPage);
            } else {
              visiblePageRange = { start: Math.max(1, activeDoc.currentPage - 2), end: Math.min(activeDoc.pageCount, activeDoc.currentPage + 2) };
              tick().then(() => document.getElementById(`continuous-page-${activeDoc.currentPage}`)?.scrollIntoView());
            }
          }}
        >
          <Layers size={14} />
          <span>{viewLayout === 'continuous' ? 'Sürekli' : 'Tek Sayfa'}</span>
        </button>

        <!-- Okuma Modu / Tam Ekran (F11) -->
        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-all"
          title="Okuma Modu / Tam Ekran (F11)"
          on:click={toggleReadingMode}
        >
          <Maximize size={14} />
        </button>

        <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-800"></div>

        <!-- Split Kaydet Butonu (Left margin 0-20% Chevron Down Dropdown) -->
        <div class="relative inline-flex items-center rounded-xl bg-indigo-600 text-white shadow-md shadow-indigo-500/20 {activeDoc.hasUnsavedChanges ? 'ring-2 ring-indigo-400 ring-offset-1 dark:ring-offset-zinc-900' : ''}">
          <button
            type="button"
            class="flex h-8 w-7 items-center justify-center rounded-l-xl border-r border-indigo-500/40 hover:bg-indigo-700 active:bg-indigo-800 transition-colors"
            title="Dışa Aktarma ve Farklı Kaydet Seçenekleri"
            on:click|stopPropagation={() => (showSaveMenu = !showSaveMenu)}
          >
            <ChevronDown size={14} class="transition-transform duration-150 {showSaveMenu ? 'rotate-180' : ''}" />
          </button>

          <button
            type="button"
            class="flex h-8 items-center gap-1.5 px-3 rounded-r-xl hover:bg-indigo-700 active:bg-indigo-800 text-xs font-semibold transition-colors disabled:opacity-50"
            disabled={busy}
            on:click={() => handleSaveTrigger(false)}
            title="Kaydet (Ctrl+S)"
          >
            <Save size={14} class={activeDoc.hasUnsavedChanges ? "text-amber-300" : ""} />
            <span>{busy ? "Kaydediliyor..." : "Kaydet"}</span>
            {#if activeDoc.hasUnsavedChanges}
              <span class="h-1.5 w-1.5 rounded-full bg-amber-400 shrink-0" title="Kaydedilmemiş değişiklikler var"></span>
            {/if}
          </button>

          {#if showSaveMenu}
            <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
            <div
              class="fixed inset-0 z-40"
              on:click={() => (showSaveMenu = false)}
            ></div>
            <div class="absolute right-0 top-full mt-1.5 z-50 w-64 rounded-xl border border-zinc-200 bg-white p-1.5 shadow-2xl dark:border-zinc-700 dark:bg-zinc-800 text-zinc-800 dark:text-zinc-200 text-xs font-medium animate-in fade-in zoom-in-95 duration-100">
              <button
                class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 hover:bg-indigo-50 hover:text-indigo-600 dark:hover:bg-indigo-950/40 dark:hover:text-indigo-300 transition-colors"
                on:click={() => {
                  showSaveMenu = false;
                  showExportConvertModal = true;
                }}
              >
                <FileText size={16} class="text-blue-600 shrink-0" />
                <div class="text-left">
                  <div class="font-bold">Dönüştür (DOCX, XLSX, Görsel...)</div>
                  <div class="text-[10px] text-zinc-400">PDF'i farklı dosya formatlarına aktar</div>
                </div>
              </button>

              <button
                class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 hover:bg-zinc-100 dark:hover:bg-zinc-700/60 transition-colors"
                on:click={() => {
                  showSaveMenu = false;
                  handleSaveTrigger(true);
                }}
              >
                <Save size={16} class="text-emerald-600 shrink-0" />
                <div class="text-left">
                  <div class="font-semibold">Farklı Kaydet (PDF)...</div>
                  <div class="text-[10px] text-zinc-400">Yeni dosya adı ile kaydet (Ctrl+Shift+S)</div>
                </div>
              </button>

              <button
                class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 hover:bg-zinc-100 dark:hover:bg-zinc-700/60 transition-colors"
                on:click={() => {
                  showSaveMenu = false;
                  showPrintModal = true;
                }}
              >
                <Printer size={16} class="text-zinc-500 shrink-0" />
                <div class="text-left">
                  <div class="font-semibold">Yazdır...</div>
                  <div class="text-[10px] text-zinc-400">Yazıcıya gönder (Ctrl+P)</div>
                </div>
              </button>
            </div>
          {/if}
        </div>
      </div>
    </div>
    {/if}

    <!-- Main Workspace with Collapsible Sidebar -->
    <div class="flex flex-1 min-h-0 overflow-hidden">
      <!-- Collapsible & Resizable Thumbnail Sidebar -->
      {#if showSidebar && !isReadingMode}
        <div
          class="relative flex shrink-0 flex-col gap-1.5 overflow-y-auto border-r border-zinc-200 bg-zinc-50 p-2 dark:border-zinc-800 dark:bg-zinc-900/60 select-none transition-[width] duration-75"
          style="width: {sidebarWidth}px;"
        >
          {#each Array.from({ length: activeDoc.pageCount }, (_, i) => i + 1) as p (activeDoc.id + '-p-' + p)}
            {@const hasEdits = activeDoc.annotations[p] && activeDoc.annotations[p].length > 0}
            {@const isDragTarget = isDraggingItem && dragOverSidebarPage === p}
            <button
              data-page-thumb={p}
              class="relative flex flex-col items-center rounded-lg border p-1 transition-all duration-150
                {isDragTarget
                  ? 'border-indigo-600 bg-indigo-100 ring-2 ring-indigo-500 scale-[1.04] shadow-lg dark:bg-indigo-950/80 z-20'
                  : activeDoc.currentPage === p
                    ? 'border-indigo-600 bg-indigo-50/70 shadow-sm ring-1 ring-indigo-500/30 dark:bg-indigo-950/40'
                    : 'border-zinc-200 bg-white hover:border-zinc-300 dark:border-zinc-800 dark:bg-zinc-900'}"
              on:click={() => setPage(p)}
              on:mouseenter={() => {
                if (isDraggingItem && selectedAnnotationIds.length > 0 && activeDoc && activeDoc.currentPage !== p) {
                  dragOverSidebarPage = p;
                }
              }}
              on:mouseleave={() => {
                if (isDraggingItem && dragOverSidebarPage === p) {
                  dragOverSidebarPage = null;
                }
              }}
              on:mouseup={() => {
                if (isDraggingItem && selectedAnnotationIds.length > 0 && activeDoc && activeDoc.currentPage !== p) {
                  transferSelectedToPage(p, "current");
                }
              }}
            >
              {#if isDragTarget}
                <div class="pointer-events-none absolute inset-0 z-30 flex flex-col items-center justify-center rounded-lg bg-indigo-600/85 text-white shadow-md backdrop-blur-[1px] animate-in fade-in zoom-in-95 duration-100">
                  <span class="text-[11px] font-bold">Buraya Bırak</span>
                  <span class="text-[9px] opacity-90">Sayfa {p}</span>
                </div>
              {/if}
              <SidebarPageThumb
                page={p}
                handle={activeDoc.handle}
                docVersion={docVersion}
                pageVersion={pageVersions[p] || 0}
                annotations={activeDoc.annotations[p] || []}
              />
              <div class="mt-1 flex w-full items-center justify-between text-[10px] text-zinc-500 px-0.5">
                <span>s. {p}</span>
                {#if hasEdits}
                  <span class="h-2 w-2 rounded-full bg-indigo-500 shrink-0" title="Düzenlendi"></span>
                {/if}
              </div>
            </button>
          {/each}
        </div>

        <!-- Draggable Resizer Border -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          class="group relative -ml-1 z-10 w-2.5 cursor-col-resize hover:bg-indigo-500/20 active:bg-indigo-500/40 transition-colors flex items-center justify-center shrink-0 {isResizingSidebar ? 'bg-indigo-500/30' : ''}"
          on:mousedown={startResizeSidebar}
          title="Kenar çubuğunu genişletmek için sürükleyin"
        >
          <div class="h-8 w-0.5 rounded-full bg-zinc-300 group-hover:bg-indigo-500 dark:bg-zinc-700 transition-colors"></div>
        </div>
      {/if}

      <!-- Center: Maximize PDF Canvas Viewport -->
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div
        bind:this={canvasContainer}
        on:wheel={viewLayout === 'single' ? handleContainerWheel : undefined}
        on:scroll={handleContinuousScroll}
        on:mousedown={handleContainerMouseDown}
        on:auxclick={(e) => { if (e.button === 1) e.preventDefault(); }}
        style={isMiddlePanning ? "cursor: grabbing !important; user-select: none;" : ""}
        class="relative flex-1 overflow-auto bg-zinc-200/60 p-6 dark:bg-zinc-950"
      >
        {#if showSearchBar && activeDoc}
          <PdfSearchBar
            doc={activeDoc}
            onJumpToPage={(p) => setPage(p)}
            onHighlightMatch={(m) => {
              activeSearchMatch = m;
              redrawOverlay();
            }}
            onClose={() => {
              showSearchBar = false;
              activeSearchMatch = null;
              redrawOverlay();
            }}
          />
        {/if}

        <!-- Floating Quick Page Pill Indicator (Smooth Visual Confirmation) -->
        {#if showPageChangeBadge && activeDoc}
          <div class="pointer-events-none sticky top-2 z-40 mx-auto w-fit flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-zinc-900/90 text-white text-xs font-semibold shadow-xl backdrop-blur-md transition-all">
            {#if pageTransitionDirection === "down"}
              <ChevronDown size={14} class="text-indigo-400 animate-bounce" />
            {:else if pageTransitionDirection === "up"}
              <ChevronUp size={14} class="text-indigo-400 animate-bounce" />
            {/if}
            <span>Sayfa {activeDoc.currentPage} / {activeDoc.pageCount}</span>
          </div>
        {/if}

        {#if viewLayout === "continuous" && activeDoc}
          <div class="flex flex-col items-center py-4">
            {#each Array.from({ length: activeDoc.pageCount }, (_, i) => i + 1) as p (p)}
              <ContinuousPage
                page={p}
                handle={activeDoc.handle}
                scale={activeDoc.scale}
                isVisible={p >= visiblePageRange.start && p <= visiblePageRange.end}
                isCurrentPage={p === activeDoc.currentPage}
                annotations={activeDoc.annotations[p] || []}
                onSelectPage={(selectedP) => {
                  if (activeDoc && selectedP !== activeDoc.currentPage) {
                    updateDocPage(activeDoc.id, selectedP);
                  }
                }}
                onLinkClick={handleLinkClick}
              />
            {/each}
          </div>
        {:else if activeDoc}
          <div class="min-w-fit min-h-fit flex flex-col items-center justify-center m-auto">
            <!-- Page Header Strip / Distinction -->
            <div class="w-full flex items-center justify-between pb-2 px-1 text-xs select-none">
              <div class="flex items-center gap-2">
                <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-200 text-xs font-semibold shadow-xs border border-zinc-200/80 dark:border-zinc-700/80">
                  <FileText size={12} class="text-indigo-600 dark:text-indigo-400" />
                  Sayfa {activeDoc.currentPage} / {activeDoc.pageCount}
                </span>
                {#if activeDoc.currentPage > 1}
                  <span class="text-[11px] text-zinc-400 dark:text-zinc-500 hidden sm:inline">
                    ↑ Önceki: Sayfa {activeDoc.currentPage - 1}
                  </span>
                {/if}
              </div>
              {#if activeDoc.currentPage < activeDoc.pageCount}
                <span class="text-[11px] text-zinc-400 dark:text-zinc-500 hidden sm:inline">
                  ↓ Sonraki: Sayfa {activeDoc.currentPage + 1}
                </span>
              {/if}
            </div>

            <div
              class="relative shadow-2xl rounded transition-all duration-200 ease-out {pageTransitionClass}"
              on:mousemove={handleWrapperMouseMove}
            >
              <canvas bind:this={pdfCanvas} class="block rounded bg-white"></canvas>

              <!-- Text Layer (Transparent Selectable Text) -->
              <div bind:this={textLayerDiv} class="textLayer absolute inset-0 select-text"></div>

              <!-- Clickable Hyperlinks -->
              <div class="absolute inset-0 pointer-events-none">
                {#each currentLinks as link}
                  <button
                    type="button"
                    class="absolute pointer-events-auto cursor-pointer rounded-xs bg-indigo-500/10 hover:bg-indigo-500/25 transition-colors border border-transparent hover:border-indigo-500/40"
                    style="left: {link.rect[0]}px; top: {link.rect[1]}px; width: {link.rect[2] - link.rect[0]}px; height: {link.rect[3] - link.rect[1]}px;"
                    title={link.url ? link.url : link.destPage ? `Sayfa ${link.destPage}'e git` : 'Bağlantı'}
                    on:click={(e) => handleLinkClick(link, e)}
                  ></button>
                {/each}
              </div>

              <!-- Interactive AcroForm Fields -->
              {#if currentFormFields.length > 0}
                <div class="absolute inset-0 pointer-events-none z-10">
                  {#each currentFormFields as field}
                    {#if field.type === "checkbox"}
                      <input
                        type="checkbox"
                        class="absolute pointer-events-auto cursor-pointer rounded text-indigo-600 focus:ring-indigo-500 border-zinc-400"
                        style="left: {field.rect[0]}px; top: {field.rect[1]}px; width: {field.rect[2] - field.rect[0]}px; height: {field.rect[3] - field.rect[1]}px;"
                        checked={docFormValues[field.name] === "Yes" || (!docFormValues[field.name] && field.value === "Yes")}
                        disabled={field.readOnly}
                        on:change={(e) => {
                          docFormValues[field.name] = e.currentTarget.checked ? "Yes" : "Off";
                          if (activeDoc) markDocumentModified(activeDoc.id);
                        }}
                      />
                    {:else}
                      <input
                        type="text"
                        class="absolute pointer-events-auto rounded bg-blue-50/40 hover:bg-white focus:bg-white px-1 py-0.5 text-xs text-zinc-900 border border-blue-400/60 focus:border-indigo-600 focus:outline-none shadow-xs transition-colors"
                        style="left: {field.rect[0]}px; top: {field.rect[1]}px; width: {field.rect[2] - field.rect[0]}px; height: {field.rect[3] - field.rect[1]}px; font-size: {Math.max(9, Math.min(14, (field.rect[3] - field.rect[1]) * 0.7))}px;"
                        value={docFormValues[field.name] !== undefined ? docFormValues[field.name] : field.value}
                        placeholder={field.name}
                        disabled={field.readOnly}
                        on:input={(e) => {
                          docFormValues[field.name] = e.currentTarget.value;
                          if (activeDoc) markDocumentModified(activeDoc.id);
                        }}
                      />
                    {/if}
                  {/each}
                </div>
              {/if}


              <canvas
                bind:this={overlayCanvas}
                style="cursor: {canvasCursor}; pointer-events: {overlayPointerEvents};"
                class="absolute inset-0 touch-none"
                on:mousedown={onMouseDown}
                on:mousemove={onMouseMove}
                on:mouseup={onMouseUp}
                on:mouseenter={() => (isMouseOverCanvas = true)}
                on:mouseleave={onMouseLeave}
              ></canvas>

              <!-- Circular Eraser Cursor Preview: Active only for Hassas and Daksil -->
              {#if currentTool === "eraser" && (eraserMode === "precision" || eraserMode === "whiteout") && isMouseOverCanvas && eraserClientPos}
                <div
                  class="pointer-events-none absolute -translate-x-1/2 -translate-y-1/2 rounded-full border-2 {eraserMode === 'whiteout' ? 'border-amber-500 bg-white/40' : 'border-indigo-600 bg-indigo-500/20'} shadow-xs flex items-center justify-center z-20"
                  style="left: {eraserClientPos.x}px; top: {eraserClientPos.y}px; width: {eraserDisplayDiameter}px; height: {eraserDisplayDiameter}px;"
                >
                  <div class="h-1 w-1 rounded-full {eraserMode === 'whiteout' ? 'bg-amber-500' : 'bg-indigo-600'}"></div>
                </div>
              {/if}

              <!-- Floating Inline Text Input: Uses exact CSS coordinates & reliable keyboard / button confirm -->
              {#if textInputVisible}
                <div
                  class="absolute z-30 flex items-center gap-1 rounded-xl border-2 border-purple-500 p-1.5 shadow-xl backdrop-blur-md"
                  style="left: {textInputCssX}px; top: {textInputCssY}px; background-color: rgba(255, 255, 255, 0.85);"
                >
                  <input
                    bind:this={textInputRef}
                    bind:value={textInputVal}
                    placeholder="Metin yazın..."
                    class="rounded border-none bg-transparent px-2 py-0.5 font-medium outline-none text-zinc-900"
                    style="font-size: {currentFontSize}px; color: {currentColor}; font-family: sans-serif;"
                    on:keydown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        commitTextInput();
                      }
                      if (e.key === "Escape") {
                        e.preventDefault();
                        textInputVisible = false;
                        textInputVal = "";
                      }
                    }}
                  />
                  <button
                    class="rounded-lg p-1 text-emerald-600 hover:bg-emerald-100/60"
                    title="Onayla (Enter)"
                    on:mousedown|stopPropagation|preventDefault
                    on:click|stopPropagation={commitTextInput}
                  >
                    <Check size={16} />
                  </button>
                  <button
                    class="rounded-lg p-1 text-rose-500 hover:bg-rose-100/60"
                    title="İptal (Esc)"
                    on:mousedown|stopPropagation|preventDefault
                    on:click|stopPropagation={() => { textInputVisible = false; textInputVal = ""; }}
                  >
                    <X size={16} />
                  </button>
                </div>
              {/if}
            </div>

            <!-- Page Footer Strip / Boundary Separator -->
            <div class="w-full flex items-center justify-center pt-3 mt-1 text-[11px] select-none text-zinc-400 dark:text-zinc-500">
              <div class="flex items-center gap-2">
                <div class="h-px w-10 bg-zinc-300 dark:bg-zinc-700"></div>
                <span>Sayfa {activeDoc.currentPage} Sonu</span>
                {#if activeDoc.currentPage < activeDoc.pageCount}
                  <span class="text-zinc-500 dark:text-zinc-400 font-medium">(Aşağı kaydırarak veya nesneyi alta sürükleyerek s. {activeDoc.currentPage + 1}'e geçin)</span>
                {:else}
                  <span class="text-emerald-600 dark:text-emerald-400 font-medium">✓ Belge Sonu</span>
                {/if}
                <div class="h-px w-10 bg-zinc-300 dark:bg-zinc-700"></div>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- Floating Selection Action Bar for Selected Text -->
      {#if selectedText && selectedTextRect}
        <div
          class="fixed z-50 flex items-center gap-1 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-white/95 dark:bg-zinc-900/95 p-1 shadow-2xl backdrop-blur-md animate-in fade-in zoom-in-95 duration-150 text-xs"
          style="left: {selectedTextRect.left}px; top: {Math.max(10, selectedTextRect.top - 40)}px; transform: translateX(-50%);"
        >
          <button
            class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-amber-50 text-amber-600 dark:hover:bg-amber-950/40 font-medium transition-colors"
            title="Seçili metni vurgula"
            on:click={() => applyTextAction("highlight")}
          >
            <Highlighter size={13} />
            <span>Vurgula</span>
          </button>
          <button
            class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-rose-50 text-rose-600 dark:hover:bg-rose-950/40 font-medium transition-colors"
            title="Seçili metnin altını çiz"
            on:click={() => applyTextAction("underline")}
          >
            <Underline size={13} />
            <span>Altını Çiz</span>
          </button>
          <button
            class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-zinc-100 text-zinc-700 dark:hover:bg-zinc-800 dark:text-zinc-200 font-medium transition-colors"
            title="Seçili metnin üstünü çiz"
            on:click={() => applyTextAction("strikethrough")}
          >
            <Strikethrough size={13} />
            <span>Üstünü Çiz</span>
          </button>
          <div class="h-3.5 w-px bg-zinc-200 dark:bg-zinc-700"></div>
          <button
            class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-zinc-100 text-zinc-600 dark:hover:bg-zinc-800 dark:text-zinc-300 font-medium transition-colors"
            title="Panoya kopyala (Ctrl+C)"
            on:click={copySelectedText}
          >
            <Copy size={13} />
            <span>Kopyala</span>
          </button>
        </div>
      {/if}

      <!-- Floating Reading Mode Header Bar -->
      {#if isReadingMode && activeDoc}
        <div class="fixed top-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 px-4 py-2 rounded-2xl bg-zinc-900/90 text-white shadow-2xl backdrop-blur-md border border-white/10 text-xs">
          <button
            class="flex items-center gap-1 rounded-lg px-2 py-1 hover:bg-white/10 disabled:opacity-30"
            disabled={activeDoc.currentPage <= 1}
            on:click={() => setPage(activeDoc.currentPage - 1)}
          >
            <ChevronLeft size={16} />
          </button>
          <span class="font-medium">Sayfa {activeDoc.currentPage} / {activeDoc.pageCount}</span>
          <button
            class="flex items-center gap-1 rounded-lg px-2 py-1 hover:bg-white/10 disabled:opacity-30"
            disabled={activeDoc.currentPage >= activeDoc.pageCount}
            on:click={() => setPage(activeDoc.currentPage + 1)}
          >
            <ChevronRight size={16} />
          </button>
          <div class="h-4 w-px bg-white/20"></div>
          <button class="p-1 hover:bg-white/10 rounded" title="Uzaklaştır" on:click={() => changeZoom(-0.2)}>
            <ZoomOut size={15} />
          </button>
          <span class="font-mono text-[11px]">%{Math.round(activeDoc.scale * 100)}</span>
          <button class="p-1 hover:bg-white/10 rounded" title="Yakınlaştır" on:click={() => changeZoom(0.2)}>
            <ZoomIn size={15} />
          </button>
          <div class="h-4 w-px bg-white/20"></div>
          <button
            class="flex items-center gap-1.5 rounded-lg bg-white/10 hover:bg-white/20 px-2.5 py-1 font-semibold transition-colors"
            on:click={toggleReadingMode}
            title="Okuma modundan çık (Esc)"
          >
            <X size={14} /> Çıkış (Esc)
          </button>
        </div>
      {/if}
    </div>

    {#if !isReadingMode}
      <!-- Bottom Page Navigation Bar -->
      <div class="flex items-center justify-center gap-4 border-t border-zinc-200 bg-white px-4 py-1.5 text-xs text-zinc-600 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-300 shrink-0">
        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
          disabled={activeDoc.currentPage <= 1}
          on:click={() => setPage(activeDoc.currentPage - 1)}
        >
          <ChevronLeft size={16} /> Önceki
        </button>

        <span class="font-medium">Sayfa {activeDoc.currentPage} / {activeDoc.pageCount}</span>

        <button
          class="flex items-center gap-1 rounded-lg px-2 py-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
          disabled={activeDoc.currentPage >= activeDoc.pageCount}
          on:click={() => setPage(activeDoc.currentPage + 1)}
        >
          Sonraki <ChevronRight size={16} />
        </button>
      </div>
    {/if}
  {/if}
</div>

{#if showSignatureModal}
  <SignaturePadModal
    onClose={() => (showSignatureModal = false)}
    onSaveSignature={handleSaveSignature}
  />
{/if}

{#if showExportConvertModal && activeDoc}
  <ExportConvertModal
    doc={activeDoc}
    onClose={() => (showExportConvertModal = false)}
  />
{/if}

{#if showPrintModal && activeDoc}
  <PrintModal
    doc={activeDoc}
    onClose={() => (showPrintModal = false)}
  />
{/if}

{#if showRedactionConfirmModal}
  <ConfirmModal
    title="Kalıcı Sansür Onayı"
    message="Bu PDF'te Sansür uygulanan yerler tekrar düzenlenemez - Sansür kaldırılamaz. Kabul ediyor musunuz?"
    confirmLabel="Kabul Ediyorum ve Kaydet"
    cancelLabel="Vazgeç"
    onConfirm={confirmRedactionAndSave}
    onCancel={() => (showRedactionConfirmModal = false)}
  />
{/if}

{#if pendingCloseDocId}
  <ConfirmModal
    title="Belgeyi Kapatmak İstiyor musunuz?"
    message="Bu belgede kaydedilmemiş düzenlemeleriniz var. Kaydetmeden kapatırsanız değişiklikler kaybolacaktır."
    confirmLabel="Kaydetmeden Kapat"
    cancelLabel="Vazgeç"
    onConfirm={confirmCloseTab}
    onCancel={() => (pendingCloseDocId = null)}
  />
{/if}

{#if showRemoveWatermarkModal && activeDoc}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
    <div class="w-full max-w-sm rounded-2xl bg-white p-6 shadow-2xl dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 animate-in fade-in zoom-in-95 duration-150">
      <div class="flex items-center gap-3 mb-4">
        <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-rose-50 text-rose-600 dark:bg-rose-950/60 dark:text-rose-400">
          <Stamp size={20} />
        </div>
        <div>
          <h3 class="text-base font-bold text-zinc-900 dark:text-zinc-100">Filigranı Kaldır</h3>
          <p class="text-xs text-zinc-500 dark:text-zinc-400">Belgedeki tüm filigranları ve damgaları temizler.</p>
        </div>
      </div>

      <p class="mb-6 text-xs text-zinc-600 dark:text-zinc-300 leading-relaxed">
        Bu işlem belgedeki tüm çapraz filigranları, damgaları ve şeffaf işaretleri otomatik olarak temizler. Devam etmek istiyor musunuz?
      </p>

      <div class="flex items-center justify-end gap-2.5">
        <button
          type="button"
          class="rounded-xl border border-zinc-300 px-4 py-2 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors"
          disabled={isRemovingWatermark}
          on:click={() => (showRemoveWatermarkModal = false)}
        >
          Vazgeç
        </button>
        <button
          type="button"
          class="flex items-center gap-1.5 rounded-xl bg-rose-600 px-4 py-2 text-xs font-bold text-white shadow-md shadow-rose-600/25 hover:bg-rose-500 disabled:opacity-50 transition-all active:scale-95"
          disabled={isRemovingWatermark}
          on:click={handleRemoveWatermarkApplyDirectly}
        >
          <Stamp size={14} />
          {isRemovingWatermark ? "Temizleniyor..." : "Filigranı Kaldır"}
        </button>
      </div>
    </div>
  </div>
{/if}
