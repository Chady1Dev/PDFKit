<script lang="ts">
  import { onDestroy } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import ThumbnailGrid, { type GridItem } from "../components/ThumbnailGrid.svelte";
  import PreviewModal from "../components/PreviewModal.svelte";
  import ImageThumb from "../components/ImageThumb.svelte";
  import ImageViewerModal from "../components/ImageViewerModal.svelte";
  import PythonGatekeeper from "../components/PythonGatekeeper.svelte";
  import { openPdf, type PdfDocHandle } from "../pdf";
  import {
    imagesToPdf,
    writeBinaryFile,
    exportOfficeDocument,
    cancelConversion,
    type ConversionProgress,
    baseName,
    fmtSize,
  } from "../api";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";
  import {
    ChevronLeft,
    ChevronRight,
    Trash2,
    AlertCircle,
    Maximize2,
    FileText,
    FileSpreadsheet,
    Presentation,
    AlignLeft,
    Image,
    Layers,
    Sparkles,
    Check,
    CheckSquare,
    X,
    FolderCheck,
    RotateCw,
    Info,
    CheckCircle2,
  } from "@lucide/svelte";

  // Tab State: "from_pdf" (PDF'ten Dönüştür) vs "to_pdf" (PDF'ye Dönüştür)
  let activeTab: "from_pdf" | "to_pdf" = "from_pdf";
  let busy = false;
  let progress = -1; // -1 = idle/hidden
  let progressText = "";
  let unlistenProgress: (() => void) | null = null;

  // --- TAB 1: PDF'ten Dönüştür ---
  let pdfDoc: PdfDocHandle | null = null;
  let fromPdfTarget: "office" | "images" = "office";
  let exportFormat: "docx" | "xlsx" | "pptx" | "txt" = "docx";
  let exportLanguage = "tr";

  // Image export options
  let items: GridItem[] = [];
  let selection = new Set<number>();
  let previewIndex: number | null = null;
  let imageFormat: "png" | "jpg" = "png";
  let imageScale = 3; // Default 216 DPI

  async function loadPdf(paths: string[]) {
    if (!paths || paths.length === 0) return;
    await unloadPdf();
    try {
      pdfDoc = await openPdf(paths[0]);
      items = Array.from({ length: pdfDoc.pageCount }, (_, i) => ({
        key: `img-p${i + 1}`,
        page: i + 1,
        rotation: 0,
      }));
      selection = new Set(items.map((_, i) => i)); // Default all selected
    } catch (e) {
      pushToast(`PDF açılamadı: ${e}`, "error");
    }
  }

  async function unloadPdf() {
    if (pdfDoc) await pdfDoc.destroy();
    pdfDoc = null;
    items = [];
    selection = new Set();
    previewIndex = null;
    progress = -1;
    progressText = "";
  }

  function toggleSelectAll() {
    if (selection.size === items.length) {
      selection = new Set();
    } else {
      selection = new Set(items.map((_, i) => i));
    }
  }

  async function handleCancelConversion() {
    if (!busy) return;
    try {
      await cancelConversion();
      pushToast("Dönüştürme işlemi durduruldu ve iptal edildi", "info");
    } catch (err) {
      console.error("Cancel conversion error:", err);
    } finally {
      busy = false;
      progress = -1;
      progressText = "";
      if (unlistenProgress) {
        unlistenProgress();
        unlistenProgress = null;
      }
    }
  }

  async function exportDocumentFormat() {
    if (!pdfDoc) return;
    busy = true;
    progress = 0;
    progressText = "Başlatılıyor...";

    const stem = baseName(pdfDoc.path).replace(/\.pdf$/i, "");
    const formatName =
      exportFormat === "docx"
        ? "Microsoft Word"
        : exportFormat === "xlsx"
        ? "Microsoft Excel"
        : exportFormat === "pptx"
        ? "Microsoft PowerPoint"
        : "Metin Belgesi";

    try {
      const outPath = await save({
        filters: [{ name: formatName, extensions: [exportFormat] }],
        defaultPath: `${stem}.${exportFormat}`,
      });
      if (!outPath) {
        busy = false;
        progress = -1;
        progressText = "";
        return;
      }

      // Real-time progress updates streamed from Python / Rust
      unlistenProgress = await listen<ConversionProgress>("conversion-progress", (event) => {
        const p = event.payload;
        progress = p.percent;
        if (p.current > 0 && p.total > 0) {
          progressText = `Sayfa ${p.current} / ${p.total} (%${p.percent})`;
        } else {
          progressText = `Dönüştürme başlatılıyor...`;
        }
      });

      // Extract text only for spreadsheet or text formats; docx and pptx use the native Python engine with direct inputPath
      const pagesText: { page: number; text: string }[] = [];
      if (exportFormat === "xlsx" || exportFormat === "txt") {
        for (let p = 1; p <= pdfDoc.pageCount; p++) {
          if (!busy) break;
          try {
            const pageText = await pdfDoc.getPageText(p);
            pagesText.push({ page: p, text: pageText });
          } catch (err) {
            pagesText.push({ page: p, text: "" });
          }
          progress = Math.round((p / pdfDoc.pageCount) * 100);
          progressText = `Sayfa ${p} / ${pdfDoc.pageCount} taranıyor...`;
        }
      }

      if (!busy) return;

      await exportOfficeDocument(outPath, exportFormat, stem, pagesText, exportLanguage, pdfDoc.path);

      logActivity([baseName(pdfDoc.path)], `${formatName} Dönüşümü Yapıldı`, outPath);
      pushToast(`Başarıyla dönüştürüldü: ${baseName(outPath)}`, "success", {
        label: "Klasöre Git",
        path: outPath,
      });
    } catch (e) {
      if (busy) {
        pushToast(`Dönüştürme hatası: ${e}`, "error");
      }
    } finally {
      busy = false;
      progress = -1;
      progressText = "";
      if (unlistenProgress) {
        unlistenProgress();
        unlistenProgress = null;
      }
    }
  }

  async function exportImages() {
    if (!pdfDoc) return;
    if (selection.size === 0) {
      pushToast("Dışa aktarmak için en az bir sayfa seçmelisiniz", "warning");
      return;
    }
    const selected = await open({ directory: true, multiple: false });
    const dir = Array.isArray(selected) ? selected[0] : selected;
    if (!dir) return;

    busy = true;
    progress = 0;
    progressText = "Görseller hazırlanıyor...";
    const stem = baseName(pdfDoc.path).replace(/\.pdf$/i, "");
    const targetIndices = Array.from(selection).sort((a, b) => a - b);

    try {
      for (let idx = 0; idx < targetIndices.length; idx++) {
        if (!busy) break;
        const p = items[targetIndices[idx]].page;
        progressText = `Sayfa ${p} / ${pdfDoc.pageCount} (${idx + 1}/${targetIndices.length}) aktarılıyor`;
        const canvas = await pdfDoc.renderPage(p, imageScale);
        const mime = imageFormat === "jpg" ? "image/jpeg" : "image/png";
        const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, mime, 0.92));
        if (!blob) throw new Error("Görsel oluşturulamadı");
        const bytes = new Uint8Array(await blob.arrayBuffer());
        await writeBinaryFile(`${dir}/${stem}_s${p}.${imageFormat}`, bytes);
        progress = Math.round(((idx + 1) / targetIndices.length) * 100);
      }

      if (!busy) return;

      logActivity([baseName(pdfDoc.path)], "Görsel Dönüşüm Yapıldı", dir);
      pushToast(
        `${targetIndices.length} sayfa ${imageFormat.toUpperCase()} (${imageScale * 72} DPI) olarak kaydedildi`,
        "success",
        {
          label: "Klasöre Git",
          path: dir,
        },
      );
    } catch (e) {
      if (busy) {
        pushToast(`Hata: ${e}`, "error");
      }
    } finally {
      busy = false;
      progress = -1;
      progressText = "";
    }
  }

  // --- TAB 2: PDF'ye Dönüştür ---
  let images: string[] = [];
  let dragImgIndex: number | null = null;
  let dragOverImgIndex: number | null = null;
  let imageModalIndex: number | null = null;

  const ALLOWED_IMAGE_EXTS = ["png", "jpg", "jpeg", "webp", "bmp", "tiff", "tif"];
  const ALLOWED_DOC_EXTS = ["txt", "csv", "svg"];
  const ALL_ALLOWED_EXTS = [...ALLOWED_IMAGE_EXTS, ...ALLOWED_DOC_EXTS];

  function validateAndAddFiles(newPaths: string[]) {
    if (!newPaths || newPaths.length === 0) return;

    const invalidFiles: string[] = [];
    const validPaths: string[] = [];

    for (const p of newPaths) {
      const ext = p.split(".").pop()?.toLowerCase() ?? "";
      if (!ALL_ALLOWED_EXTS.includes(ext)) {
        invalidFiles.push(baseName(p));
      } else {
        validPaths.push(p);
      }
    }

    if (invalidFiles.length > 0) {
      pushToast(
        `Desteklenmeyen dosya türü tespit edildi (${invalidFiles.slice(0, 3).join(", ")}${invalidFiles.length > 3 ? "..." : ""}). Yalnızca görsel ve metin formatları kabul edilir.`,
        "warning",
      );
    }

    if (validPaths.length === 0) return;

    // Check category incompatibility (don't mix image files with txt/csv documents)
    const currentTypes = images.map((p) => p.split(".").pop()?.toLowerCase() ?? "");
    const hasCurrentImages = currentTypes.some((t) => ALLOWED_IMAGE_EXTS.includes(t));
    const hasCurrentDocs = currentTypes.some((t) => ALLOWED_DOC_EXTS.includes(t));

    const newTypes = validPaths.map((p) => p.split(".").pop()?.toLowerCase() ?? "");
    const hasNewImages = newTypes.some((t) => ALLOWED_IMAGE_EXTS.includes(t));
    const hasNewDocs = newTypes.some((t) => ALLOWED_DOC_EXTS.includes(t));

    if ((hasCurrentImages && hasNewDocs) || (hasCurrentDocs && hasNewImages) || (hasNewImages && hasNewDocs)) {
      pushToast(
        "Farklı kategorideki dosyalar (görseller ile metin/tablo belgeleri) aynı anda karıştırılamaz. Lütfen tek bir kategori seçin.",
        "warning",
      );
      return;
    }

    images = [...images, ...validPaths];
  }

  function moveImage(from: number, to: number) {
    if (from < 0 || from >= images.length || to < 0 || to >= images.length) return;
    const next = [...images];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    images = next;
  }

  function removeImage(index: number) {
    images = images.filter((_, i) => i !== index);
  }

  function clearAllImages() {
    images = [];
    selectedImageIndices = [];
  }

  $: if ($pendingFiles && $pendingFiles.targetView === "images") {
    const pdfs = $pendingFiles.paths.filter((x) => x.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      activeTab = "from_pdf";
      loadPdf(pdfs);
    } else {
      activeTab = "to_pdf";
      validateAndAddFiles($pendingFiles.paths);
    }
    pendingFiles.set(null);
  }

  // --- TAB 2 Selection & Pointer Drag state ---
  let selectedImageIndices: number[] = [];
  $: selectedImagesSet = new Set(selectedImageIndices);

  let isDraggingImg = false;
  let dragCandidateImg: { index: number; startX: number; startY: number } | null = null;
  let activeDragImgIndices: number[] = [];
  let dragOverImgTargetIndex: number | null = null;
  let pointerImgX = 0;
  let pointerImgY = 0;
  let autoScrollImgSpeed = 0;
  let scrollImgAnimId: number | null = null;
  let wasDraggingImg = false;
  let imgGridContainer: HTMLDivElement | null = null;

  function toggleImageSelect(index: number) {
    if (selectedImagesSet.has(index)) {
      selectedImageIndices = selectedImageIndices.filter((i) => i !== index);
    } else {
      selectedImageIndices = [...selectedImageIndices, index];
    }
  }

  function toggleSelectAllImages() {
    if (selectedImageIndices.length === images.length) {
      selectedImageIndices = [];
    } else {
      selectedImageIndices = images.map((_, i) => i);
    }
  }

  function clearImageSelections() {
    selectedImageIndices = [];
  }

  function deleteSelectedImages() {
    if (selectedImageIndices.length === 0) return;
    const toRemove = new Set(selectedImageIndices);
    images = images.filter((_, i) => !toRemove.has(i));
    selectedImageIndices = [];
  }

  function reorderMultipleImages(draggedIndices: number[], targetIndex: number) {
    if (draggedIndices.length === 0 || targetIndex < 0 || targetIndex >= images.length) return;
    const moving = draggedIndices.map((i) => images[i]);
    const draggedSet = new Set(draggedIndices);
    const targetItem = images[targetIndex];
    const remaining = images.filter((_, i) => !draggedSet.has(i));

    let insertPos = remaining.indexOf(targetItem);
    if (insertPos === -1) {
      insertPos = Math.min(targetIndex, remaining.length);
    }

    const next = [
      ...remaining.slice(0, insertPos),
      ...moving,
      ...remaining.slice(insertPos),
    ];
    images = next;
    selectedImageIndices = Array.from({ length: moving.length }, (_, k) => insertPos + k);
  }

  function handleCardPointerDownImg(e: PointerEvent, index: number) {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (target && (target.closest("button") || target.closest("input") || target.closest("a"))) {
      return;
    }
    dragCandidateImg = { index, startX: e.clientX, startY: e.clientY };
  }

  function handleWindowPointerMoveImg(e: PointerEvent) {
    if (dragCandidateImg && !isDraggingImg) {
      const dist = Math.hypot(e.clientX - dragCandidateImg.startX, e.clientY - dragCandidateImg.startY);
      if (dist > 6) {
        isDraggingImg = true;
        wasDraggingImg = true;
        if (selectedImagesSet.has(dragCandidateImg.index) && selectedImagesSet.size > 1) {
          activeDragImgIndices = [...selectedImageIndices];
        } else {
          activeDragImgIndices = [dragCandidateImg.index];
        }
      }
    }

    if (isDraggingImg) {
      pointerImgX = e.clientX;
      pointerImgY = e.clientY;
      updateDragTargetAndScrollImg(e.clientX, e.clientY);
    }
  }

  function updateDragTargetAndScrollImg(clientX: number, clientY: number) {
    if (!imgGridContainer) return;
    const rect = imgGridContainer.getBoundingClientRect();
    const relY = clientY - rect.top;

    const threshold = 75;
    if (relY < threshold && clientY >= rect.top - 40) {
      const intensity = Math.max(0.15, (threshold - relY) / threshold);
      autoScrollImgSpeed = -Math.ceil(intensity * 24);
      startAutoScrollImg();
    } else if (relY > rect.height - threshold && clientY <= rect.bottom + 40) {
      const intensity = Math.max(0.15, (relY - (rect.height - threshold)) / threshold);
      autoScrollImgSpeed = Math.ceil(intensity * 24);
      startAutoScrollImg();
    } else {
      stopAutoScrollImg();
    }

    const el = document.elementFromPoint(clientX, clientY);
    const card = el?.closest("[data-img-index]");
    if (card) {
      const idx = Number(card.getAttribute("data-img-index"));
      if (!isNaN(idx) && idx >= 0 && idx < images.length) {
        dragOverImgTargetIndex = idx;
      }
    }
  }

  function startAutoScrollImg() {
    if (scrollImgAnimId !== null) return;
    function loop() {
      if (!isDraggingImg || autoScrollImgSpeed === 0 || !imgGridContainer) {
        stopAutoScrollImg();
        return;
      }
      imgGridContainer.scrollTop += autoScrollImgSpeed;
      updateDragTargetAndScrollImg(pointerImgX, pointerImgY);
      scrollImgAnimId = requestAnimationFrame(loop);
    }
    scrollImgAnimId = requestAnimationFrame(loop);
  }

  function stopAutoScrollImg() {
    if (scrollImgAnimId !== null) {
      cancelAnimationFrame(scrollImgAnimId);
      scrollImgAnimId = null;
    }
    autoScrollImgSpeed = 0;
  }

  function handleWindowPointerUpImg() {
    if (isDraggingImg && dragOverImgTargetIndex !== null && activeDragImgIndices.length > 0) {
      reorderMultipleImages(activeDragImgIndices, dragOverImgTargetIndex);
    }
    stopAutoScrollImg();
    isDraggingImg = false;
    dragCandidateImg = null;
    activeDragImgIndices = [];
    dragOverImgTargetIndex = null;
    setTimeout(() => {
      wasDraggingImg = false;
    }, 50);
  }

  function handleWindowKeyDownImg(e: KeyboardEvent) {
    if (e.key === "Escape" && isDraggingImg) {
      stopAutoScrollImg();
      isDraggingImg = false;
      dragCandidateImg = null;
      activeDragImgIndices = [];
      dragOverImgTargetIndex = null;
    }
  }

  async function convertToPdf() {
    if (images.length === 0) return;
    const output = await save({
      filters: [{ name: "PDF Belgesi", extensions: ["pdf"] }],
      defaultPath: "birlestirilmis_belge.pdf",
    });
    if (!output) return;

    busy = true;
    progress = 0;
    progressText = "PDF oluşturuluyor...";

    try {
      const result = await imagesToPdf(images, output);
      logActivity(images.map((img) => baseName(img)), "PDF Oluşturuldu", output);
      pushToast(
        `Kaydedildi: ${baseName(result.outputs[0])} — ${result.total_pages} sayfa, ${fmtSize(result.total_size_bytes)}`,
        "success",
        {
          label: "Klasöre Git",
          path: output,
        },
      );
      images = [];
    } catch (e) {
      if (busy) {
        pushToast(`Hata: ${e}`, "error");
      }
    } finally {
      busy = false;
      progress = -1;
      progressText = "";
    }
  }

  onDestroy(() => {
    unloadPdf();
    if (unlistenProgress) {
      unlistenProgress();
      unlistenProgress = null;
    }
  });
</script>

<svelte:window
  on:pointermove={handleWindowPointerMoveImg}
  on:pointerup={handleWindowPointerUpImg}
  on:keydown={handleWindowKeyDownImg}
/>

<ToolHeader
  title="PDF Dönüştürücü"
  description="PDF belgelerinizi düzenlenebilir Office belgelerine veya görsellere dönüştürün; ya da dosyalarınızdan tek bir PDF oluşturun"
>
  <div class="flex rounded-xl bg-zinc-200/80 p-1 dark:bg-zinc-800/80 border border-zinc-300/60 dark:border-zinc-700/60 shadow-inner">
    <button
      type="button"
      class="flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-bold transition-all {activeTab === 'from_pdf'
        ? 'bg-white text-indigo-600 shadow-sm dark:bg-zinc-700 dark:text-white'
        : 'text-zinc-600 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-200'}"
      on:click={() => (activeTab = "from_pdf")}
    >
      <FileText size={15} />
      <span>PDF'ten Dönüştür</span>
    </button>
    <button
      type="button"
      class="flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-bold transition-all {activeTab === 'to_pdf'
        ? 'bg-white text-indigo-600 shadow-sm dark:bg-zinc-700 dark:text-white'
        : 'text-zinc-600 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-200'}"
      on:click={() => (activeTab = "to_pdf")}
    >
      <Layers size={15} />
      <span>PDF'ye Dönüştür</span>
    </button>
  </div>
</ToolHeader>

<!-- ========================================== -->
<!-- TAB 1: PDF'TEN DÖNÜŞTÜR (FROM PDF)         -->
<!-- ========================================== -->
{#if activeTab === "from_pdf"}
  <!-- Bilgi & Desteklenen Formatlar Rozetleri -->
  <div class="mb-4 flex flex-col gap-2.5 rounded-2xl border border-indigo-100 bg-gradient-to-r from-indigo-50/80 via-blue-50/50 to-indigo-50/30 p-3.5 text-xs text-indigo-950 dark:border-indigo-900/30 dark:from-indigo-950/30 dark:via-blue-950/20 dark:to-zinc-900 dark:text-indigo-200 shadow-xs">
    <div class="flex items-center gap-2.5">
      <Sparkles size={17} class="shrink-0 text-indigo-600 dark:text-indigo-400" />
      <span>
        <strong>Dönüştürülebilir Formatlar:</strong> PDF belgenizi tam sayfa düzeni korumalı Word, PowerPoint sunumu, Excel tablosu, metin veya yüksek çözünürlüklü görsellere aktarın.
      </span>
    </div>
    <div class="flex flex-wrap items-center gap-1.5 font-bold pl-7">
      <span class="rounded-md bg-blue-100 px-2 py-0.5 text-[11px] text-blue-700 dark:bg-blue-900/50 dark:text-blue-300">DOCX</span>
      <span class="rounded-md bg-orange-100 px-2 py-0.5 text-[11px] text-orange-700 dark:bg-orange-900/50 dark:text-orange-300">PPTX</span>
      <span class="rounded-md bg-emerald-100 px-2 py-0.5 text-[11px] text-emerald-700 dark:bg-emerald-900/50 dark:text-emerald-300">XLSX</span>
      <span class="rounded-md bg-zinc-200 px-2 py-0.5 text-[11px] text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300">TXT</span>
      <span class="rounded-md bg-purple-100 px-2 py-0.5 text-[11px] text-purple-700 dark:bg-purple-900/50 dark:text-purple-300">PNG/JPG</span>
    </div>
  </div>

  {#if !pdfDoc}
    <FileDropzone
      extensions={["pdf"]}
      label="Dönüştürmek istediğiniz PDF belgesini buraya sürükleyin veya seçin"
      onFiles={loadPdf}
    />
  {:else}
    <!-- Belge Bilgi Kartı -->
    <div class="mb-4 flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-zinc-200 bg-white p-4 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
      <div class="flex items-center gap-3">
        <div class="flex h-11 w-11 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 dark:bg-indigo-950/60 dark:text-indigo-400">
          <FileText size={22} />
        </div>
        <div>
          <h3 class="text-sm font-bold text-zinc-900 dark:text-zinc-100" title={pdfDoc.path}>
            {baseName(pdfDoc.path)}
          </h3>
          <p class="text-xs text-zinc-500 dark:text-zinc-400">
            {pdfDoc.pageCount} sayfa yüklendi &bull; Yerel ve Güvenli İşlem
          </p>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <!-- Sub-switch: Office vs Görsel -->
        <div class="flex rounded-lg bg-zinc-100 p-0.5 dark:bg-zinc-800">
          <button
            type="button"
            class="rounded-md px-3 py-1.5 text-xs font-semibold transition-all {fromPdfTarget === 'office'
              ? 'bg-white text-zinc-900 shadow dark:bg-zinc-700 dark:text-white'
              : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400'}"
            on:click={() => (fromPdfTarget = "office")}
          >
            Office & Metin
          </button>
          <button
            type="button"
            class="rounded-md px-3 py-1.5 text-xs font-semibold transition-all {fromPdfTarget === 'images'
              ? 'bg-white text-zinc-900 shadow dark:bg-zinc-700 dark:text-white'
              : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400'}"
            on:click={() => (fromPdfTarget = "images")}
          >
            Sayfa Görselleri (PNG/JPG)
          </button>
        </div>

        <button
          type="button"
          class="rounded-xl border border-zinc-200 bg-zinc-50 px-3 py-1.5 text-xs font-semibold text-rose-600 hover:bg-rose-50 hover:border-rose-200 dark:border-zinc-700 dark:bg-zinc-800 dark:hover:bg-rose-950/30 transition-all"
          on:click={unloadPdf}
          disabled={busy}
        >
          PDF'i Değiştir
        </button>
      </div>
    </div>

    <!-- Active Process Progress Bar & Cancellation Button -->
    <!-- Active Process Progress Bar & Cancellation Card (Shown exclusively when busy) -->
    {#if busy && progress >= 0}
      <div class="mx-auto max-w-2xl rounded-2xl border border-indigo-200 bg-white p-6 shadow-xl dark:border-indigo-900/50 dark:bg-zinc-900 animate-in fade-in zoom-in-95 duration-150">
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-center gap-2.5 text-sm font-bold text-zinc-900 dark:text-zinc-100">
            <RotateCw size={18} class="animate-spin text-indigo-600 dark:text-indigo-400" />
            <span>{progressText || "Dönüştürülüyor..."}</span>
          </div>
          <span class="rounded-lg bg-indigo-600 px-2.5 py-1 text-xs font-bold text-white shadow-xs">
            {Math.round(progress)}%
          </span>
        </div>

        <div class="h-3 w-full overflow-hidden rounded-full bg-zinc-100 dark:bg-zinc-800">
          <div
            class="h-full rounded-full bg-gradient-to-r from-indigo-500 via-blue-500 to-indigo-600 transition-all duration-300"
            style="width: {Math.min(100, Math.max(0, progress))}%"
          ></div>
        </div>

        <div class="mt-5 flex items-center justify-between border-t border-zinc-100 pt-4 text-xs text-zinc-500 dark:border-zinc-800 dark:text-zinc-400">
          <span>İşlem büyük belgelerde daha uzun sürebilir.</span>
          <button
            type="button"
            class="rounded-xl bg-rose-600 hover:bg-rose-700 active:scale-95 px-4 py-2 font-bold text-white shadow transition-all"
            on:click={handleCancelConversion}
          >
            Vazgeç / İptal Et
          </button>
        </div>
      </div>
    {:else if fromPdfTarget === "office"}
      <!-- SECTION A: OFFICE & METİN FORMATLARI -->
      <PythonGatekeeper requiredFormat={exportFormat} on:bypass={() => (fromPdfTarget = "images")}>
        <div class="mx-auto max-w-2xl rounded-2xl border border-zinc-200 bg-white p-6 shadow-xl dark:border-zinc-800 dark:bg-zinc-900">
          <h4 class="mb-3 text-xs font-bold uppercase tracking-wider text-zinc-500 dark:text-zinc-400">
          Hedef Çıktı Formatı Seçin
        </h4>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-5">
          <!-- DOCX -->
          <label
            class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all
              {exportFormat === 'docx'
              ? 'border-blue-600 bg-blue-50/60 dark:bg-blue-950/30 ring-2 ring-blue-500/30 shadow-xs'
              : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
          >
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-2.5">
                <div class="rounded-lg bg-blue-100 p-2 text-blue-600 dark:bg-blue-900/40 dark:text-blue-400">
                  <FileText size={20} />
                </div>
                <div>
                  <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Microsoft Word</h5>
                  <p class="text-[11px] text-zinc-500 dark:text-zinc-400">.docx formatı</p>
                </div>
              </div>
              <input type="radio" bind:group={exportFormat} value="docx" class="accent-blue-600 h-4 w-4 mt-1" />
            </div>
            <div class="mt-3 flex items-center justify-between text-[11px]">
              <span class="text-zinc-500 dark:text-zinc-400">Gelişmiş Düzen Analizi</span>
              <span class="rounded bg-blue-100 px-2 py-0.5 font-bold text-blue-700 dark:bg-blue-900/60 dark:text-blue-300">
                %99 Uyumlu
              </span>
            </div>
          </label>

          <!-- PPTX -->
          <label
            class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all
              {exportFormat === 'pptx'
              ? 'border-orange-600 bg-orange-50/60 dark:bg-orange-950/30 ring-2 ring-orange-500/30 shadow-xs'
              : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
          >
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-2.5">
                <div class="rounded-lg bg-orange-100 p-2 text-orange-600 dark:bg-orange-900/40 dark:text-orange-400">
                  <Presentation size={20} />
                </div>
                <div>
                  <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Microsoft PowerPoint</h5>
                  <p class="text-[11px] text-zinc-500 dark:text-zinc-400">.pptx sunum slaytları</p>
                </div>
              </div>
              <input type="radio" bind:group={exportFormat} value="pptx" class="accent-orange-600 h-4 w-4 mt-1" />
            </div>
            <div class="mt-3 text-[11px] text-zinc-500 dark:text-zinc-400">
              16:9 Sunum Slaytları
            </div>
          </label>

          <!-- XLSX -->
          <label
            class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all
              {exportFormat === 'xlsx'
              ? 'border-emerald-600 bg-emerald-50/60 dark:bg-emerald-950/30 ring-2 ring-emerald-500/30 shadow-xs'
              : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
          >
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-2.5">
                <div class="rounded-lg bg-emerald-100 p-2 text-emerald-600 dark:bg-emerald-900/40 dark:text-emerald-400">
                  <FileSpreadsheet size={20} />
                </div>
                <div>
                  <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Microsoft Excel</h5>
                  <p class="text-[11px] text-zinc-500 dark:text-zinc-400">.xlsx hesap tablosu</p>
                </div>
              </div>
              <input type="radio" bind:group={exportFormat} value="xlsx" class="accent-emerald-600 h-4 w-4 mt-1" />
            </div>
            <div class="mt-3 text-[11px] text-zinc-500 dark:text-zinc-400">
              Tablo & Veri Ayrıştırma
            </div>
          </label>

          <!-- TXT -->
          <label
            class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all
              {exportFormat === 'txt'
              ? 'border-indigo-600 bg-indigo-50/60 dark:bg-indigo-950/30 ring-2 ring-indigo-500/30 shadow-xs'
              : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
          >
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-2.5">
                <div class="rounded-lg bg-zinc-200 p-2 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                  <AlignLeft size={20} />
                </div>
                <div>
                  <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Düz Metin</h5>
                  <p class="text-[11px] text-zinc-500 dark:text-zinc-400">.txt metin dosyası</p>
                </div>
              </div>
              <input type="radio" bind:group={exportFormat} value="txt" class="accent-indigo-600 h-4 w-4 mt-1" />
            </div>
            <div class="mt-3 text-[11px] text-zinc-500 dark:text-zinc-400">
              Evrensel Metin Standartı
            </div>
          </label>
        </div>

        <!-- Seçenekler: Belge Dili -->
        <div class="flex flex-wrap items-center justify-between gap-3 pt-4 border-t border-zinc-100 dark:border-zinc-800">
          <div class="flex items-center gap-2 text-xs text-zinc-600 dark:text-zinc-400">
            <Info size={15} />
            <span>Tanınan Belge Dili (Karakter Boşluk Düzeltmesi):</span>
          </div>
          <select
            bind:value={exportLanguage}
            class="rounded-xl border border-zinc-300 bg-white px-3 py-1.5 text-xs font-semibold text-zinc-800 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200 shadow-xs"
          >
            <option value="tr">Türkçe (ç, ğ, ı, ö, ş, ü otomatik koruma)</option>
            <option value="en">İngilizce - US</option>
            <option value="de">Almanca - DE</option>
            <option value="fr">Fransızca - FR</option>
          </select>
        </div>

        <!-- Butonlar -->
        <div class="mt-6 flex items-center justify-end gap-3 pt-4 border-t border-zinc-100 dark:border-zinc-800">
          <ActionButton
            label="Dönüştür ve Dışa Aktar"
            icon={FileText}
            {busy}
            onClick={exportDocumentFormat}
          />
        </div>
      </div>
    </PythonGatekeeper>
    {:else if fromPdfTarget === "images"}
      <div class="mb-3 flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-zinc-200 bg-white p-3.5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900 text-sm">
        <div class="flex flex-wrap items-center gap-3">
          <span class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">
            {selection.size} / {items.length} sayfa seçili
          </span>

          <button
            type="button"
            class="rounded-lg border border-zinc-300 bg-zinc-100 px-2.5 py-1 text-xs font-medium text-zinc-700 hover:bg-zinc-200 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700"
            on:click={toggleSelectAll}
          >
            {selection.size === items.length && items.length > 0 ? "Seçimi Kaldır" : "Tümünü Seç"}
          </button>

          <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-800"></div>

          <!-- Format (PNG / JPG) -->
          <div class="flex items-center gap-1.5">
            <span class="text-xs font-medium text-zinc-500 dark:text-zinc-400">Format:</span>
            <select
              bind:value={imageFormat}
              class="rounded-lg border border-zinc-300 bg-white px-2.5 py-1 text-xs font-semibold text-zinc-800 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-100"
            >
              <option value="png">PNG (Şeffaf / Kayıpsız)</option>
              <option value="jpg">JPG (Kompakt Boyut)</option>
            </select>
          </div>

          <!-- DPI Çözünürlük -->
          <div class="flex items-center gap-1.5">
            <span class="text-xs font-medium text-zinc-500 dark:text-zinc-400">Çözünürlük:</span>
            <select
              bind:value={imageScale}
              class="rounded-lg border border-zinc-300 bg-white px-2.5 py-1 text-xs font-semibold text-zinc-800 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-100"
            >
              <option value={3}>216 DPI (Önerilen / Yüksek)</option>
              <option value={2}>144 DPI (Orta Kalite)</option>
              <option value={1}>72 DPI (Düşük / Hızlı)</option>
              <option value={4}>300 DPI (Baskı Kalitesi)</option>
            </select>
          </div>
        </div>

        {#if busy}
          <button
            type="button"
            class="rounded-xl bg-rose-500 hover:bg-rose-600 px-4 py-2 text-xs font-bold text-white shadow transition-all active:scale-95"
            on:click={handleCancelConversion}
          >
            Vazgeç
          </button>
        {:else}
          <ActionButton
            label={`Seçili Sayfaları Dışa Aktar (${selection.size})`}
            {busy}
            disabled={selection.size === 0}
            onClick={exportImages}
          />
        {/if}
      </div>

      <ThumbnailGrid
        {items}
        {selection}
        renderPage={pdfDoc.renderPage}
        rotatable
        removable
        reorderable
        showCheckbox={true}
        onToggleSelect={(i) => {
          const next = new Set(selection);
          if (next.has(i)) next.delete(i);
          else next.add(i);
          selection = next;
        }}
        onPreview={(i) => (previewIndex = i)}
        onRotate={(i) => {
          items = items.map((it, j) => (j === i ? { ...it, rotation: (it.rotation + 90) % 360 } : it));
        }}
        onRemove={(i) => {
          items = items.filter((_, j) => j !== i);
        }}
        onReorder={(from, to) => {
          const next = [...items];
          const [moved] = next.splice(from, 1);
          next.splice(to, 0, moved);
          items = next;
        }}
        onReorderMultiple={(indices, target) => {
          const moving = indices.map((i) => items[i]);
          const movingSet = new Set(indices);
          const targetItem = items[target];
          const remaining = items.filter((_, i) => !movingSet.has(i));
          let insertPos = remaining.indexOf(targetItem);
          if (insertPos === -1) insertPos = Math.min(target, remaining.length);
          items = [...remaining.slice(0, insertPos), ...moving, ...remaining.slice(insertPos)];
        }}
      />
    {/if}
  {/if}
{/if}

<!-- ========================================== -->
<!-- TAB 2: PDF'YE DÖNÜŞTÜR (TO PDF)           -->
<!-- ========================================== -->
{#if activeTab === "to_pdf"}
  <!-- Bilgi & Desteklenen Formatlar Rozetleri -->
  <div class="mb-4 flex flex-col gap-2.5 rounded-2xl border border-emerald-100 bg-gradient-to-r from-emerald-50/80 via-teal-50/50 to-emerald-50/30 p-3.5 text-xs text-emerald-950 dark:border-emerald-900/30 dark:from-emerald-950/30 dark:via-teal-950/20 dark:to-zinc-900 dark:text-emerald-200 shadow-xs">
    <div class="flex items-center gap-2.5">
      <CheckCircle2 size={17} class="shrink-0 text-emerald-600 dark:text-emerald-400" />
      <span>
        <strong>Desteklenen Dosya Formatları:</strong> Görsellerinizi veya metin belgelerinizi sıralayarak yüksek çözünürlüklü tek bir PDF belgesi oluşturun.
      </span>
    </div>
    <div class="flex flex-wrap items-center gap-1.5 font-bold pl-7">
      <span class="rounded-md bg-emerald-100 px-2 py-0.5 text-[11px] text-emerald-800 dark:bg-emerald-900/50 dark:text-emerald-300">PNG</span>
      <span class="rounded-md bg-emerald-100 px-2 py-0.5 text-[11px] text-emerald-800 dark:bg-emerald-900/50 dark:text-emerald-300">JPG</span>
      <span class="rounded-md bg-emerald-100 px-2 py-0.5 text-[11px] text-emerald-800 dark:bg-emerald-900/50 dark:text-emerald-300">WEBP</span>
      <span class="rounded-md bg-emerald-100 px-2 py-0.5 text-[11px] text-emerald-800 dark:bg-emerald-900/50 dark:text-emerald-300">BMP</span>
      <span class="rounded-md bg-emerald-100 px-2 py-0.5 text-[11px] text-emerald-800 dark:bg-emerald-900/50 dark:text-emerald-300">TIFF</span>
      <span class="rounded-md bg-teal-100 px-2 py-0.5 text-[11px] text-teal-800 dark:bg-teal-900/50 dark:text-teal-300">SVG</span>
      <span class="rounded-md bg-cyan-100 px-2 py-0.5 text-[11px] text-cyan-800 dark:bg-cyan-900/50 dark:text-cyan-300">TXT</span>
      <span class="rounded-md bg-cyan-100 px-2 py-0.5 text-[11px] text-cyan-800 dark:bg-cyan-900/50 dark:text-cyan-300">CSV</span>
    </div>
  </div>

  {#if images.length === 0}
    <FileDropzone
      extensions={ALL_ALLOWED_EXTS}
      multiple
      label="PDF yapılacak görselleri veya metinleri buraya sürükleyin ya da seçin"
      onFiles={validateAndAddFiles}
    />
  {:else}
    <!-- Toolbar with Multi-select Actions -->
    <div class="mb-4 flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-zinc-200 bg-white p-3.5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900 text-sm">
      <div class="flex flex-wrap items-center gap-2">
        <span class="text-xs font-bold text-zinc-800 dark:text-zinc-200 mr-1">
          Toplam {images.length} dosya eklendi
        </span>
        <button
          type="button"
          class="rounded-lg border border-zinc-300 bg-zinc-100 px-2.5 py-1 text-xs font-medium text-zinc-700 hover:bg-zinc-200 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 transition-colors"
          on:click={toggleSelectAllImages}
          disabled={busy}
        >
          {selectedImageIndices.length === images.length && images.length > 0 ? "Seçimi Kaldır" : "Tümünü Seç"}
        </button>
        <button
          type="button"
          disabled={selectedImageIndices.length === 0}
          class="flex items-center gap-1 rounded-lg px-2.5 py-1 text-xs font-semibold transition-all shadow-sm disabled:cursor-not-allowed disabled:opacity-40
            {selectedImageIndices.length > 1
              ? 'bg-blue-600 text-white hover:bg-blue-500 shadow-blue-500/25 ring-2 ring-blue-500/30'
              : 'border border-zinc-300 bg-zinc-100 text-zinc-700 hover:bg-zinc-200 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300'}"
          on:click={clearImageSelections}
        >
          <X size={13} strokeWidth={2.5} />
          Seçimleri İptal Et ({selectedImageIndices.length})
        </button>
        {#if selectedImageIndices.length > 0}
          <button
            type="button"
            class="flex items-center gap-1 rounded-lg bg-red-100 px-2.5 py-1 text-xs font-semibold text-red-700 hover:bg-red-200 dark:bg-red-950/50 dark:text-red-300 transition-colors"
            on:click={deleteSelectedImages}
          >
            <Trash2 size={13} />
            Seçilenleri Sil ({selectedImageIndices.length})
          </button>
        {/if}
        <button
          type="button"
          class="rounded-lg border border-zinc-300 bg-zinc-100 px-2.5 py-1 text-xs font-medium text-zinc-700 hover:bg-zinc-200 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 transition-colors"
          on:click={clearAllImages}
          disabled={busy}
        >
          Tümünü Temizle
        </button>
      </div>

      <div class="flex items-center gap-3">
        {#if busy}
          <button
            type="button"
            class="rounded-xl bg-rose-500 hover:bg-rose-600 px-4 py-2 text-xs font-bold text-white shadow transition-all active:scale-95"
            on:click={handleCancelConversion}
          >
            Vazgeç
          </button>
        {:else}
          <ActionButton
            label={`PDF Oluştur (${images.length} Sayfa)`}
            {busy}
            disabled={images.length === 0}
            onClick={convertToPdf}
          />
        {/if}
      </div>
    </div>

    <!-- Active Process Progress Bar -->
    {#if busy && progress >= 0}
      <div class="mb-4">
        <ProgressBar percent={progress} label={progressText || "PDF dosyası oluşturuluyor"} />
      </div>
    {/if}

    <!-- Add more dropzone -->
    <div class="mb-4">
      <FileDropzone
        extensions={ALL_ALLOWED_EXTS}
        multiple
        label="+ Daha Fazla Dosya Ekle (Sürükleyin veya tıklayın)"
        onFiles={validateAndAddFiles}
      />
    </div>


    <!-- Floating multi-item drag indicator following the mouse -->
    {#if isDraggingImg}
      <div
        class="fixed pointer-events-none z-50 flex items-center gap-2.5 rounded-xl bg-indigo-600/95 px-3.5 py-2 text-xs font-bold text-white shadow-2xl backdrop-blur-md border border-indigo-400/40 select-none animate-in fade-in zoom-in-95 duration-75"
        style="left: {pointerImgX + 16}px; top: {pointerImgY + 16}px;"
      >
        <span class="flex h-5 w-5 items-center justify-center rounded-full bg-white text-indigo-600 text-[11px] font-black shadow-xs">
          {activeDragImgIndices.length}
        </span>
        <span>
          {activeDragImgIndices.length > 1
            ? `${activeDragImgIndices.length} Dosya Taşınıyor`
            : `Dosya Taşınıyor`}
        </span>
      </div>
    {/if}

    <!-- Visual Thumbnail Grid with Pointer Drag & Reorder & Boundary Auto-Scroll -->
    <div
      bind:this={imgGridContainer}
      class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3.5 pb-6 select-none"
    >
      {#each images as img, i (img + i)}
        {@const isSelected = selectedImagesSet.has(i)}
        {@const isDragged = isDraggingImg && activeDragImgIndices.includes(i)}
        {@const isTarget = isDraggingImg && dragOverImgTargetIndex === i && !isDragged}
        <div
          data-img-index={i}
          class="group relative flex flex-col rounded-2xl border-2 bg-white p-2 shadow-sm transition-all dark:bg-zinc-900 cursor-pointer select-none
            {isSelected ? 'border-indigo-500 bg-indigo-500/5 ring-1 ring-indigo-500' : 'border-zinc-200 dark:border-zinc-800 hover:border-zinc-400'}
            {isDragged ? 'opacity-35 scale-95 ring-2 ring-indigo-400/40' : ''}
            {isTarget ? 'ring-4 ring-indigo-500 scale-105 shadow-2xl z-30' : ''}"
          role="button"
          tabindex="0"
          on:pointerdown={(e) => handleCardPointerDownImg(e, i)}
          on:click={() => {
            if (wasDraggingImg) return;
            toggleImageSelect(i);
          }}
          on:keydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              toggleImageSelect(i);
            }
          }}
        >

          <!-- Preview container -->
          <div
            class="relative flex h-36 w-full items-center justify-center overflow-hidden rounded-xl bg-zinc-100 dark:bg-zinc-800/60"
          >
            {#if img.endsWith(".txt") || img.endsWith(".csv")}
              <div class="flex flex-col items-center justify-center p-3 text-center text-zinc-500">
                <FileText size={32} class="mb-1 text-cyan-600" />
                <span class="text-[10px] font-bold uppercase">{img.split('.').pop()} Belgesi</span>
              </div>
            {:else}
              <ImageThumb path={img} alt={baseName(img)} />
            {/if}

            <!-- Top Overlay: Order Badge, Checkbox and Actions -->
            <div class="absolute inset-x-2 top-2 flex items-center justify-between pointer-events-none z-10">
              <button
                type="button"
                class="pointer-events-auto flex h-6 min-w-6 px-1 items-center justify-center rounded-md border-2 transition-all shadow-sm
                  {isSelected
                    ? 'border-indigo-600 bg-indigo-600 text-white font-bold scale-105 shadow-indigo-500/40'
                    : 'border-zinc-400 bg-white/95 text-transparent hover:border-indigo-500 hover:scale-105 dark:border-zinc-500 dark:bg-zinc-800/95'}"
                title={isSelected ? "Seçimi Kaldır" : "Dosyayı Seç"}
                on:click|stopPropagation={() => toggleImageSelect(i)}
              >
                {#if isSelected}
                  <span class="text-xs font-black leading-none text-white">{selectedImageIndices.indexOf(i) + 1}</span>
                {:else}
                  <span class="opacity-0 text-xs">0</span>
                {/if}
              </button>

              <div class="flex items-center gap-1 pointer-events-auto">
                <span class="rounded bg-zinc-900/70 px-1.5 py-0.5 text-[10px] font-bold text-white shadow-xs">
                  #{i + 1}
                </span>
                <button
                  type="button"
                  class="rounded-md bg-zinc-900/80 p-1 text-white opacity-80 hover:bg-indigo-600 hover:opacity-100 shadow transition-all"
                  title="Büyük Önizleme"
                  on:click|stopPropagation={() => (imageModalIndex = i)}
                >
                  <Maximize2 size={13} />
                </button>
                <button
                  type="button"
                  class="rounded-md bg-zinc-900/80 p-1 text-white opacity-80 hover:bg-red-600 hover:opacity-100 shadow transition-all"
                  title="Listeden Kaldır"
                  on:click|stopPropagation={() => removeImage(i)}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            </div>
          </div>

          <!-- Bottom bar: Reorder Buttons and Filename -->
          <div class="mt-2 flex items-center justify-between gap-1 px-1">
            <button
              type="button"
              class="flex h-6 w-6 items-center justify-center rounded text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-20 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800"
              title="Öne Taşı"
              disabled={i === 0}
              on:click|stopPropagation={() => moveImage(i, i - 1)}
            >
              <ChevronLeft size={14} />
            </button>

            <span class="flex-1 truncate text-center text-[11px] font-semibold text-zinc-700 dark:text-zinc-300" title={baseName(img)}>
              {baseName(img)}
            </span>

            <button
              type="button"
              class="flex h-6 w-6 items-center justify-center rounded text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-20 disabled:hover:bg-transparent dark:text-zinc-400 dark:hover:bg-zinc-800"
              title="Arkaya Taşı"
              disabled={i === images.length - 1}
              on:click|stopPropagation={() => moveImage(i, i + 1)}
            >
              <ChevronRight size={14} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
{/if}

<!-- Modals -->
{#if previewIndex !== null && pdfDoc}
  <PreviewModal
    page={items[previewIndex].page}
    pageCount={pdfDoc.pageCount}
    rotation={0}
    renderPage={pdfDoc.renderPage}
    onClose={() => (previewIndex = null)}
  />
{/if}

{#if imageModalIndex !== null && images[imageModalIndex]}
  <ImageViewerModal
    {images}
    currentIndex={imageModalIndex}
    onClose={() => (imageModalIndex = null)}
  />
{/if}
