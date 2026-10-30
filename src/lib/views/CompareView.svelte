<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import {
    ArrowLeftRight, FileText, Play, Square, SlidersHorizontal,
    ChevronLeft, ChevronRight, ChevronsLeft, ZoomIn, ZoomOut,
    Maximize2, MoveHorizontal, Link2, Unlink2, Layers, Flame,
    Eye, Download, Search, AlertTriangle, Info, CheckCircle2,
    FolderOpen, RefreshCw, Columns2, Sparkles, RotateCcw,
  } from "@lucide/svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import {
    comparePdfs, cancelComparison, exportComparisonReport,
    getPdfInfo, baseName, fmtSize,
    type CompareMode, type CompareOptions, type ComparisonResult,
    type DiffChangeItem, type EnrichedPageData, type PageAlignment,
    type BoundingBox,
  } from "../api";
  import { openPdf, type PdfDocHandle } from "../pdf";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";
  import {
    ScrollSyncGuard,
    computeSyncedScrollTarget,
    filterChanges,
    computeFilteredSummary,
    navigateChangeIndex,
    computeCanvasVisualDiff,
    type FilterCategory,
    type VisualDisplayMode,
  } from "../compare/syncEngine";

  interface LoadedCompareDoc {
    path: string;
    name: string;
    pageCount: number;
    fileSize: number;
    version: string;
    handle: PdfDocHandle;
  }

  let leftDoc: LoadedCompareDoc | null = null;
  let rightDoc: LoadedCompareDoc | null = null;
  let loadingLeft = false;
  let loadingRight = false;
  let leftError: string | null = null;
  let rightError: string | null = null;

  // Comparison state
  let comparing = false;
  let progressPercent = 0;
  let progressStage = "";
  let comparisonResult: ComparisonResult | null = null;
  let allCombinedChanges: DiffChangeItem[] = [];
  let inspectedVisualPairs = new Set<string>();

  // Options & Settings
  let mode: CompareMode = "auto";
  let ignoreCase = false;
  let ignoreWhitespace = true;
  let normalizeHyphenation = true;
  let unicodeNormalization = true;
  let normalizeLigatures = true;
  let visualTolerance = 24;
  let ignoreAntialiasing = true;
  let showSettingsPanel = false;

  // Customizable highlight colors (consistent across both panes & legend)
  let colorDeleted = "#e11d48";   // Rose-600
  let colorAdded = "#059669";     // Emerald-600
  let colorModified = "#d97706";  // Amber-600
  let colorVisual = "#7c3aed";    // Violet-600

  // Viewer state
  let leftPage = 1;
  let rightPage = 1;
  let leftScale = 1.25;
  let rightScale = 1.25;
  let syncScroll = true;
  let syncZoom = true;
  let visualDisplayMode: VisualDisplayMode = "side_by_side";
  let overlayAlpha = 0.5;
  let quickFlipShowRight = false;

  // Filter & Navigation state
  let activeFilter: FilterCategory = "all";
  let searchQuery = "";
  let selectedChangeIndex = -1;

  // DOM refs
  let leftScrollContainer: HTMLDivElement | null = null;
  let rightScrollContainer: HTMLDivElement | null = null;
  let leftCanvas: HTMLCanvasElement | null = null;
  let rightCanvas: HTMLCanvasElement | null = null;
  let leftHighlightCanvas: HTMLCanvasElement | null = null;
  let rightHighlightCanvas: HTMLCanvasElement | null = null;
  let compositeCanvas: HTMLCanvasElement | null = null;

  // Bounded LRU Canvas Cache (max 12 rendered pages to protect memory on large PDFs)
  const MAX_CACHE_ENTRIES = 12;
  const renderCache = new Map<string, HTMLCanvasElement>();

  function getCachedRender(side: "L" | "R", page: number, scale: number): HTMLCanvasElement | undefined {
    const key = `${side}_${page}_${scale.toFixed(2)}`;
    const hit = renderCache.get(key);
    if (hit) {
      renderCache.delete(key);
      renderCache.set(key, hit);
    }
    return hit;
  }

  function setCachedRender(side: "L" | "R", page: number, scale: number, canvas: HTMLCanvasElement) {
    const key = `${side}_${page}_${scale.toFixed(2)}`;
    if (renderCache.has(key)) renderCache.delete(key);
    renderCache.set(key, canvas);
    while (renderCache.size > MAX_CACHE_ENTRIES) {
      const oldestKey = renderCache.keys().next().value;
      if (oldestKey) renderCache.delete(oldestKey);
    }
  }

  const scrollGuard = new ScrollSyncGuard();
  let unlistenProgress: (() => void) | null = null;

  // Handle dropped files from global drag-drop
  $: if ($pendingFiles && $pendingFiles.targetView === "compare") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    pendingFiles.set(null);
    if (pdfs.length >= 2) {
      loadDocument("left", pdfs[0]);
      loadDocument("right", pdfs[1]);
    } else if (pdfs.length === 1) {
      if (!leftDoc) loadDocument("left", pdfs[0]);
      else loadDocument("right", pdfs[0]);
    }
  }

  $: filteredChanges = filterChanges(allCombinedChanges, activeFilter, searchQuery);
  $: filteredSummary = computeFilteredSummary(
    filteredChanges,
    comparisonResult?.alignments ?? [],
  );
  $: selectedChange =
    selectedChangeIndex >= 0 && selectedChangeIndex < filteredChanges.length
      ? filteredChanges[selectedChangeIndex]
      : null;

  $: currentAlignment = (comparisonResult?.alignments ?? []).find(
    (a) => a.left_page === leftPage || a.right_page === rightPage,
  );

  onMount(async () => {
    try {
      unlistenProgress = await listen<{ current: number; total: number; percent: number; stage: string }>(
        "compare-progress",
        (ev) => {
          progressPercent = ev.payload.percent;
          progressStage = ev.payload.stage;
        },
      );
    } catch {}
  });

  onDestroy(() => {
    unlistenProgress?.();
    renderCache.clear();
    leftDoc?.handle.destroy().catch(() => {});
    rightDoc?.handle.destroy().catch(() => {});
  });

  function formatFriendlyError(err: unknown): string {
    const msg = String(err || "");
    if (msg.toLowerCase().includes("password") || msg.toLowerCase().includes("encrypted")) {
      return "Bu PDF parola korumalıdır. Karşılaştırma öncesinde 'Şifrele' aracından şifresini kaldırın.";
    }
    if (msg.toLowerCase().includes("empty")) {
      return "Seçilen PDF dosyasında geçerli sayfa bulunamadı.";
    }
    return `PDF dosyası açılamadı: ${msg}`;
  }

  async function pickFile(side: "left" | "right") {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF Belgeleri", extensions: ["pdf"] }],
    });
    const paths = Array.isArray(selected)
      ? selected
      : typeof selected === "string" && selected
      ? [selected]
      : [];
    if (paths.length === 0) return;

    if (paths.length >= 2) {
      if (paths.length > 2) {
        pushToast(
          "En fazla 2 PDF karşılaştırılabilir. İlk seçilen 2 dosya (1. Eski, 2. Yeni) olarak yüklendi.",
          "info",
        );
      }
      await Promise.all([
        loadDocument("left", paths[0]),
        loadDocument("right", paths[1]),
      ]);
    } else {
      await loadDocument(side, paths[0]);
    }
  }

  async function resetCompareTool() {
    if (comparing) {
      await cancelComparison().catch(() => {});
      comparing = false;
    }
    if (leftDoc) {
      await leftDoc.handle.destroy().catch(() => {});
    }
    if (rightDoc) {
      await rightDoc.handle.destroy().catch(() => {});
    }

    leftDoc = null;
    rightDoc = null;
    loadingLeft = false;
    loadingRight = false;
    leftError = null;
    rightError = null;

    comparisonResult = null;
    allCombinedChanges = [];
    progressPercent = 0;
    progressStage = "";
    inspectedVisualPairs.clear();
    renderCache.clear();
    scrollGuard.reset();

    mode = "auto";
    ignoreCase = false;
    ignoreWhitespace = true;
    normalizeHyphenation = true;
    unicodeNormalization = true;
    normalizeLigatures = true;
    visualTolerance = 24;
    ignoreAntialiasing = true;
    showSettingsPanel = false;

    colorDeleted = "#e11d48";
    colorAdded = "#059669";
    colorModified = "#d97706";
    colorVisual = "#7c3aed";

    leftPage = 1;
    rightPage = 1;
    leftScale = 1.25;
    rightScale = 1.25;
    syncScroll = true;
    syncZoom = true;
    visualDisplayMode = "side_by_side";
    overlayAlpha = 0.5;
    quickFlipShowRight = false;

    activeFilter = "all";
    searchQuery = "";
    selectedChangeIndex = -1;

    for (const c of [leftCanvas, rightCanvas, leftHighlightCanvas, rightHighlightCanvas, compositeCanvas]) {
      if (c) {
        const ctx = c.getContext("2d");
        ctx?.clearRect(0, 0, c.width, c.height);
        c.width = 0;
        c.height = 0;
      }
    }
  }

  async function toggleSyncZoom() {
    syncZoom = !syncZoom;
    if (syncZoom) {
      leftScale = 1.25;
      rightScale = 1.25;
      await Promise.all([renderLeftPage(), renderRightPage()]);
    }
  }

  async function loadDocument(side: "left" | "right", path: string) {
    if (side === "left") {
      loadingLeft = true;
      leftError = null;
    } else {
      loadingRight = true;
      rightError = null;
    }

    try {
      const info = await getPdfInfo(path);
      if (info.encrypted) {
        throw new Error("document is password-protected");
      }
      const handle = await openPdf(path);
      const loaded: LoadedCompareDoc = {
        path,
        name: baseName(path),
        pageCount: handle.pageCount,
        fileSize: info.file_size_bytes,
        version: info.version,
        handle,
      };

      const sidePrefix = side === "left" ? "L_" : "R_";
      for (const k of Array.from(renderCache.keys())) {
        if (k.startsWith(sidePrefix)) renderCache.delete(k);
      }
      comparisonResult = null;
      allCombinedChanges = [];
      selectedChangeIndex = -1;
      inspectedVisualPairs.clear();

      if (side === "left") {
        if (leftDoc) await leftDoc.handle.destroy().catch(() => {});
        leftDoc = loaded;
        leftPage = 1;
        await tick();
        await renderLeftPage();
      } else {
        if (rightDoc) await rightDoc.handle.destroy().catch(() => {});
        rightDoc = loaded;
        rightPage = 1;
        await tick();
        await renderRightPage();
      }
    } catch (e) {
      const friendly = formatFriendlyError(e);
      if (side === "left") leftError = friendly;
      else rightError = friendly;
      pushToast(friendly, "error");
    } finally {
      if (side === "left") loadingLeft = false;
      else loadingRight = false;
    }
  }

  async function swapDocuments() {
    if (!leftDoc && !rightDoc) return;
    const tmpDoc = leftDoc;
    leftDoc = rightDoc;
    rightDoc = tmpDoc;

    const tmpPage = leftPage;
    leftPage = rightPage;
    rightPage = tmpPage;

    const tmpErr = leftError;
    leftError = rightError;
    rightError = tmpErr;

    renderCache.clear();
    await tick();
    await Promise.all([renderLeftPage(), renderRightPage()]);
    if (comparisonResult && leftDoc && rightDoc) {
      await runComparison();
    }
  }

  async function extractEnrichedPages(doc: LoadedCompareDoc): Promise<EnrichedPageData[]> {
    const out: EnrichedPageData[] = [];
    // Extract pdf.js decoded spans for up to 80 pages so custom subset fonts are 100% decoded
    const limit = Math.min(doc.pageCount, 80);
    for (let p = 1; p <= limit; p++) {
      try {
        const items = await doc.handle.getPageTextItems(p);
        out.push({
          page: p,
          spans: items.map((it) => ({
            text: it.str,
            x: it.x,
            y: it.y,
            width: it.width,
            height: it.height,
            font_name: it.fontName ?? "",
            font_size: it.fontSize ?? it.height,
          })),
        });
      } catch {}
    }
    return out;
  }

  async function runComparison() {
    if (!leftDoc || !rightDoc || comparing) return;
    comparing = true;
    progressPercent = 2;
    progressStage = "Metin ve yapı katmanları hazırlanıyor…";
    selectedChangeIndex = -1;
    inspectedVisualPairs.clear();

    try {
      const options: CompareOptions = {
        mode,
        normalization: {
          ignore_case: ignoreCase,
          ignore_whitespace: ignoreWhitespace,
          normalize_hyphenation: normalizeHyphenation,
          unicode_normalization: unicodeNormalization,
          normalize_ligatures: normalizeLigatures,
        },
        visual_tolerance: visualTolerance,
        ignore_antialiasing: ignoreAntialiasing,
        min_page_similarity: 0.28,
      };

      const [leftEnriched, rightEnriched] = await Promise.all([
        extractEnrichedPages(leftDoc),
        extractEnrichedPages(rightDoc),
      ]);

      const res = await comparePdfs(
        leftDoc.path,
        rightDoc.path,
        options,
        leftEnriched,
        rightEnriched,
      );

      comparisonResult = res;
      allCombinedChanges = [...res.changes];

      await tick();
      await Promise.all([renderLeftPage(), renderRightPage()]);
      await runActivePairVisualAnalysis();

      if (allCombinedChanges.length > 0) {
        selectedChangeIndex = 0;
      }

      logActivity(
        [leftDoc.name, rightDoc.name],
        "PDF Karşılaştırıldı",
        `${allCombinedChanges.length} fark tespit edildi`,
      );
      pushToast(
        allCombinedChanges.length === 0
          ? "İki PDF arasında anlamlı bir fark bulunamadı."
          : `Karşılaştırma tamamlandı: ${allCombinedChanges.length} fark bulundu.`,
        "success",
      );
    } catch (e) {
      pushToast(`Karşılaştırma hatası: ${e}`, "error");
    } finally {
      comparing = false;
      progressPercent = 0;
      progressStage = "";
    }
  }

  async function handleCancelComparison() {
    await cancelComparison();
    comparing = false;
    pushToast("Karşılaştırma iptal edildi.", "info");
  }

  async function renderLeftPage() {
    if (!leftDoc || !leftCanvas) return;
    const p = Math.max(1, Math.min(leftDoc.pageCount, leftPage));
    let rendered = getCachedRender("L", p, leftScale);
    if (!rendered) {
      rendered = await leftDoc.handle.renderPage(p, leftScale);
      setCachedRender("L", p, leftScale, rendered);
    }
    leftCanvas.width = rendered.width;
    leftCanvas.height = rendered.height;
    const ctx = leftCanvas.getContext("2d");
    if (ctx) ctx.drawImage(rendered, 0, 0);

    drawHighlightOverlay("left");
    updateCompositeView();
  }

  async function renderRightPage() {
    if (!rightDoc || !rightCanvas) return;
    const p = Math.max(1, Math.min(rightDoc.pageCount, rightPage));
    let rendered = getCachedRender("R", p, rightScale);
    if (!rendered) {
      rendered = await rightDoc.handle.renderPage(p, rightScale);
      setCachedRender("R", p, rightScale, rendered);
    }
    rightCanvas.width = rendered.width;
    rightCanvas.height = rendered.height;
    const ctx = rightCanvas.getContext("2d");
    if (ctx) ctx.drawImage(rendered, 0, 0);

    drawHighlightOverlay("right");
    updateCompositeView();
  }

  async function runActivePairVisualAnalysis() {
    if (!comparisonResult || !leftDoc || !rightDoc || !leftCanvas || !rightCanvas) return;
    if (mode === "text_only") return;

    const pairKey = `${leftPage}_${rightPage}_${visualTolerance}_${ignoreAntialiasing}`;
    const leftDims = await leftDoc.handle.getPageDimensions(leftPage);
    const rightDims = await rightDoc.handle.getPageDimensions(rightPage);

    // Exclude regions already covered by text diffs on this page pair when in Auto mode
    const excludeRects: BoundingBox[] =
      mode === "auto"
        ? allCombinedChanges
            .filter(
              (c) =>
                c.layer === "text" &&
                c.left_page === leftPage &&
                c.right_page === rightPage,
            )
            .flatMap((c) => [c.left_rect, c.right_rect].filter((r): r is BoundingBox => r !== null))
        : [];

    let analysisLeftCanvas = leftCanvas;
    let analysisRightCanvas = rightCanvas;
    if (Math.abs(leftScale - rightScale) > 0.01) {
      const baseScale = 1.25;
      const lp = Math.max(1, Math.min(leftDoc.pageCount, leftPage));
      const rp = Math.max(1, Math.min(rightDoc.pageCount, rightPage));
      let lRendered = getCachedRender("L", lp, baseScale);
      if (!lRendered) {
        lRendered = await leftDoc.handle.renderPage(lp, baseScale);
        setCachedRender("L", lp, baseScale, lRendered);
      }
      let rRendered = getCachedRender("R", rp, baseScale);
      if (!rRendered) {
        rRendered = await rightDoc.handle.renderPage(rp, baseScale);
        setCachedRender("R", rp, baseScale, rRendered);
      }
      analysisLeftCanvas = lRendered;
      analysisRightCanvas = rRendered;
    }

    const visualOut = computeCanvasVisualDiff(
      analysisLeftCanvas,
      analysisRightCanvas,
      [leftDims.width, leftDims.height],
      [rightDims.width, rightDims.height],
      visualTolerance,
      ignoreAntialiasing,
      excludeRects,
    );

    if (!inspectedVisualPairs.has(pairKey) && visualOut.regionsPt.length > 0) {
      inspectedVisualPairs.add(pairKey);
      // Avoid duplicating if structural image/vector changes already cover the exact same box
      const existingVisualRects = allCombinedChanges
        .filter((c) => c.left_page === leftPage && c.right_page === rightPage && c.layer !== "text")
        .flatMap((c) => [c.left_rect, c.right_rect].filter((r): r is BoundingBox => r !== null));

      const newItems: DiffChangeItem[] = [];
      for (const reg of visualOut.regionsPt) {
        const alreadyCovered = existingVisualRects.some((ex) => {
          const ix0 = Math.max(reg.x, ex.x);
          const iy0 = Math.max(reg.y, ex.y);
          const ix1 = Math.min(reg.x + reg.width, ex.x + ex.width);
          const iy1 = Math.min(reg.y + reg.height, ex.y + ex.height);
          if (ix1 <= ix0 || iy1 <= iy0) return false;
          const inter = (ix1 - ix0) * (iy1 - iy0);
          return inter / Math.max(1, Math.min(reg.width * reg.height, ex.width * ex.height)) >= 0.4;
        });
        if (!alreadyCovered) {
          newItems.push({
            id: `vis_px_${leftPage}_${rightPage}_${allCombinedChanges.length + newItems.length + 1}`,
            layer: "visual",
            category: "images_graphics",
            kind: "visual_region_diff",
            left_page: leftPage,
            right_page: rightPage,
            left_rect: reg,
            right_rect: reg,
            title: `Görsel Fark Bölgesi (${Math.round(reg.width)}x${Math.round(reg.height)} pt)`,
            detail: `Sayfa ${leftPage} ↔ ${rightPage} üzerinde (${Math.round(reg.x)}, ${Math.round(reg.y)}) koordinatında piksel düzeyinde görsel fark tespit edildi.`,
            before_text: null,
            after_text: null,
            is_heuristic: false,
            confidence: 0.9,
          });
        }
      }

      if (newItems.length > 0) {
        allCombinedChanges = [...allCombinedChanges, ...newItems];
        if (comparisonResult) {
          comparisonResult.alignments = comparisonResult.alignments.map((a) =>
            a.left_page === leftPage && a.right_page === rightPage && a.status === "identical"
              ? { ...a, status: "modified" }
              : a,
          );
        }
        drawHighlightOverlay("left");
        drawHighlightOverlay("right");
      }
    }

    updateCompositeView(visualOut.heatmapCanvas);
  }

  function getChangeColor(c: DiffChangeItem): string {
    if (c.kind === "text_deleted" || c.kind === "image_deleted" || c.kind === "annotation_deleted" || c.kind === "form_field_deleted" || c.kind === "page_deleted") {
      return colorDeleted;
    }
    if (c.kind === "text_added" || c.kind === "image_added" || c.kind === "annotation_added" || c.kind === "form_field_added" || c.kind === "page_added") {
      return colorAdded;
    }
    if (c.layer === "visual") {
      return colorVisual;
    }
    return colorModified;
  }

  function hexToRgba(hex: string, alpha: number): string {
    const clean = hex.replace("#", "");
    const r = parseInt(clean.slice(0, 2), 16) || 0;
    const g = parseInt(clean.slice(2, 4), 16) || 0;
    const b = parseInt(clean.slice(4, 6), 16) || 0;
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }

  function drawHighlightOverlay(side: "left" | "right") {
    const baseCanvas = side === "left" ? leftCanvas : rightCanvas;
    const hlCanvas = side === "left" ? leftHighlightCanvas : rightHighlightCanvas;
    const scale = side === "left" ? leftScale : rightScale;
    const pageNum = side === "left" ? leftPage : rightPage;

    if (!baseCanvas || !hlCanvas) return;
    hlCanvas.width = baseCanvas.width;
    hlCanvas.height = baseCanvas.height;
    const ctx = hlCanvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, hlCanvas.width, hlCanvas.height);

    for (const c of filteredChanges) {
      const targetPage = side === "left" ? c.left_page : c.right_page;
      const rect = side === "left" ? c.left_rect : c.right_rect;
      if (targetPage !== pageNum || !rect) continue;

      const isSelected = selectedChange?.id === c.id;
      const col = getChangeColor(c);
      const x = rect.x * scale;
      const y = rect.y * scale;
      const w = Math.max(6, rect.width * scale);
      const h = Math.max(6, rect.height * scale);

      ctx.fillStyle = hexToRgba(col, isSelected ? 0.34 : 0.18);
      ctx.fillRect(x - 2, y - 2, w + 4, h + 4);

      ctx.strokeStyle = col;
      ctx.lineWidth = isSelected ? 2.8 : 1.5;
      if (isSelected) {
        ctx.setLineDash([]);
      } else {
        ctx.setLineDash([]);
      }
      ctx.strokeRect(x - 2, y - 2, w + 4, h + 4);
    }
  }

  function updateCompositeView(cachedHeatmap?: HTMLCanvasElement | null) {
    if (!compositeCanvas || !leftCanvas || !rightCanvas) return;
    if (visualDisplayMode === "side_by_side" || visualDisplayMode === "highlight") return;

    const w = Math.max(leftCanvas.width, rightCanvas.width);
    const h = Math.max(leftCanvas.height, rightCanvas.height);
    if (w <= 0 || h <= 0) return;

    compositeCanvas.width = w;
    compositeCanvas.height = h;
    const ctx = compositeCanvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, w, h);

    if (visualDisplayMode === "overlay") {
      ctx.globalAlpha = 1.0 - overlayAlpha;
      ctx.drawImage(leftCanvas, 0, 0);
      ctx.globalAlpha = overlayAlpha;
      ctx.drawImage(rightCanvas, 0, 0);
      ctx.globalAlpha = 1.0;
    } else if (visualDisplayMode === "quick_flip") {
      ctx.drawImage(quickFlipShowRight ? rightCanvas : leftCanvas, 0, 0);
    } else if (visualDisplayMode === "heatmap") {
      ctx.globalAlpha = 0.35;
      ctx.drawImage(rightCanvas, 0, 0);
      ctx.globalAlpha = 1.0;
      const hm =
        cachedHeatmap !== undefined
          ? cachedHeatmap
          : computeCanvasVisualDiff(
              leftCanvas,
              rightCanvas,
              [leftCanvas.width / leftScale, leftCanvas.height / leftScale],
              [rightCanvas.width / rightScale, rightCanvas.height / rightScale],
              visualTolerance,
              ignoreAntialiasing,
            ).heatmapCanvas;
      if (hm) {
        ctx.drawImage(hm, 0, 0);
      }
    }
  }

  function handleCanvasClick(side: "left" | "right", e: MouseEvent) {
    const hlCanvas = side === "left" ? leftHighlightCanvas : rightHighlightCanvas;
    const scale = side === "left" ? leftScale : rightScale;
    const pageNum = side === "left" ? leftPage : rightPage;
    if (!hlCanvas) return;

    const rect = hlCanvas.getBoundingClientRect();
    const ptX = (e.clientX - rect.left) / scale;
    const ptY = (e.clientY - rect.top) / scale;

    const hitIdx = filteredChanges.findIndex((c) => {
      const targetPage = side === "left" ? c.left_page : c.right_page;
      const box = side === "left" ? c.left_rect : c.right_rect;
      if (targetPage !== pageNum || !box) return false;
      return (
        ptX >= box.x - 4 &&
        ptX <= box.x + box.width + 4 &&
        ptY >= box.y - 4 &&
        ptY <= box.y + box.height + 4
      );
    });

    if (hitIdx !== -1) {
      selectChange(hitIdx);
    }
  }

  async function setLeftPageNumber(p: number) {
    if (!leftDoc) return;
    const next = Math.max(1, Math.min(leftDoc.pageCount, p));
    if (next === leftPage) return;
    leftPage = next;
    await renderLeftPage();

    if (syncScroll && comparisonResult) {
      const target = computeSyncedScrollTarget("left", leftPage, 0, 0, true, comparisonResult.alignments);
      if (target && rightDoc && target.targetPage !== rightPage) {
        rightPage = Math.max(1, Math.min(rightDoc.pageCount, target.targetPage));
        await renderRightPage();
      }
    }
    await runActivePairVisualAnalysis();
  }

  async function setRightPageNumber(p: number) {
    if (!rightDoc) return;
    const next = Math.max(1, Math.min(rightDoc.pageCount, p));
    if (next === rightPage) return;
    rightPage = next;
    await renderRightPage();

    if (syncScroll && comparisonResult) {
      const target = computeSyncedScrollTarget("right", rightPage, 0, 0, true, comparisonResult.alignments);
      if (target && leftDoc && target.targetPage !== leftPage) {
        leftPage = Math.max(1, Math.min(leftDoc.pageCount, target.targetPage));
        await renderLeftPage();
      }
    }
    await runActivePairVisualAnalysis();
  }

  function handlePaneScroll(side: "left" | "right") {
    if (!syncScroll) return;
    const srcEl = side === "left" ? leftScrollContainer : rightScrollContainer;
    const dstEl = side === "left" ? rightScrollContainer : leftScrollContainer;
    if (!srcEl || !dstEl) return;

    if (!scrollGuard.canProcessScroll(side, syncScroll)) {
      return;
    }

    const maxScrollY = Math.max(1, srcEl.scrollHeight - srcEl.clientHeight);
    const maxScrollX = Math.max(1, srcEl.scrollWidth - srcEl.clientWidth);
    const relY = srcEl.scrollTop / maxScrollY;
    const relX = srcEl.scrollLeft / maxScrollX;

    if (comparisonResult && comparisonResult.alignments.length > 0) {
      const srcPage = side === "left" ? leftPage : rightPage;
      const syncTarget = computeSyncedScrollTarget(
        side,
        srcPage,
        relY,
        relX,
        syncScroll,
        comparisonResult.alignments,
      );
      if (syncTarget) {
        if (side === "left" && rightDoc && syncTarget.targetPage !== rightPage) {
          rightPage = Math.max(1, Math.min(rightDoc.pageCount, syncTarget.targetPage));
          renderRightPage();
        } else if (side === "right" && leftDoc && syncTarget.targetPage !== leftPage) {
          leftPage = Math.max(1, Math.min(leftDoc.pageCount, syncTarget.targetPage));
          renderLeftPage();
        }
      }
    }

    const dstMaxY = Math.max(0, dstEl.scrollHeight - dstEl.clientHeight);
    const dstMaxX = Math.max(0, dstEl.scrollWidth - dstEl.clientWidth);
    dstEl.scrollTop = relY * dstMaxY;
    dstEl.scrollLeft = relX * dstMaxX;
  }

  async function changePaneZoom(side: "left" | "right", delta: number) {
    if (side === "left" || syncZoom) {
      leftScale = Math.round(Math.max(0.4, Math.min(3.0, leftScale + delta)) * 100) / 100;
    }
    if (side === "right" || syncZoom) {
      rightScale = Math.round(Math.max(0.4, Math.min(3.0, rightScale + delta)) * 100) / 100;
    }
    await Promise.all([renderLeftPage(), renderRightPage()]);
  }

  async function fitPaneWidth(side: "left" | "right") {
    const container = side === "left" ? leftScrollContainer : rightScrollContainer;
    const doc = side === "left" ? leftDoc : rightDoc;
    const page = side === "left" ? leftPage : rightPage;
    if (!container || !doc) return;
    const dims = await doc.handle.getPageDimensions(page);
    const next = Math.round(Math.max(0.4, Math.min(3.0, (container.clientWidth - 32) / dims.width)) * 100) / 100;
    if (side === "left" || syncZoom) leftScale = next;
    if (side === "right" || syncZoom) rightScale = next;
    await Promise.all([renderLeftPage(), renderRightPage()]);
  }

  async function fitPanePage(side: "left" | "right") {
    const container = side === "left" ? leftScrollContainer : rightScrollContainer;
    const doc = side === "left" ? leftDoc : rightDoc;
    const page = side === "left" ? leftPage : rightPage;
    if (!container || !doc) return;
    const dims = await doc.handle.getPageDimensions(page);
    const scaleW = (container.clientWidth - 32) / dims.width;
    const scaleH = (container.clientHeight - 32) / dims.height;
    const next = Math.round(Math.max(0.4, Math.min(3.0, Math.min(scaleW, scaleH))) * 100) / 100;
    if (side === "left" || syncZoom) leftScale = next;
    if (side === "right" || syncZoom) rightScale = next;
    await Promise.all([renderLeftPage(), renderRightPage()]);
  }

  async function selectChange(index: number) {
    if (index < 0 || index >= filteredChanges.length) return;
    selectedChangeIndex = index;
    const item = filteredChanges[index];

    if (item.left_page !== null && leftDoc && item.left_page !== leftPage) {
      leftPage = Math.max(1, Math.min(leftDoc.pageCount, item.left_page));
      await renderLeftPage();
    } else {
      drawHighlightOverlay("left");
    }

    if (item.right_page !== null && rightDoc && item.right_page !== rightPage) {
      rightPage = Math.max(1, Math.min(rightDoc.pageCount, item.right_page));
      await renderRightPage();
    } else {
      drawHighlightOverlay("right");
    }

    // Scroll both panes to center the change bounding box without triggering sync feedback
    scrollGuard.markProgrammaticScroll("left");
    if (item.left_rect && leftScrollContainer) {
      const targetY = item.left_rect.y * leftScale - leftScrollContainer.clientHeight * 0.35;
      leftScrollContainer.scrollTo({ top: Math.max(0, targetY), behavior: "smooth" });
    }
    if (item.right_rect && rightScrollContainer) {
      const targetY = item.right_rect.y * rightScale - rightScrollContainer.clientHeight * 0.35;
      rightScrollContainer.scrollTo({ top: Math.max(0, targetY), behavior: "smooth" });
    }
  }

  async function stepChange(dir: "first" | "prev" | "next") {
    const nextIdx = navigateChangeIndex(selectedChangeIndex, filteredChanges.length, dir);
    if (nextIdx !== -1) {
      await selectChange(nextIdx);
    }
  }

  async function handleExportReport(format: "json" | "csv" | "html") {
    if (!comparisonResult) return;
    const output = await save({
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
      defaultPath: `karsilastirma_raporu.${format}`,
    });
    if (!output) return;
    try {
      const exportPayload: ComparisonResult = {
        ...comparisonResult,
        changes: filteredChanges,
        summary: filteredSummary,
      };
      await exportComparisonReport(exportPayload, output, format);
      pushToast(`Rapor dışa aktarıldı: ${baseName(output)}`, "success", {
        label: "Klasöre Git",
        path: output,
      });
    } catch (e) {
      pushToast(`Rapor dışa aktarma hatası: ${e}`, "error");
    }
  }

  const filterButtons: { id: FilterCategory; label: string }[] = [
    { id: "all", label: "Tüm Farklar" },
    { id: "text", label: "Metin" },
    { id: "formatting", label: "Biçim & Yerleşim" },
    { id: "images_graphics", label: "Görsel & Grafik" },
    { id: "annotations", label: "Anotasyon" },
    { id: "forms", label: "Form Alanları" },
    { id: "header_footer", label: "Üst/Altbilgi" },
    { id: "background", label: "Arka Plan" },
    { id: "pages", label: "Sayfa Yapısı" },
  ];
</script>

<div class="flex h-full flex-col gap-3">
  <ToolHeader
    title="PDF Karşılaştır"
    description="Metin, görsel ve belge yapısı farklarını 3 ayrık katmanda %100 çevrimdışı analiz edin"
  >
    <div class="flex flex-wrap items-center gap-2">
      <!-- Sıfırla Butonu -->
      <button
        type="button"
        class="inline-flex items-center gap-1.5 rounded-lg bg-rose-600 px-3 py-1.5 text-xs font-semibold text-white shadow-sm transition-colors hover:bg-rose-500 active:scale-95 cursor-pointer"
        on:click={resetCompareTool}
        title="Seçilen dosyaları ve karşılaştırma ekranını ilk haline sıfırla"
      >
        <RotateCcw size={13} /> Sıfırla
      </button>

      <!-- Karşılaştırma Modu -->
      <select
        bind:value={mode}
        on:change={() => comparisonResult && runComparison()}
        class="rounded-lg border border-zinc-200 bg-white px-2.5 py-1.5 text-xs font-medium text-zinc-700 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200"
        title="Karşılaştırma Modu"
      >
        <option value="auto">Otomatik (Metin + Görsel + Yapı)</option>
        <option value="text_only">Yalnızca Metin Farkları</option>
        <option value="visual_only">Görsel Karşılaştırma</option>
        <option value="content_structure">İçerik ve Biçim Karşılaştırması</option>
      </select>

      <!-- Normalizasyon & Renk Ayarları -->
      <button
        type="button"
        class="inline-flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs font-medium transition-colors
          {showSettingsPanel
            ? 'border-indigo-500 bg-indigo-50 text-indigo-700 dark:bg-indigo-950/60 dark:text-indigo-300'
            : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-300'}"
        on:click={() => (showSettingsPanel = !showSettingsPanel)}
      >
        <SlidersHorizontal size={14} /> Ayarlar
      </button>

      <!-- Yer Değiştir (Swap) -->
      <button
        type="button"
        disabled={!leftDoc && !rightDoc}
        class="inline-flex items-center gap-1.5 rounded-lg border border-zinc-200 bg-white px-2.5 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-100 disabled:opacity-40 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-300"
        on:click={swapDocuments}
        title="Referans ve Karşılaştırılan PDF'lerin yerini değiştir"
      >
        <ArrowLeftRight size={14} /> Yer Değiştir
      </button>

      {#if comparing}
        <button
          type="button"
          class="inline-flex items-center gap-1.5 rounded-lg bg-rose-600 px-3 py-1.5 text-xs font-semibold text-white shadow-sm hover:bg-rose-500"
          on:click={handleCancelComparison}
        >
          <Square size={13} /> İptal ({progressPercent}%)
        </button>
      {:else}
        <button
          type="button"
          disabled={!leftDoc || !rightDoc}
          class="inline-flex items-center gap-1.5 rounded-lg bg-indigo-600 px-3.5 py-1.5 text-xs font-semibold text-white shadow-sm hover:bg-indigo-500 disabled:opacity-40"
          on:click={runComparison}
        >
          <Play size={13} /> Karşılaştır
        </button>
      {/if}
    </div>
  </ToolHeader>

  <!-- Normalizasyon ve Renk Ayarları Çekmecesi -->
  {#if showSettingsPanel}
    <div class="grid grid-cols-1 gap-4 rounded-xl border border-zinc-200 bg-white p-3.5 text-xs shadow-sm dark:border-zinc-800 dark:bg-zinc-900 md:grid-cols-3">
      <div class="space-y-1.5">
        <p class="font-semibold uppercase tracking-wider text-zinc-400">Metin Normalizasyonu</p>
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={ignoreCase} class="rounded text-indigo-600" />
          <span>Büyük/küçük harf farklarını yoksay (Türkçe İ/i, I/ı uyumlu)</span>
        </label>
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={ignoreWhitespace} class="rounded text-indigo-600" />
          <span>Çoklu boşluk ve satır sonu farklarını yoksay</span>
        </label>
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={normalizeHyphenation} class="rounded text-indigo-600" />
          <span>Satır sonu tire bölünmelerini birleştir</span>
        </label>
      </div>

      <div class="space-y-1.5">
        <p class="font-semibold uppercase tracking-wider text-zinc-400">Unicode, Ligatür & Görsel Eşik</p>
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={unicodeNormalization} class="rounded text-indigo-600" />
          <span>Unicode NFC normalizasyonu (Türkçe karakter korumalı)</span>
        </label>
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={normalizeLigatures} class="rounded text-indigo-600" />
          <span>Ligatürleri (ﬁ, ﬂ, ﬀ) ve OCR boşluklarını normalize et</span>
        </label>
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={ignoreAntialiasing} class="rounded text-indigo-600" />
          <span>Anti-aliasing kenar gürültüsünü filtrele</span>
        </label>
        <div class="flex items-center gap-2 pt-1">
          <span class="text-zinc-500">Piksel Toleransı: {visualTolerance}</span>
          <input type="range" min="5" max="80" bind:value={visualTolerance} class="flex-1" />
        </div>
      </div>

      <div class="space-y-1.5">
        <p class="font-semibold uppercase tracking-wider text-zinc-400">Fark Vurgu Renkleri</p>
        <div class="grid grid-cols-2 gap-2">
          <label class="flex items-center gap-2">
            <input type="color" bind:value={colorDeleted} on:input={() => { drawHighlightOverlay("left"); drawHighlightOverlay("right"); }} class="h-5 w-6 cursor-pointer rounded border-0" />
            <span>Silinen (Sol)</span>
          </label>
          <label class="flex items-center gap-2">
            <input type="color" bind:value={colorAdded} on:input={() => { drawHighlightOverlay("left"); drawHighlightOverlay("right"); }} class="h-5 w-6 cursor-pointer rounded border-0" />
            <span>Eklenen (Sağ)</span>
          </label>
          <label class="flex items-center gap-2">
            <input type="color" bind:value={colorModified} on:input={() => { drawHighlightOverlay("left"); drawHighlightOverlay("right"); }} class="h-5 w-6 cursor-pointer rounded border-0" />
            <span>Değişen / Biçim</span>
          </label>
          <label class="flex items-center gap-2">
            <input type="color" bind:value={colorVisual} on:input={() => { drawHighlightOverlay("left"); drawHighlightOverlay("right"); }} class="h-5 w-6 cursor-pointer rounded border-0" />
            <span>Görsel / Grafik</span>
          </label>
        </div>
      </div>
    </div>
  {/if}

  <!-- İlerleme Çubuğu -->
  {#if comparing}
    <div class="rounded-xl border border-indigo-200 bg-indigo-50/70 px-4 py-2.5 dark:border-indigo-900/60 dark:bg-indigo-950/40">
      <div class="mb-1 flex items-center justify-between text-xs font-medium text-indigo-700 dark:text-indigo-300">
        <span>{progressStage || "Karşılaştırılıyor…"}</span>
        <span>%{progressPercent}</span>
      </div>
      <div class="h-1.5 w-full overflow-hidden rounded-full bg-indigo-200 dark:bg-indigo-900">
        <div class="h-full bg-indigo-600 transition-all duration-150" style="width: {progressPercent}%"></div>
      </div>
    </div>
  {/if}

  <!-- Dosya Seçim Kartları & Senkronizasyon / Görünüm Kontrolleri -->
  <div class="grid grid-cols-1 gap-2.5 lg:grid-cols-12">
    <!-- Sol Dosya Kartı -->
    <div class="flex items-center justify-between gap-2 rounded-xl border border-zinc-200 bg-white px-3 py-2 dark:border-zinc-800 dark:bg-zinc-900 lg:col-span-4">
      <div class="flex min-w-0 items-center gap-2.5">
        <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-rose-50 text-rose-600 dark:bg-rose-950/60 dark:text-rose-400">
          <FileText size={16} />
        </div>
        <div class="min-w-0">
          <p class="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">1. Referans / Eski PDF</p>
          {#if loadingLeft}
            <p class="text-xs text-zinc-500">Yükleniyor…</p>
          {:else if leftDoc}
            <p class="truncate text-xs font-semibold text-zinc-800 dark:text-zinc-100" title={leftDoc.path}>{leftDoc.name}</p>
            <p class="text-[11px] text-zinc-400">{leftDoc.pageCount} sayfa · {fmtSize(leftDoc.fileSize)}</p>
          {:else}
            <p class="text-xs text-zinc-400">Dosya seçilmedi</p>
          {/if}
        </div>
      </div>
      <button
        type="button"
        class="shrink-0 inline-flex items-center gap-1 rounded-lg border border-zinc-200 px-2.5 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
        on:click={() => pickFile("left")}
      >
        <FolderOpen size={13} /> {leftDoc ? "Değiştir" : "PDF Seç"}
      </button>
    </div>

    <!-- Orta Senkronizasyon ve Görsel İnceleme Modu Araç Çubuğu -->
    <div class="flex flex-wrap items-center justify-center gap-1.5 rounded-xl border border-zinc-200 bg-white px-3 py-2 dark:border-zinc-800 dark:bg-zinc-900 lg:col-span-4">
      <button
        type="button"
        class="inline-flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-colors
          {syncScroll
            ? 'bg-indigo-600 text-white'
            : 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300'}"
        on:click={() => {
          syncScroll = !syncScroll;
          scrollGuard.reset();
        }}
        title="Senkronize Kaydırma: Açık olduğunda iki bölme eşleşen sayfaları birlikte takip eder"
      >
        {#if syncScroll}<Link2 size={13} />{:else}<Unlink2 size={13} />{/if}
        <span>{syncScroll ? "Senkron Kaydırma" : "Bağımsız Kaydırma"}</span>
      </button>

      <button
        type="button"
        class="inline-flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-colors
          {syncZoom
            ? 'bg-indigo-100 text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300'
            : 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400'}"
        on:click={toggleSyncZoom}
        title="Yakınlaştırmayı Senkronize Et (Açıldığında yakınlaştırmayı %125'te eşitler)"
      >
        <span>Senkron Zoom</span>
      </button>

      <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-700"></div>

      <!-- Görsel İnceleme Gösterim Yöntemi -->
      <div class="inline-flex items-center rounded-lg bg-zinc-100 p-0.5 text-[11px] dark:bg-zinc-800">
        <button
          type="button"
          class="rounded-md px-2 py-1 font-medium {visualDisplayMode === 'side_by_side' ? 'bg-white text-zinc-900 shadow-sm dark:bg-zinc-700 dark:text-white' : 'text-zinc-500'}"
          on:click={() => (visualDisplayMode = "side_by_side")}
          title="Yan Yana Görüntüleme ve Fark Vurgulama"
        >
          <Columns2 size={12} class="inline mr-0.5" /> Yan Yana
        </button>
        <button
          type="button"
          class="rounded-md px-2 py-1 font-medium {visualDisplayMode === 'overlay' ? 'bg-white text-zinc-900 shadow-sm dark:bg-zinc-700 dark:text-white' : 'text-zinc-500'}"
          on:click={() => {
            visualDisplayMode = "overlay";
            tick().then(() => updateCompositeView());
          }}
          title="Saydamlık Ayarlı Üst Üste Bindirme (Overlay)"
        >
          <Layers size={12} class="inline mr-0.5" /> Overlay
        </button>
        <button
          type="button"
          class="rounded-md px-2 py-1 font-medium {visualDisplayMode === 'heatmap' ? 'bg-white text-zinc-900 shadow-sm dark:bg-zinc-700 dark:text-white' : 'text-zinc-500'}"
          on:click={() => {
            visualDisplayMode = "heatmap";
            tick().then(() => updateCompositeView());
          }}
          title="Fark / Isı Haritası (Difference Map)"
        >
          <Flame size={12} class="inline mr-0.5" /> Isı Haritası
        </button>
        <button
          type="button"
          class="rounded-md px-2 py-1 font-medium {visualDisplayMode === 'quick_flip' ? 'bg-white text-zinc-900 shadow-sm dark:bg-zinc-700 dark:text-white' : 'text-zinc-500'}"
          on:click={() => {
            visualDisplayMode = "quick_flip";
            tick().then(() => updateCompositeView());
          }}
          title="İki Görünüm Arasında Hızlı Geçiş"
        >
          <Eye size={12} class="inline mr-0.5" /> Geçiş
        </button>
      </div>
    </div>

    <!-- Sağ Dosya Kartı -->
    <div class="flex items-center justify-between gap-2 rounded-xl border border-zinc-200 bg-white px-3 py-2 dark:border-zinc-800 dark:bg-zinc-900 lg:col-span-4">
      <div class="flex min-w-0 items-center gap-2.5">
        <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-emerald-50 text-emerald-600 dark:bg-emerald-950/60 dark:text-emerald-400">
          <FileText size={16} />
        </div>
        <div class="min-w-0">
          <p class="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">2. Karşılaştırılan / Yeni PDF</p>
          {#if loadingRight}
            <p class="text-xs text-zinc-500">Yükleniyor…</p>
          {:else if rightDoc}
            <p class="truncate text-xs font-semibold text-zinc-800 dark:text-zinc-100" title={rightDoc.path}>{rightDoc.name}</p>
            <p class="text-[11px] text-zinc-400">{rightDoc.pageCount} sayfa · {fmtSize(rightDoc.fileSize)}</p>
          {:else}
            <p class="text-xs text-zinc-400">Dosya seçilmedi</p>
          {/if}
        </div>
      </div>
      <button
        type="button"
        class="shrink-0 inline-flex items-center gap-1 rounded-lg border border-zinc-200 px-2.5 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
        on:click={() => pickFile("right")}
      >
        <FolderOpen size={13} /> {rightDoc ? "Değiştir" : "PDF Seç"}
      </button>
    </div>
  </div>

  {#if leftError || rightError}
    <div class="flex items-center gap-2 rounded-xl border border-rose-300 bg-rose-50 px-3.5 py-2 text-xs text-rose-700 dark:border-rose-900 dark:bg-rose-950/50 dark:text-rose-300">
      <AlertTriangle size={15} class="shrink-0" />
      <span>{leftError || rightError}</span>
    </div>
  {/if}

  <!-- Ana Çalışma Alanı: Sol/Sağ PDF Bölmeleri + Sağ Sonuç/Filtre Paneli -->
  <div class="flex min-h-0 flex-1 gap-3 overflow-hidden">
    <!-- Görüntüleme Alanı -->
    <div class="flex min-w-0 flex-1 flex-col gap-2 overflow-hidden">
      {#if visualDisplayMode === "overlay" || visualDisplayMode === "heatmap" || visualDisplayMode === "quick_flip"}
        <!-- Birleşik Görsel Analiz Tuvali (Overlay / Heatmap / Hızlı Geçiş) -->
        <div class="flex items-center justify-between rounded-xl border border-zinc-200 bg-white px-3 py-1.5 text-xs dark:border-zinc-800 dark:bg-zinc-900">
          <div class="flex items-center gap-3">
            <span class="font-semibold text-indigo-600 dark:text-indigo-400">
              {visualDisplayMode === "overlay"
                ? "Saydamlık Ayarlı Üst Üste Bindirme"
                : visualDisplayMode === "heatmap"
                ? "Piksel Fark / Isı Haritası"
                : "Hızlı Geçiş Modu"}
            </span>
            <span class="text-zinc-500">Sol Sayfa #{leftPage} ↔ Sağ Sayfa #{rightPage}</span>
          </div>

          {#if visualDisplayMode === "overlay"}
            <div class="flex items-center gap-2">
              <span class="text-zinc-500">Referans (%0)</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                bind:value={overlayAlpha}
                on:input={() => updateCompositeView()}
                class="w-36"
              />
              <span class="text-zinc-500">Yeni (%100)</span>
            </div>
          {:else if visualDisplayMode === "quick_flip"}
            <button
              type="button"
              class="inline-flex items-center gap-1.5 rounded-lg bg-indigo-600 px-3 py-1 text-xs font-semibold text-white"
              on:click={() => {
                quickFlipShowRight = !quickFlipShowRight;
                updateCompositeView();
              }}
            >
              <RefreshCw size={12} />
              Şu An Gösterilen: {quickFlipShowRight ? `Yeni PDF (Sayfa ${rightPage})` : `Referans PDF (Sayfa ${leftPage})`} — Geçiş Yap
            </button>
          {/if}
        </div>

        <div class="flex flex-1 items-start justify-center overflow-auto rounded-xl border border-zinc-200 bg-zinc-200/60 p-4 dark:border-zinc-800 dark:bg-zinc-950">
          <canvas bind:this={compositeCanvas} class="rounded bg-white shadow-lg"></canvas>
        </div>
      {/if}

      <!-- Yan Yana İki Bağımsız PDF Bölmesi (Overlay modunda gizli kalıp arka planda render üretir) -->
      <div
        class="grid min-h-0 flex-1 grid-cols-1 gap-3 md:grid-cols-2
          {visualDisplayMode !== 'side_by_side' && visualDisplayMode !== 'highlight' ? 'hidden' : ''}"
      >
        <!-- SOL BÖLME (Referans PDF) -->
        <div class="flex min-h-0 flex-col overflow-hidden rounded-xl border border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900">
          <!-- Sol Bölme Kontrolleri -->
          <div class="flex flex-wrap items-center justify-between gap-1 border-b border-zinc-200 px-2.5 py-1.5 text-xs dark:border-zinc-800">
            <div class="flex items-center gap-1">
              <button
                type="button"
                disabled={!leftDoc || leftPage <= 1}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => setLeftPageNumber(leftPage - 1)}
                title="Önceki Sayfa"
              >
                <ChevronLeft size={14} />
              </button>
              <input
                type="number"
                min="1"
                max={leftDoc?.pageCount ?? 1}
                value={leftPage}
                disabled={!leftDoc}
                on:change={(e) => setLeftPageNumber(parseInt(e.currentTarget.value, 10) || 1)}
                class="w-12 rounded border border-zinc-200 bg-zinc-50 px-1.5 py-0.5 text-center text-xs dark:border-zinc-700 dark:bg-zinc-800"
              />
              <span class="text-zinc-400">/ {leftDoc?.pageCount ?? 0}</span>
              <button
                type="button"
                disabled={!leftDoc || leftPage >= leftDoc.pageCount}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => setLeftPageNumber(leftPage + 1)}
                title="Sonraki Sayfa"
              >
                <ChevronRight size={14} />
              </button>
            </div>

            {#if currentAlignment && currentAlignment.left_page === leftPage}
              <span class="rounded-full bg-zinc-100 px-2 py-0.5 text-[10px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                {#if currentAlignment.status === "deleted"}
                  Silinen Sayfa
                {:else if currentAlignment.status === "moved"}
                  Taşındı → Sağ #{currentAlignment.right_page}
                {:else if currentAlignment.right_page}
                  Eşleşen: Sağ #{currentAlignment.right_page} (%{Math.round(currentAlignment.similarity * 100)})
                {/if}
              </span>
            {/if}

            <div class="flex items-center gap-0.5">
              <button
                type="button"
                disabled={!leftDoc}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => changePaneZoom("left", -0.2)}
                title="Uzaklaştır"
              >
                <ZoomOut size={14} />
              </button>
              <span class="w-11 text-center font-mono text-[11px]">%{Math.round(leftScale * 100)}</span>
              <button
                type="button"
                disabled={!leftDoc}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => changePaneZoom("left", 0.2)}
                title="Yakınlaştır"
              >
                <ZoomIn size={14} />
              </button>
              <button
                type="button"
                disabled={!leftDoc}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => fitPaneWidth("left")}
                title="Genişliğe Sığdır"
              >
                <MoveHorizontal size={13} />
              </button>
              <button
                type="button"
                disabled={!leftDoc}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => fitPanePage("left")}
                title="Sayfaya Sığdır"
              >
                <Maximize2 size={13} />
              </button>
            </div>
          </div>

          <!-- Sol Tuval Kaydırma Alanı -->
          <div
            bind:this={leftScrollContainer}
            on:scroll={() => handlePaneScroll("left")}
            class="flex flex-1 items-start justify-center overflow-auto bg-zinc-100/80 p-3 dark:bg-zinc-950/60"
          >
            {#if leftDoc}
              <div class="relative inline-block shadow-md">
                <canvas bind:this={leftCanvas} class="block bg-white"></canvas>
                <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
                <canvas
                  bind:this={leftHighlightCanvas}
                  on:click={(e) => handleCanvasClick("left", e)}
                  class=" absolute inset-0 cursor-pointer"
                ></canvas>
              </div>
            {:else}
              <button
                type="button"
                on:click={() => pickFile("left")}
                class="flex h-full w-full cursor-pointer flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-zinc-300/80 bg-white/40 p-6 text-center text-xs text-zinc-400 transition-all hover:border-indigo-400 hover:bg-indigo-50/40 hover:text-indigo-600 dark:border-zinc-800 dark:bg-zinc-900/30 dark:hover:border-indigo-500/60 dark:hover:bg-indigo-950/20 dark:hover:text-indigo-300"
                title="Referans (Eski) PDF dosyasını seçmek için tıklayın"
              >
                <FileText size={32} class="opacity-50" />
                <p class="font-medium">Referans (Eski) PDF dosyasını seçin veya buraya sürükleyin</p>
              </button>
            {/if}
          </div>
        </div>

        <!-- SAĞ BÖLME (Karşılaştırılan PDF) -->
        <div class="flex min-h-0 flex-col overflow-hidden rounded-xl border border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900">
          <!-- Sağ Bölme Kontrolleri -->
          <div class="flex flex-wrap items-center justify-between gap-1 border-b border-zinc-200 px-2.5 py-1.5 text-xs dark:border-zinc-800">
            <div class="flex items-center gap-1">
              <button
                type="button"
                disabled={!rightDoc || rightPage <= 1}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => setRightPageNumber(rightPage - 1)}
                title="Önceki Sayfa"
              >
                <ChevronLeft size={14} />
              </button>
              <input
                type="number"
                min="1"
                max={rightDoc?.pageCount ?? 1}
                value={rightPage}
                disabled={!rightDoc}
                on:change={(e) => setRightPageNumber(parseInt(e.currentTarget.value, 10) || 1)}
                class="w-12 rounded border border-zinc-200 bg-zinc-50 px-1.5 py-0.5 text-center text-xs dark:border-zinc-700 dark:bg-zinc-800"
              />
              <span class="text-zinc-400">/ {rightDoc?.pageCount ?? 0}</span>
              <button
                type="button"
                disabled={!rightDoc || rightPage >= rightDoc.pageCount}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => setRightPageNumber(rightPage + 1)}
                title="Sonraki Sayfa"
              >
                <ChevronRight size={14} />
              </button>
            </div>

            {#if currentAlignment && currentAlignment.right_page === rightPage}
              <span class="rounded-full bg-zinc-100 px-2 py-0.5 text-[10px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                {#if currentAlignment.status === "added"}
                  Yeni Eklenen Sayfa
                {:else if currentAlignment.status === "moved"}
                  Taşındı ← Sol #{currentAlignment.left_page}
                {:else if currentAlignment.left_page}
                  Eşleşen: Sol #{currentAlignment.left_page} (%{Math.round(currentAlignment.similarity * 100)})
                {/if}
              </span>
            {/if}

            <div class="flex items-center gap-0.5">
              <button
                type="button"
                disabled={!rightDoc}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => changePaneZoom("right", -0.2)}
                title="Uzaklaştır"
              >
                <ZoomOut size={14} />
              </button>
              <span class="w-11 text-center font-mono text-[11px]">%{Math.round(rightScale * 100)}</span>
              <button
                type="button"
                disabled={!rightDoc}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => changePaneZoom("right", 0.2)}
                title="Yakınlaştır"
              >
                <ZoomIn size={14} />
              </button>
              <button
                type="button"
                disabled={!rightDoc}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => fitPaneWidth("right")}
                title="Genişliğe Sığdır"
              >
                <MoveHorizontal size={13} />
              </button>
              <button
                type="button"
                disabled={!rightDoc}
                class="rounded p-1 hover:bg-zinc-100 disabled:opacity-30 dark:hover:bg-zinc-800"
                on:click={() => fitPanePage("right")}
                title="Sayfaya Sığdır"
              >
                <Maximize2 size={13} />
              </button>
            </div>
          </div>

          <!-- Sağ Tuval Kaydırma Alanı -->
          <div
            bind:this={rightScrollContainer}
            on:scroll={() => handlePaneScroll("right")}
            class="flex flex-1 items-start justify-center overflow-auto bg-zinc-100/80 p-3 dark:bg-zinc-950/60"
          >
            {#if rightDoc}
              <div class="relative inline-block shadow-md">
                <canvas bind:this={rightCanvas} class="block bg-white"></canvas>
                <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
                <canvas
                  bind:this={rightHighlightCanvas}
                  on:click={(e) => handleCanvasClick("right", e)}
                  class="absolute inset-0 cursor-pointer"
                ></canvas>
              </div>
            {:else}
              <button
                type="button"
                on:click={() => pickFile("right")}
                class="flex h-full w-full cursor-pointer flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-zinc-300/80 bg-white/40 p-6 text-center text-xs text-zinc-400 transition-all hover:border-indigo-400 hover:bg-indigo-50/40 hover:text-indigo-600 dark:border-zinc-800 dark:bg-zinc-900/30 dark:hover:border-indigo-500/60 dark:hover:bg-indigo-950/20 dark:hover:text-indigo-300"
                title="Karşılaştırılacak (Yeni) PDF dosyasını seçmek için tıklayın"
              >
                <FileText size={32} class="opacity-50" />
                <p class="font-medium">Karşılaştırılacak (Yeni) PDF dosyasını seçin veya buraya sürükleyin</p>
              </button>
            {/if}
          </div>
        </div>
      </div>
    </div>

    <!-- SAĞ SONUÇ ÖZETİ, FİLTRE VE DEĞİŞİKLİK LİSTESİ PANELİ -->
    <aside class="flex w-80 shrink-0 flex-col overflow-hidden rounded-xl border border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900 xl:w-96">
      <div class="border-b border-zinc-200 p-3 dark:border-zinc-800">
        <div class="mb-2 flex items-center justify-between">
          <h3 class="text-xs font-bold uppercase tracking-wider text-zinc-700 dark:text-zinc-200">
            Sonuç Özeti ({filteredSummary.total_changes})
          </h3>
          {#if comparisonResult}
            <div class="flex items-center gap-1">
              <button
                type="button"
                class="rounded border border-zinc-200 px-1.5 py-0.5 text-[10px] font-semibold hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
                on:click={() => handleExportReport("json")}
                title="JSON Raporu İndir"
              >
                JSON
              </button>
              <button
                type="button"
                class="rounded border border-zinc-200 px-1.5 py-0.5 text-[10px] font-semibold hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
                on:click={() => handleExportReport("csv")}
                title="CSV Raporu İndir"
              >
                CSV
              </button>
              <button
                type="button"
                class="rounded border border-zinc-200 px-1.5 py-0.5 text-[10px] font-semibold hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
                on:click={() => handleExportReport("html")}
                title="HTML Raporu İndir"
              >
                HTML
              </button>
            </div>
          {/if}
        </div>

        <!-- 3 Ayrık Katman Özet Kutuları -->
        <div class="grid grid-cols-3 gap-1.5 text-center text-[11px]">
          <div class="rounded-lg bg-zinc-50 p-1.5 dark:bg-zinc-800/70">
            <div class="text-sm font-bold text-indigo-600 dark:text-indigo-400">
              {filteredSummary.text_added_count + filteredSummary.text_deleted_count + filteredSummary.text_modified_count + filteredSummary.text_moved_count}
            </div>
            <div class="text-[10px] text-zinc-500">Metin Farkı</div>
            <div class="mt-0.5 text-[9px] text-zinc-400">
              +{filteredSummary.text_added_count} / -{filteredSummary.text_deleted_count} / ~{filteredSummary.text_modified_count}
            </div>
          </div>

          <div class="rounded-lg bg-zinc-50 p-1.5 dark:bg-zinc-800/70">
            <div class="text-sm font-bold text-violet-600 dark:text-violet-400">
              {filteredSummary.formatting_changes_count + filteredSummary.visual_changes_count + filteredSummary.image_graphic_changes_count + filteredSummary.background_changes_count}
            </div>
            <div class="text-[10px] text-zinc-500">Görsel & Biçim</div>
            <div class="mt-0.5 text-[9px] text-zinc-400">
              Biçim:{filteredSummary.formatting_changes_count} Görsel:{filteredSummary.image_graphic_changes_count + filteredSummary.visual_changes_count}
            </div>
          </div>

          <div class="rounded-lg bg-zinc-50 p-1.5 dark:bg-zinc-800/70">
            <div class="text-sm font-bold text-amber-600 dark:text-amber-400">
              {filteredSummary.pages_added_count + filteredSummary.pages_deleted_count + filteredSummary.pages_moved_count + filteredSummary.form_changes_count + filteredSummary.annotation_changes_count + filteredSummary.header_footer_changes_count}
            </div>
            <div class="text-[10px] text-zinc-500">Belge Yapısı</div>
            <div class="mt-0.5 text-[9px] text-zinc-400">
              Sayfa:{filteredSummary.pages_added_count + filteredSummary.pages_deleted_count + filteredSummary.pages_moved_count} Form/Not:{filteredSummary.form_changes_count + filteredSummary.annotation_changes_count}
            </div>
          </div>
        </div>

        {#if filteredSummary.changed_left_pages.length > 0 || filteredSummary.changed_right_pages.length > 0}
          <div class="mt-2 text-[10px] text-zinc-500 dark:text-zinc-400">
            <span>Değişiklik olan sayfalar:</span>
            {#if filteredSummary.changed_left_pages.length > 0}
              <span class="ml-1 font-medium">Sol [{filteredSummary.changed_left_pages.join(", ")}]</span>
            {/if}
            {#if filteredSummary.changed_right_pages.length > 0}
              <span class="ml-1 font-medium">Sağ [{filteredSummary.changed_right_pages.join(", ")}]</span>
            {/if}
          </div>
        {/if}

        <!-- Kategori Filtreleri -->
        <div class="mt-2.5 flex flex-wrap gap-1">
          {#each filterButtons as fb}
            <button
              type="button"
              class="rounded-md px-2 py-0.5 text-[10px] font-medium transition-colors
                {activeFilter === fb.id
                  ? 'bg-indigo-600 text-white'
                  : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-300'}"
              on:click={() => {
                activeFilter = fb.id;
                selectedChangeIndex = filteredChanges.length > 0 ? 0 : -1;
                drawHighlightOverlay("left");
                drawHighlightOverlay("right");
              }}
            >
              {fb.label}
            </button>
          {/each}
        </div>

        <!-- Arama ve Önceki/Sonraki Navigasyon -->
        <div class="mt-2 flex items-center gap-1.5">
          <div class="relative flex-1">
            <Search size={12} class="absolute left-2 top-1/2 -translate-y-1/2 text-zinc-400" />
            <input
              type="text"
              placeholder="Değişikliklerde ara…"
              bind:value={searchQuery}
              on:input={() => {
                selectedChangeIndex = filteredChanges.length > 0 ? 0 : -1;
                drawHighlightOverlay("left");
                drawHighlightOverlay("right");
              }}
              class="w-full rounded-lg border border-zinc-200 bg-zinc-50 py-1 pl-6 pr-2 text-xs dark:border-zinc-700 dark:bg-zinc-800"
            />
          </div>

          <button
            type="button"
            disabled={filteredChanges.length === 0}
            class="rounded-lg border border-zinc-200 p-1 hover:bg-zinc-100 disabled:opacity-30 dark:border-zinc-700 dark:hover:bg-zinc-800"
            on:click={() => stepChange("first")}
            title="İlk Değişikliğe Git"
          >
            <ChevronsLeft size={14} />
          </button>
          <button
            type="button"
            disabled={filteredChanges.length === 0}
            class="rounded-lg border border-zinc-200 p-1 hover:bg-zinc-100 disabled:opacity-30 dark:border-zinc-700 dark:hover:bg-zinc-800"
            on:click={() => stepChange("prev")}
            title="Önceki Değişiklik"
          >
            <ChevronLeft size={14} />
          </button>
          <span class="min-w-[38px] text-center font-mono text-[10px] text-zinc-500">
            {filteredChanges.length > 0 ? `${selectedChangeIndex + 1}/${filteredChanges.length}` : "0/0"}
          </span>
          <button
            type="button"
            disabled={filteredChanges.length === 0}
            class="rounded-lg border border-zinc-200 p-1 hover:bg-zinc-100 disabled:opacity-30 dark:border-zinc-700 dark:hover:bg-zinc-800"
            on:click={() => stepChange("next")}
            title="Sonraki Değişiklik"
          >
            <ChevronRight size={14} />
          </button>
        </div>
      </div>

      <!-- Eşleştirme Belirsizlikleri ve Sınırlama Bildirimleri -->
      {#if comparisonResult && (comparisonResult.uncertainties.length > 0 || comparisonResult.limitations.length > 0)}
        <div class="max-h-28 space-y-1 overflow-y-auto border-b border-zinc-200 bg-amber-50/60 px-3 py-2 text-[11px] dark:border-zinc-800 dark:bg-amber-950/20">
          {#each comparisonResult.uncertainties as unc}
            <div class="flex items-start gap-1.5 text-amber-800 dark:text-amber-300">
              <AlertTriangle size={12} class="mt-0.5 shrink-0" />
              <span>{unc}</span>
            </div>
          {/each}
          {#each comparisonResult.limitations as lim}
            <div class="flex items-start gap-1.5 text-zinc-600 dark:text-zinc-400">
              <Info size={12} class="mt-0.5 shrink-0" />
              <span>{lim}</span>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Değişiklik Listesi -->
      <div class="flex-1 overflow-y-auto p-2">
        {#if !comparisonResult}
          <div class="flex h-full flex-col items-center justify-center gap-2 p-4 text-center text-xs text-zinc-400">
            <Sparkles size={24} class="opacity-40" />
            <p>İki PDF seçip <strong>Karşılaştır</strong> butonuna bastığınızda tüm metin, biçim, görsel ve yapısal farklar burada listelenir.</p>
          </div>
        {:else if filteredChanges.length === 0}
          <div class="flex h-full flex-col items-center justify-center gap-2 p-4 text-center text-xs text-emerald-600 dark:text-emerald-400">
            <CheckCircle2 size={28} />
            <p class="font-semibold">Bu filtrede fark bulunmadı</p>
            <p class="text-zinc-400">Seçili kriterlere göre belgeler birbiriyle uyumlu görünüyor.</p>
          </div>
        {:else}
          <ul class="space-y-1.5">
            {#each filteredChanges as item, idx (item.id)}
              <li>
                <button
                  type="button"
                  on:click={() => selectChange(idx)}
                  class="w-full rounded-xl border p-2.5 text-left transition-all
                    {selectedChangeIndex === idx
                      ? 'border-indigo-500 bg-indigo-50/70 ring-1 ring-indigo-500 dark:border-indigo-500 dark:bg-indigo-950/40'
                      : 'border-zinc-200 bg-white hover:border-zinc-300 dark:border-zinc-800 dark:bg-zinc-900/60 dark:hover:border-zinc-700'}"
                >
                  <div class="mb-1 flex items-center justify-between gap-1">
                    <span
                      class="inline-block rounded-full px-2 py-0.5 text-[10px] font-semibold text-white"
                      style="background-color: {getChangeColor(item)}"
                    >
                      {item.layer === "text" ? "Metin" : item.layer === "visual" ? "Görsel" : "Yapısal"}
                    </span>
                    <span class="font-mono text-[10px] text-zinc-400">
                      {item.left_page ? `Sol S.${item.left_page}` : ""}
                      {item.left_page && item.right_page ? " ↔ " : ""}
                      {item.right_page ? `Sağ S.${item.right_page}` : ""}
                    </span>
                  </div>
                  <p class="text-xs font-semibold text-zinc-800 dark:text-zinc-100">{item.title}</p>
                  <p class="mt-0.5 text-[11px] text-zinc-500 dark:text-zinc-400">{item.detail}</p>
                  {#if item.before_text || item.after_text}
                    <div class="mt-1.5 space-y-0.5 rounded-lg bg-zinc-100/80 p-1.5 font-mono text-[10px] dark:bg-zinc-800/80">
                      {#if item.before_text}
                        <div class="truncate text-rose-600 dark:text-rose-400" title={item.before_text}>− {item.before_text}</div>
                      {/if}
                      {#if item.after_text}
                        <div class="truncate text-emerald-600 dark:text-emerald-400" title={item.after_text}>+ {item.after_text}</div>
                      {/if}
                    </div>
                  {/if}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </aside>
  </div>
</div>
