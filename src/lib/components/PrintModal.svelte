<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    X, ChevronLeft, ChevronRight, HelpCircle, Info, Printer,
    Layers, Check, LayoutGrid, FileText, Settings
  } from "@lucide/svelte";
  import type { OpenDocument } from "../docStore";
  import {
    baseName,
    getSystemPrinters,
    openPrinterProperties,
    directPrintPages,
    type PrinterInfo,
  } from "../api";
  import { pushToast } from "../stores";

  export let doc: OpenDocument;
  export let onClose: () => void;

  // Printer selection
  let printerList: PrinterInfo[] = [];
  let printerName = "Varsayılan Sistem Yazıcısı";
  let copies = 1;
  let collate = true; // Harmanla
  let isGrayscale = false;
  let saveInk = false;

  // Pages to print
  let pageSelection: "all" | "current" | "range" = "all";
  let pageRange = `1-${doc.pageCount}`;
  let previewTargetIndex = 0;

  // Sizing & tabs
  let activeTab: "size" | "poster" | "multiple" | "booklet" = "size";
  let sizingMode: "fit" | "actual" | "shrink" | "custom" = "fit";
  let customScale = 100;

  // Multiple pages per sheet (N-Up)
  let pagesPerSheet: 2 | 4 | 6 = 2;
  let pageOrder: "horizontal" | "vertical" = "horizontal";
  let printPageBorders = true;

  // Orientation
  let orientation: "auto" | "portrait" | "landscape" = "auto";
  let currentDetectedLandscape = false;
  let commentsMode: "doc_and_marks" | "doc_only" | "marks_only" = "doc_and_marks";

  // Paper & Margins (Standard A4 Defaults)
  let paperSize: "A4" | "A3" | "Letter" | "Legal" = "A4";
  let marginLeftMm = 15;
  let marginRightMm = 15;
  let marginTopMm = 15;
  let marginBottomMm = 15;

  const paperDimensions: Record<string, { label: string; widthMm: number; heightMm: number; css: string }> = {
    A4: { label: "A4 (210 x 297 mm)", widthMm: 210, heightMm: 297, css: "A4" },
    A3: { label: "A3 (297 x 420 mm)", widthMm: 297, heightMm: 420, css: "A3" },
    Letter: { label: "Letter (215,9 x 279,4 mm)", widthMm: 215.9, heightMm: 279.4, css: "letter" },
    Legal: { label: "Legal (215,9 x 355,6 mm)", widthMm: 215.9, heightMm: 355.6, css: "legal" },
  };

  // Preview state
  let previewCanvas: HTMLCanvasElement;
  let isRenderingPreview = false;
  let isPrinting = false;

  // Calculated info
  $: targetPages = parsePageRange(pageSelection, pageRange, doc);
  $: if (previewTargetIndex >= targetPages.length) {
    previewTargetIndex = Math.max(0, targetPages.length - 1);
  }
  $: currentPreviewPageNum = targetPages[previewTargetIndex] || 1;
  $: totalSheets = calculateTotalSheets(targetPages.length, activeTab, pagesPerSheet);

  function calculateTotalSheets(pageCount: number, tab: string, nup: number): number {
    if (tab === "multiple") {
      return Math.ceil(pageCount / nup);
    }
    return pageCount;
  }

  function parsePageRange(sel: string, rng: string, currentDoc: OpenDocument): number[] {
    if (sel === "current") return [currentDoc.currentPage];
    if (sel === "all") {
      return Array.from({ length: currentDoc.pageCount }, (_, i) => i + 1);
    }
    const pages: Set<number> = new Set();
    const parts = rng.split(",");
    for (const part of parts) {
      const trimmed = part.trim();
      if (trimmed.includes("-")) {
        const [start, end] = trimmed.split("-").map(Number);
        if (!isNaN(start) && !isNaN(end)) {
          for (let i = Math.max(1, start); i <= Math.min(currentDoc.pageCount, end); i++) {
            pages.add(i);
          }
        }
      } else {
        const p = Number(trimmed);
        if (!isNaN(p) && p >= 1 && p <= currentDoc.pageCount) {
          pages.add(p);
        }
      }
    }
    return pages.size > 0 ? Array.from(pages).sort((a, b) => a - b) : [1];
  }

  onMount(async () => {
    try {
      const res = await getSystemPrinters();
      if (res && res.printers.length > 0) {
        printerList = res.printers;
        printerName = res.default_printer || res.printers[0].name;
      }
    } catch (e) {
      console.warn("getSystemPrinters error:", e);
    }
    renderPreview();
  });

  let isOpeningProperties = false;

  async function openPropertiesDialog() {
    if (isOpeningProperties) return;
    isOpeningProperties = true;
    try {
      pushToast(`'${printerName}' yazıcı tercihleri penceresi açılıyor...`, "info");
      await openPrinterProperties(printerName);
    } catch (e) {
      pushToast(`Yazıcı özellikleri açılamadı: ${e}`, "error");
    } finally {
      isOpeningProperties = false;
    }
  }


  async function renderSinglePageToCanvas(pageNum: number, scale = 0.8): Promise<HTMLCanvasElement> {
    const rendered = await doc.handle.renderPage(pageNum, scale);
    const canvas = document.createElement("canvas");
    canvas.width = rendered.width;
    canvas.height = rendered.height;
    const ctx = canvas.getContext("2d")!;

    if (commentsMode !== "marks_only") {
      ctx.drawImage(rendered, 0, 0);
    }

    // Draw annotations if requested
    if (commentsMode !== "doc_only" && doc.annotations && doc.annotations[pageNum]) {
      const list = doc.annotations[pageNum];
      ctx.save();
      ctx.scale(scale, scale);
      for (const item of list) {
        ctx.save();
        if (item.type === "redact") {
          ctx.fillStyle = "#000000";
          ctx.fillRect(item.x || 0, item.y || 0, item.width || 0, item.height || 0);
        } else if (item.type === "rect") {
          ctx.strokeStyle = item.color || "#000";
          ctx.lineWidth = item.strokeWidth || 3;
          ctx.strokeRect(item.x || 0, item.y || 0, item.width || 0, item.height || 0);
        } else if (item.type === "circle") {
          ctx.strokeStyle = item.color || "#000";
          ctx.lineWidth = item.strokeWidth || 3;
          const rx = (item.width || 0) / 2;
          const ry = (item.height || 0) / 2;
          ctx.beginPath();
          ctx.ellipse((item.x || 0) + rx, (item.y || 0) + ry, rx, ry, 0, 0, Math.PI * 2);
          ctx.stroke();
        } else if (item.type === "text" && item.text) {
          ctx.font = `${item.fontSize || 18}px sans-serif`;
          ctx.fillStyle = item.color || "#9333ea";
          ctx.textBaseline = "top";
          ctx.fillText(item.text, item.x || 0, item.y || 0);
        } else if ((item.type === "stroke" || item.type === "highlighter") && item.points && item.points.length > 1) {
          ctx.strokeStyle = item.color || (item.type === "highlighter" ? "rgba(250,204,21,0.45)" : "#000");
          ctx.lineWidth = item.type === "highlighter" ? 18 : (item.strokeWidth || 3);
          ctx.lineCap = "round";
          ctx.lineJoin = "round";
          ctx.beginPath();
          ctx.moveTo(item.points[0].x, item.points[0].y);
          for (let i = 1; i < item.points.length; i++) ctx.lineTo(item.points[i].x, item.points[i].y);
          ctx.stroke();
        } else if (item.type === "image" && item.imageEl) {
          ctx.drawImage(item.imageEl, item.x || 0, item.y || 0, item.width || 120, item.height || 60);
        }
        ctx.restore();
      }
      ctx.restore();
    }

    return canvas;
  }

  async function renderPreview() {
    if (!doc || !previewCanvas) return;
    isRenderingPreview = true;
    try {
      const rendered = await renderSinglePageToCanvas(currentPreviewPageNum, 0.7);
      const isLandscape = orientation === "landscape" || (orientation === "auto" && rendered.width > rendered.height);
      currentDetectedLandscape = isLandscape;

      const paperInfo = paperDimensions[paperSize] || paperDimensions.A4;

      // Real sheet dimensions for preview container
      const sheetW = isLandscape ? 380 : Math.round(380 * (paperInfo.widthMm / paperInfo.heightMm));
      const sheetH = isLandscape ? Math.round(380 * (paperInfo.widthMm / paperInfo.heightMm)) : 380;

      previewCanvas.width = sheetW;
      previewCanvas.height = sheetH;
      const ctx = previewCanvas.getContext("2d");
      if (!ctx) return;

      // Clean sheet background with crisp white paper
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, sheetW, sheetH);

      if (activeTab === "multiple") {
        // N-up preview
        const startIndex = currentPreviewPageNum;
        const count = pagesPerSheet;
        const cols = count === 2 ? (isLandscape ? 2 : 1) : 2;
        const rows = count === 2 ? (isLandscape ? 1 : 2) : (count === 4 ? 2 : 3);
        const cellW = (sheetW - 24) / cols;
        const cellH = (sheetH - 24) / rows;

        for (let i = 0; i < count; i++) {
          const p = startIndex + i;
          if (p > doc.pageCount) break;
          const col = pageOrder === "horizontal" ? (i % cols) : Math.floor(i / rows);
          const row = pageOrder === "horizontal" ? Math.floor(i / cols) : (i % rows);
          const x = 12 + col * cellW + 4;
          const y = 12 + row * cellH + 4;
          const pCanvas = await renderSinglePageToCanvas(p, 0.35);

          const fitScale = Math.min((cellW - 8) / pCanvas.width, (cellH - 8) / pCanvas.height);
          const drawW = pCanvas.width * fitScale;
          const drawH = pCanvas.height * fitScale;
          const drawX = x + (cellW - 8 - drawW) / 2;
          const drawY = y + (cellH - 8 - drawH) / 2;

          ctx.drawImage(pCanvas, drawX, drawY, drawW, drawH);
          if (printPageBorders) {
            ctx.strokeStyle = "#cbd5e1";
            ctx.lineWidth = 1;
            ctx.strokeRect(drawX, drawY, drawW, drawH);
          }
        }
      } else {
        // Standard single page preview
        // Calculate margin boxes on preview sheet
        const scaleFactor = sheetW / (isLandscape ? paperInfo.heightMm : paperInfo.widthMm);
        const ml = marginLeftMm * scaleFactor;
        const mr = marginRightMm * scaleFactor;
        const mt = marginTopMm * scaleFactor;
        const mb = marginBottomMm * scaleFactor;

        const availW = Math.max(20, sheetW - ml - mr);
        const availH = Math.max(20, sheetH - mt - mb);

        let drawW = rendered.width;
        let drawH = rendered.height;

        if (sizingMode === "custom") {
          const s = (customScale / 100) * Math.min(availW / rendered.width, availH / rendered.height);
          drawW = rendered.width * s;
          drawH = rendered.height * s;
        } else if (sizingMode === "actual") {
          const s = 0.5; // Normalized 100% representation
          drawW = rendered.width * s;
          drawH = rendered.height * s;
        } else {
          // Fit
          const fitScale = Math.min(availW / rendered.width, availH / rendered.height);
          drawW = rendered.width * fitScale;
          drawH = rendered.height * fitScale;
        }

        const drawX = ml + (availW - drawW) / 2;
        const drawY = mt + (availH - drawH) / 2;

        ctx.drawImage(rendered, drawX, drawY, drawW, drawH);

        // Subtle dashed indicator for printable margin boundary
        ctx.strokeStyle = "rgba(99, 102, 241, 0.3)";
        ctx.lineWidth = 1;
        ctx.setLineDash([4, 4]);
        ctx.strokeRect(ml, mt, availW, availH);
        ctx.setLineDash([]);
      }
    } catch (e) {
      console.warn("Preview render error:", e);
    } finally {
      isRenderingPreview = false;
    }
  }

  $: if (
    currentPreviewPageNum ||
    commentsMode ||
    activeTab ||
    pagesPerSheet ||
    orientation ||
    isGrayscale ||
    paperSize ||
    sizingMode ||
    customScale ||
    targetPages
  ) {
    renderPreview();
  }

  async function handlePrint() {
    isPrinting = true;
    try {
      const pagesToPrint = targetPages;
      if (pagesToPrint.length === 0) {
        pushToast("Yazdırılacak sayfa seçilmedi.", "warning");
        return;
      }

      pushToast(`${pagesToPrint.length} sayfa (${copies} kopya) '${printerName}' yazıcısına doğrudan iletiliyor...`, "info");

      // Render crisp print pages (~180-200 DPI)
      const pagePngs: number[][] = [];
      for (const p of pagesToPrint) {
        const c = await renderSinglePageToCanvas(p, 2.0);
        const blob = await new Promise<Blob | null>((resolve) => c.toBlob(resolve, "image/png"));
        if (blob) {
          const buf = await blob.arrayBuffer();
          pagePngs.push(Array.from(new Uint8Array(buf)));
        }
      }

      // DIRECT PRINT: Sends to Windows Print Spooler directly without any Edge browser popups!
      await directPrintPages({
        printer_name: printerName,
        doc_title: doc.name,
        page_pngs: pagePngs,
        copies,
        collate,
        orientation,
        margins: {
          left: marginLeftMm,
          right: marginRightMm,
          top: marginTopMm,
          bottom: marginBottomMm,
        },
        grayscale: isGrayscale,
      });

      pushToast(`Yazdırma komutu '${printerName}' yazıcısına başarıyla gönderildi.`, "success");
      onClose();
    } catch (e: any) {
      pushToast(`Doğrudan yazdırma hatası: ${e}`, "error");
    } finally {
      isPrinting = false;
    }
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4"
  on:click|self={onClose}
>
  <div class="relative flex w-full max-w-4xl flex-col rounded-xl border border-zinc-300 bg-[#f4f4f4] text-zinc-900 shadow-2xl dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 font-sans text-xs">
    <!-- Title Bar -->
    <div class="flex items-center justify-between border-b border-zinc-200 bg-white/80 px-4 py-2.5 dark:border-zinc-800 dark:bg-zinc-800/80 rounded-t-xl">
      <div class="flex items-center gap-2">
        <Printer size={16} class="text-indigo-600 dark:text-indigo-400" />
        <h3 class="text-sm font-semibold">Yazdır - {baseName(doc.name)}</h3>
      </div>
      <button
        class="rounded p-1 text-zinc-400 hover:bg-zinc-200 hover:text-zinc-800 dark:hover:bg-zinc-700 dark:hover:text-zinc-100"
        on:click={onClose}
      >
        <X size={16} />
      </button>
    </div>

    <!-- Main Content Grid -->
    <div class="grid grid-cols-1 md:grid-cols-12 gap-4 p-4 max-h-[80vh] overflow-y-auto">
      <!-- Left Column (Settings): 7 cols -->
      <div class="md:col-span-7 flex flex-col gap-3">
        <!-- Top Yazıcı & Kopyalar -->
        <div class="rounded-lg border border-zinc-300 bg-white p-3 dark:border-zinc-700 dark:bg-zinc-800/60 shadow-2xs space-y-2.5">
          <div class="flex items-center gap-2">
            <span class="w-14 font-medium text-zinc-700 dark:text-zinc-300">Yazıcı:</span>
            <select
              bind:value={printerName}
              class="flex-1 rounded border border-zinc-300 bg-white px-2 py-1 text-xs dark:border-zinc-600 dark:bg-zinc-800 font-medium"
            >
              {#if printerList.length > 0}
                {#each printerList as p}
                  <option value={p.name}>{p.name} {p.is_default ? '(Varsayılan)' : ''}</option>
                {/each}
              {:else}
                <option value="Varsayılan Sistem Yazıcısı">Varsayılan Sistem Yazıcısı</option>
                <option value="Canon E410 series">Canon E410 series</option>
                <option value="Microsoft Print to PDF">Microsoft Print to PDF</option>
              {/if}
            </select>
            <button
              class="rounded border border-zinc-300 px-2.5 py-1 text-xs bg-zinc-50 hover:bg-zinc-100 dark:border-zinc-600 dark:bg-zinc-700 font-medium flex items-center gap-1 text-zinc-700 dark:text-zinc-200 transition-colors shadow-2xs"
              on:click={openPropertiesDialog}
              title="Resmi Windows/üretici yazıcı sürücüsü ayarlarını açar"
            >
              <Settings size={12} class="text-zinc-500" />
              Yazıcı Özellikleri
            </button>
          </div>

          <div class="flex flex-wrap items-center gap-4 pt-1 border-t border-zinc-100 dark:border-zinc-700/60">
            <div class="flex items-center gap-1.5">
              <span class="font-medium text-zinc-700 dark:text-zinc-300">Kopyalar:</span>
              <input
                type="number"
                min="1"
                max="99"
                bind:value={copies}
                class="w-14 rounded border border-zinc-300 px-2 py-0.5 text-xs text-center dark:border-zinc-600 dark:bg-zinc-800 font-bold"
              />
            </div>
            {#if copies > 1}
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input type="checkbox" bind:checked={collate} class="rounded accent-indigo-600" />
                <span>Harmanla (1,2,3... 1,2,3...)</span>
              </label>
            {/if}
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="checkbox" bind:checked={isGrayscale} class="rounded accent-indigo-600" />
              <span>Siyah-Beyaz Baskı (Gri Tonlama)</span>
            </label>
            <label class="flex items-center gap-1.5 cursor-pointer" title="Toner tüketimini azaltmak için kontrastı optimize eder">
              <input type="checkbox" bind:checked={saveInk} class="rounded accent-indigo-600" />
              <span>Mürekkep tasarrufu</span>
            </label>
          </div>
        </div>

        <!-- Yazdırılacak Sayfalar -->
        <fieldset class="rounded-lg border border-zinc-300 bg-white p-3 dark:border-zinc-700 dark:bg-zinc-800/60 shadow-2xs">
          <legend class="px-1 font-semibold text-zinc-800 dark:text-zinc-200">Yazdırılacak Sayfalar</legend>
          <div class="flex items-center gap-4 mt-1">
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="radio" bind:group={pageSelection} value="all" class="accent-indigo-600" />
              <span>Tümü ({doc.pageCount} sayfa)</span>
            </label>
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="radio" bind:group={pageSelection} value="current" class="accent-indigo-600" />
              <span>Mevcut (s. {doc.currentPage})</span>
            </label>
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="radio" bind:group={pageSelection} value="range" class="accent-indigo-600" />
              <span>Aralık:</span>
            </label>
            <input
              type="text"
              bind:value={pageRange}
              disabled={pageSelection !== "range"}
              class="w-28 rounded border border-zinc-300 px-2 py-0.5 text-xs dark:border-zinc-600 dark:bg-zinc-800 disabled:opacity-50 font-mono"
              placeholder="Örn: 1-2, 4"
            />
          </div>
          <div class="mt-2 text-[11px] text-zinc-500 dark:text-zinc-400">
            Kuyruğa gönderilecek: <strong class="text-zinc-800 dark:text-zinc-200">{targetPages.length} sayfa</strong> ({totalSheets} yaprak)
          </div>
        </fieldset>

        <!-- Sayfa Boyutlandırma ve İşleme (Adobe & SumatraPDF stili sekmeler) -->
        <fieldset class="rounded-lg border border-zinc-300 bg-white p-3 dark:border-zinc-700 dark:bg-zinc-800/60 shadow-2xs space-y-2.5">
          <legend class="px-1 font-semibold text-zinc-800 dark:text-zinc-200 flex items-center gap-1">
            Sayfa Boyutlandırma ve İşleme
          </legend>

          <!-- Tab group [Boyut] [Poster] [Çoklu] [Kitapçık] -->
          <div class="flex rounded border border-zinc-300 bg-zinc-100 p-0.5 dark:border-zinc-700 dark:bg-zinc-800 w-fit">
            {#each [["size", "Boyut"], ["multiple", "Çoklu (N-Up)"], ["poster", "Poster"], ["booklet", "Kitapçık"]] as [tab, label]}
              <button
                class="rounded px-3 py-1 font-medium transition-all {activeTab === tab ? 'bg-white shadow-xs dark:bg-zinc-700 text-zinc-900 dark:text-white font-bold' : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900'}"
                on:click={() => (activeTab = tab as any)}
              >
                {label}
              </button>
            {/each}
          </div>

          <!-- Sizing Options Tab -->
          {#if activeTab === "size"}
            <div class="grid grid-cols-2 gap-2 pt-1">
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" bind:group={sizingMode} value="fit" class="accent-indigo-600" />
                <span>Sayfaya sığdır (Önerilen)</span>
              </label>
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" bind:group={sizingMode} value="actual" class="accent-indigo-600" />
                <span>Gerçek boyut (%100)</span>
              </label>
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" bind:group={sizingMode} value="shrink" class="accent-indigo-600" />
                <span>Büyük sayfaları daralt</span>
              </label>
              <div class="flex items-center gap-1.5">
                <label class="flex items-center gap-1.5 cursor-pointer">
                  <input type="radio" bind:group={sizingMode} value="custom" class="accent-indigo-600" />
                  <span>Özel Ölçek:</span>
                </label>
                <input
                  type="number"
                  min="25"
                  max="400"
                  bind:value={customScale}
                  disabled={sizingMode !== "custom"}
                  class="w-14 rounded border border-zinc-300 px-1 py-0.5 text-xs text-center dark:border-zinc-600 dark:bg-zinc-800 disabled:opacity-50 font-bold"
                />
                <span>%</span>
              </div>
            </div>
          {/if}

          <!-- Multiple (N-Up) Tab -->
          {#if activeTab === "multiple"}
            <div class="grid grid-cols-2 gap-3 pt-1 bg-zinc-50 dark:bg-zinc-800/40 p-2.5 rounded border border-zinc-200 dark:border-zinc-700">
              <div class="flex items-center gap-2">
                <span class="font-medium text-zinc-700 dark:text-zinc-300">Yaprak başına:</span>
                <select
                  bind:value={pagesPerSheet}
                  class="rounded border border-zinc-300 bg-white px-2 py-1 text-xs dark:border-zinc-600 dark:bg-zinc-800"
                >
                  <option value={2}>2 sayfa (1x2)</option>
                  <option value={4}>4 sayfa (2x2)</option>
                  <option value={6}>6 sayfa (2x3)</option>
                </select>
              </div>

              <div class="flex items-center gap-2">
                <span class="font-medium text-zinc-700 dark:text-zinc-300">Sıralama:</span>
                <select
                  bind:value={pageOrder}
                  class="rounded border border-zinc-300 bg-white px-2 py-1 text-xs dark:border-zinc-600 dark:bg-zinc-800"
                >
                  <option value="horizontal">Yatay (Soldan Sağa)</option>
                  <option value="vertical">Dikey (Yukarıdan Aşağı)</option>
                </select>
              </div>

              <label class="col-span-2 flex items-center gap-1.5 cursor-pointer pt-1">
                <input type="checkbox" bind:checked={printPageBorders} class="rounded accent-indigo-600" />
                <span>Sayfa kenarlıkları çiz (Kutucuklu ayrım)</span>
              </label>
            </div>
          {/if}

          <!-- Poster Tab -->
          {#if activeTab === "poster"}
            <div class="flex items-center gap-4 pt-1 bg-zinc-50 dark:bg-zinc-800/40 p-2.5 rounded border border-zinc-200 dark:border-zinc-700">
              <div class="flex items-center gap-1.5">
                <span>Döşeme Ölçeği:</span>
                <input type="number" value="100" class="w-14 rounded border border-zinc-300 px-1 py-0.5 text-center text-xs dark:border-zinc-600 dark:bg-zinc-800 font-bold" />
                <span>%</span>
              </div>
              <div class="flex items-center gap-1.5">
                <span>Çakışma:</span>
                <input type="number" value="5" class="w-12 rounded border border-zinc-300 px-1 py-0.5 text-center text-xs dark:border-zinc-600 dark:bg-zinc-800 font-bold" />
                <span>mm</span>
              </div>
            </div>
          {/if}

          <!-- Booklet Tab -->
          {#if activeTab === "booklet"}
            <div class="flex items-center gap-4 pt-1 bg-zinc-50 dark:bg-zinc-800/40 p-2.5 rounded border border-zinc-200 dark:border-zinc-700">
              <span class="font-medium">Kitapçık Alt Kümesi:</span>
              <select class="rounded border border-zinc-300 bg-white px-2 py-1 text-xs dark:border-zinc-600 dark:bg-zinc-800">
                <option>Her iki taraf</option>
                <option>Yalnızca ön taraf</option>
                <option>Yalnızca arka taraf</option>
              </select>
            </div>
          {/if}

          <!-- Yönlendirme (Otomatik / Dikey / Yatay) -->
          <div class="flex items-center gap-4 pt-2 border-t border-zinc-100 dark:border-zinc-700/60">
            <span class="font-medium text-zinc-700 dark:text-zinc-300">Yönlendirme:</span>
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="radio" bind:group={orientation} value="auto" class="accent-indigo-600" />
              <span>Otomatik</span>
            </label>
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="radio" bind:group={orientation} value="portrait" class="accent-indigo-600" />
              <span>Dikey</span>
            </label>
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="radio" bind:group={orientation} value="landscape" class="accent-indigo-600" />
              <span>Yatay</span>
            </label>
          </div>
        </fieldset>

        <!-- Açıklamalar & İşaretleyiciler -->
        <fieldset class="rounded-lg border border-zinc-300 bg-white p-3 dark:border-zinc-700 dark:bg-zinc-800/60 shadow-2xs">
          <legend class="px-1 font-semibold text-zinc-800 dark:text-zinc-200">Açıklamalar & İşaretleyiciler</legend>
          <div class="flex items-center justify-between gap-2 mt-1">
            <select
              bind:value={commentsMode}
              class="flex-1 rounded border border-zinc-300 bg-white px-2 py-1 text-xs dark:border-zinc-600 dark:bg-zinc-800 font-medium"
            >
              <option value="doc_and_marks">Belge ve Tüm İşaretleyiciler (Çizim, Sansür, İmza)</option>
              <option value="doc_only">Yalnızca Orijinal Belge (Çizimleri Gizle)</option>
              <option value="marks_only">Yalnızca İşaretleyiciler</option>
            </select>
          </div>
        </fieldset>
      </div>

      <!-- Right Column (Live Preview matching Adobe & SumatraPDF): 5 cols -->
      <div class="md:col-span-5 flex flex-col items-center justify-between rounded-lg border border-zinc-300 bg-white p-4 dark:border-zinc-700 dark:bg-zinc-800/60 shadow-2xs">
        <div class="flex w-full items-center justify-between text-xs text-zinc-600 dark:text-zinc-400 pb-2">
          <span>
            {#if activeTab === 'multiple'}
              Çoklu: {pagesPerSheet} Sayfa / Yaprak
            {:else if sizingMode === 'custom'}
              Ölçek: %{customScale}
            {:else if sizingMode === 'actual'}
              Ölçek: %100 (Gerçek)
            {:else}
              Ölçek: Sayfaya Sığdır
            {/if}
          </span>
          <span class="font-medium text-zinc-800 dark:text-zinc-200">
            {paperDimensions[paperSize]?.label || "A4"} ({orientation === 'landscape' ? 'Yatay' : 'Dikey'})
          </span>
        </div>

        <!-- Canvas Preview Area -->
        <div class="relative flex flex-1 items-center justify-center p-2 min-h-[300px] w-full bg-zinc-100/70 dark:bg-zinc-900/60 rounded border border-zinc-200 dark:border-zinc-700 overflow-hidden">
          <canvas
            bind:this={previewCanvas}
            class="max-h-[340px] max-w-full object-contain rounded shadow-md border border-zinc-300 bg-white dark:border-zinc-600 transition-all duration-200"
            style="{isGrayscale ? 'filter: grayscale(100%);' : ''} {saveInk ? 'filter: contrast(85%) brightness(105%);' : ''}"
          ></canvas>
          {#if isRenderingPreview}
            <div class="absolute inset-0 flex items-center justify-center bg-white/50 dark:bg-zinc-900/50 backdrop-blur-2xs">
              <span class="text-xs font-semibold text-zinc-700 dark:text-zinc-200">Baskı önizlemesi hazırlanıyor...</span>
            </div>
          {/if}
        </div>

        <!-- Preview Navigation Footer (Browses only the selected target pages) -->
        <div class="flex items-center justify-between w-full pt-3 mt-2 border-t border-zinc-200 dark:border-zinc-700">
          <button
            class="rounded border border-zinc-300 p-1 hover:bg-zinc-100 disabled:opacity-30 dark:border-zinc-600 dark:hover:bg-zinc-700"
            disabled={previewTargetIndex <= 0}
            on:click={() => (previewTargetIndex = Math.max(0, previewTargetIndex - 1))}
            title="Önceki Sayfa"
          >
            <ChevronLeft size={16} />
          </button>
          <span class="text-xs font-medium text-zinc-700 dark:text-zinc-300 text-center">
            Baskı Sayfası: <strong>{currentPreviewPageNum}</strong>
            <span class="text-zinc-400">({previewTargetIndex + 1} / {targetPages.length})</span>
          </span>
          <button
            class="rounded border border-zinc-300 p-1 hover:bg-zinc-100 disabled:opacity-30 dark:border-zinc-600 dark:hover:bg-zinc-700"
            disabled={previewTargetIndex >= targetPages.length - 1}
            on:click={() => (previewTargetIndex = Math.min(targetPages.length - 1, previewTargetIndex + 1))}
            title="Sonraki Sayfa"
          >
            <ChevronRight size={16} />
          </button>
        </div>
      </div>
    </div>

    <!-- Bottom Action Bar -->
    <div class="flex items-center justify-end gap-2.5 border-t border-zinc-200 bg-white/80 px-4 py-2.5 dark:border-zinc-800 dark:bg-zinc-800/80 rounded-b-xl">
      <button
        class="rounded-lg bg-indigo-600 hover:bg-indigo-700 px-5 py-1.5 text-xs font-bold text-white shadow transition-all active:scale-95 disabled:opacity-50 flex items-center gap-1.5"
        on:click={handlePrint}
        disabled={isPrinting}
      >
        <Printer size={14} />
        {isPrinting ? "Yazıcıya Gönderiliyor..." : "Yazdır"}
      </button>
      <button
        class="rounded-lg border border-zinc-300 bg-white hover:bg-zinc-100 px-4 py-1.5 text-xs font-semibold text-zinc-700 dark:border-zinc-600 dark:bg-zinc-700 dark:text-zinc-200 dark:hover:bg-zinc-600 transition-colors"
        on:click={onClose}
        disabled={isPrinting}
      >
        İptal
      </button>
    </div>
  </div>
</div>
