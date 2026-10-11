<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    ChevronLeft,
    ChevronRight,
    ZoomIn,
    ZoomOut,
    Maximize2,
    PenLine,
    Move,
    Check,
    Layers,
  } from "@lucide/svelte";
  import type { PdfDocHandle, PdfFormField } from "../pdf";

  export let doc: PdfDocHandle | null = null;
  export let currentPage: number = 1;
  export let mode: "default" | "forms" | "signature" = "forms";
  export let highlightFieldName: string | null = null;
  export let sigPage: number = 1;
  export let sigX: number = 400;
  export let sigY: number = 50;
  export let sigW: number = 150;
  export let sigH: number = 60;
  export let sigImage: string | null = null;
  export let onSigChange:
    | ((pos: { x: number; y: number; width: number; height: number; page: number }) => void)
    | undefined = undefined;
  export let onFieldClick: ((fieldName: string) => void) | undefined = undefined;
  export let formValues: Record<string, string> = {};

  let canvasContainer: HTMLDivElement | null = null;
  let canvas: HTMLCanvasElement | null = null;
  let loading = false;
  let scale = 1.0;
  let autoFitScale = 1.0;
  let pageDims = { width: 595, height: 842 };
  let pageFields: PdfFormField[] = [];

  // Dragging signature stamp
  let isDraggingSig = false;
  let dragOffset = { x: 0, y: 0 };

  $: if (doc && currentPage) {
    loadPage();
  }

  $: if (scale && doc && currentPage) {
    renderCanvas();
  }

  async function loadPage() {
    if (!doc) return;
    if (currentPage < 1) currentPage = 1;
    if (currentPage > doc.pageCount) currentPage = doc.pageCount;

    try {
      const dims = await doc.getPageDimensions(currentPage);
      if (dims && dims.width > 0 && dims.height > 0) {
        pageDims = dims;
      }
    } catch (e) {
      console.warn("Failed to get page dimensions:", e);
    }

    computeAutoFit();
    await renderCanvas();
    await loadFields();
  }

  function computeAutoFit() {
    if (!canvasContainer || pageDims.width <= 0) return;
    const containerW = canvasContainer.clientWidth - 40;
    const containerH = canvasContainer.clientHeight - 60;
    if (containerW > 0 && containerH > 0) {
      const scaleW = containerW / pageDims.width;
      const scaleH = containerH / pageDims.height;
      autoFitScale = Math.min(scaleW, scaleH, 1.6);
      if (scale === 1.0 || scale === 0) {
        scale = Math.max(0.4, Math.round(autoFitScale * 100) / 100);
      }
    }
  }

  async function renderCanvas() {
    if (!doc || !canvas || loading) return;
    loading = true;
    try {
      const effectiveScale = Math.max(0.2, Math.min(3.0, scale));
      const rendered = await doc.renderPage(currentPage, effectiveScale);
      canvas.width = rendered.width;
      canvas.height = rendered.height;
      const ctx = canvas.getContext("2d");
      if (ctx) {
        ctx.drawImage(rendered, 0, 0);
      }
    } catch (e) {
      console.warn("Failed to render preview page:", e);
    } finally {
      loading = false;
    }
  }

  async function loadFields() {
    if (!doc || mode !== "forms") {
      pageFields = [];
      return;
    }
    try {
      pageFields = await doc.getPageFormFields(currentPage, scale);
    } catch {
      pageFields = [];
    }
  }

  function prevPage() {
    if (currentPage > 1) {
      currentPage -= 1;
    }
  }

  function nextPage() {
    if (doc && currentPage < doc.pageCount) {
      currentPage += 1;
    }
  }

  function zoomIn() {
    scale = Math.min(3.0, Math.round((scale + 0.15) * 100) / 100);
  }

  function zoomOut() {
    scale = Math.max(0.3, Math.round((scale - 0.15) * 100) / 100);
  }

  function resetZoom() {
    computeAutoFit();
    scale = Math.max(0.4, Math.round(autoFitScale * 100) / 100);
  }

  // --- Signature Box Math ---
  $: sigBoxWidth = Math.round(sigW * scale);
  $: sigBoxHeight = Math.round(sigH * scale);
  $: sigBoxLeft = Math.round(sigX * scale);
  $: sigBoxTop = Math.round((pageDims.height - (sigY + sigH)) * scale);

  function handleCanvasClick(e: MouseEvent) {
    if (mode !== "signature" || isDraggingSig || !canvas) return;
    const rect = canvas.getBoundingClientRect();
    const clickX = e.clientX - rect.left;
    const clickY = e.clientY - rect.top;

    // Center signature at click point
    const newBoxLeft = Math.max(0, Math.min(canvas.width - sigBoxWidth, clickX - sigBoxWidth / 2));
    const newBoxTop = Math.max(0, Math.min(canvas.height - sigBoxHeight, clickY - sigBoxHeight / 2));

    const newX = Math.round(newBoxLeft / scale);
    const newY = Math.round(pageDims.height - newBoxTop / scale - sigH);

    sigX = Math.max(0, newX);
    sigY = Math.max(0, newY);
    sigPage = currentPage;

    if (onSigChange) {
      onSigChange({ x: sigX, y: sigY, width: sigW, height: sigH, page: sigPage });
    }
  }

  function startSigDrag(e: MouseEvent) {
    if (mode !== "signature" || !canvas) return;
    e.preventDefault();
    e.stopPropagation();
    isDraggingSig = true;
    const rect = canvas.getBoundingClientRect();
    dragOffset = {
      x: e.clientX - rect.left - sigBoxLeft,
      y: e.clientY - rect.top - sigBoxTop,
    };

    window.addEventListener("mousemove", onSigDrag);
    window.addEventListener("mouseup", stopSigDrag);
  }

  function onSigDrag(e: MouseEvent) {
    if (!isDraggingSig || !canvas) return;
    const rect = canvas.getBoundingClientRect();
    const currX = e.clientX - rect.left - dragOffset.x;
    const currY = e.clientY - rect.top - dragOffset.y;

    const maxLeft = Math.max(0, canvas.width - sigBoxWidth);
    const maxTop = Math.max(0, canvas.height - sigBoxHeight);

    const clampedLeft = Math.max(0, Math.min(maxLeft, currX));
    const clampedTop = Math.max(0, Math.min(maxTop, currY));

    sigX = Math.max(0, Math.round(clampedLeft / scale));
    sigY = Math.max(0, Math.round(pageDims.height - clampedTop / scale - sigH));
    sigPage = currentPage;

    if (onSigChange) {
      onSigChange({ x: sigX, y: sigY, width: sigW, height: sigH, page: sigPage });
    }
  }

  function stopSigDrag() {
    isDraggingSig = false;
    window.removeEventListener("mousemove", onSigDrag);
    window.removeEventListener("mouseup", stopSigDrag);
  }

  onMount(() => {
    computeAutoFit();
    window.addEventListener("resize", computeAutoFit);
    return () => {
      window.removeEventListener("resize", computeAutoFit);
      window.removeEventListener("mousemove", onSigDrag);
      window.removeEventListener("mouseup", stopSigDrag);
    };
  });
</script>

<div class="flex h-full flex-col rounded-2xl border border-zinc-200 bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-900/60 overflow-hidden shadow-xs">
  <!-- Toolbar -->
  <div class="flex items-center justify-between border-b border-zinc-200/80 bg-white/80 px-3 py-2 backdrop-blur dark:border-zinc-800 dark:bg-zinc-900/80">
    <!-- Page Navigation -->
    <div class="flex items-center gap-1.5">
      <button
        type="button"
        class="flex h-7 w-7 items-center justify-center rounded-lg border border-zinc-200 text-zinc-600 transition-colors hover:bg-zinc-100 disabled:opacity-30 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer"
        disabled={currentPage <= 1 || !doc}
        on:click={prevPage}
        title="Önceki Sayfa"
      >
        <ChevronLeft size={16} />
      </button>

      <span class="px-1 text-xs font-semibold text-zinc-700 dark:text-zinc-200">
        Sayfa {currentPage} / {doc ? doc.pageCount : 1}
      </span>

      <button
        type="button"
        class="flex h-7 w-7 items-center justify-center rounded-lg border border-zinc-200 text-zinc-600 transition-colors hover:bg-zinc-100 disabled:opacity-30 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800 cursor-pointer"
        disabled={!doc || currentPage >= doc.pageCount}
        on:click={nextPage}
        title="Sonraki Sayfa"
      >
        <ChevronRight size={16} />
      </button>
    </div>

    <!-- Mode Badge & Zoom Controls -->
    <div class="flex items-center gap-2">
      {#if mode === "signature"}
        <span class="hidden sm:inline-flex items-center gap-1 rounded-md bg-purple-50 px-2 py-0.5 text-[11px] font-semibold text-purple-700 dark:bg-purple-950/60 dark:text-purple-300">
          <PenLine size={12} />
          İmza Konumu
        </span>
      {:else if mode === "forms"}
        <span class="hidden sm:inline-flex items-center gap-1 rounded-md bg-indigo-50 px-2 py-0.5 text-[11px] font-semibold text-indigo-700 dark:bg-indigo-950/60 dark:text-indigo-300">
          <Layers size={12} />
          {pageFields.length} Alan
        </span>
      {/if}

      <div class="flex items-center gap-1">
        <button
          type="button"
          class="flex h-7 w-7 items-center justify-center rounded-lg text-zinc-500 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800 cursor-pointer"
          on:click={zoomOut}
          title="Uzaklaştır"
        >
          <ZoomOut size={14} />
        </button>
        <span class="w-10 text-center text-xs font-medium text-zinc-500 dark:text-zinc-400 select-none">
          {Math.round(scale * 100)}%
        </span>
        <button
          type="button"
          class="flex h-7 w-7 items-center justify-center rounded-lg text-zinc-500 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800 cursor-pointer"
          on:click={zoomIn}
          title="Yakınlaştır"
        >
          <ZoomIn size={14} />
        </button>
        <button
          type="button"
          class="flex h-7 w-7 items-center justify-center rounded-lg text-zinc-500 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800 cursor-pointer"
          on:click={resetZoom}
          title="Ekrana Sığdır"
        >
          <Maximize2 size={13} />
        </button>
      </div>
    </div>
  </div>

  <!-- Canvas Container with scrollable preview area -->
  <div
    bind:this={canvasContainer}
    class="relative flex-1 overflow-auto p-4 flex items-center justify-center min-h-[350px]"
  >
    {#if !doc}
      <div class="flex flex-col items-center justify-center gap-2 text-zinc-400">
        <Layers size={36} class="opacity-40" />
        <span class="text-xs">Önizlenecek PDF yok</span>
      </div>
    {:else}
      <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
      <div
        class="relative shadow-md rounded-lg overflow-hidden bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 transition-all select-none"
        style="width: {canvas ? canvas.width : Math.round(pageDims.width * scale)}px; height: {canvas ? canvas.height : Math.round(pageDims.height * scale)}px;"
        on:click={handleCanvasClick}
      >
        <canvas bind:this={canvas} class="block"></canvas>

        {#if loading}
          <div class="absolute inset-0 flex items-center justify-center bg-white/50 backdrop-blur-xs dark:bg-zinc-900/50">
            <span class="text-xs font-semibold text-indigo-600 dark:text-indigo-400 animate-pulse">Sayfa Yükleniyor...</span>
          </div>
        {/if}

        <!-- Interactive Form Fields Overlay -->
        {#if mode === "forms" && pageFields.length > 0}
          <div class="absolute inset-0 pointer-events-none">
            {#each pageFields as f}
              <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
              <div
                class="absolute pointer-events-auto cursor-pointer rounded transition-all flex items-center justify-center text-[10px] {highlightFieldName === f.name ? 'ring-2 ring-indigo-500 bg-indigo-500/25 border-indigo-600' : 'border border-blue-400/40 bg-blue-500/10 hover:bg-blue-500/20'}"
                style="left: {f.rect[0]}px; top: {f.rect[1]}px; width: {f.rect[2] - f.rect[0]}px; height: {f.rect[3] - f.rect[1]}px;"
                title="{f.name} ({f.type})"
                on:click={() => onFieldClick?.(f.name)}
              >
                {#if f.type === "checkbox"}
                  {#if formValues[f.name] === "Yes" || formValues[f.name] === "On" || (!formValues[f.name] && (f.value === "Yes" || f.value === "On"))}
                    <Check size={Math.min(14, (f.rect[3] - f.rect[1]) * 0.9)} class="text-indigo-600 dark:text-indigo-400 stroke-[3]" />
                  {/if}
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        <!-- Interactive Signature Stamp Box -->
        {#if mode === "signature" && currentPage === sigPage}
          <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
          <div
            class="absolute pointer-events-auto cursor-move rounded-lg border-2 border-dashed border-purple-500 bg-purple-500/20 shadow-lg backdrop-blur-xs flex flex-col items-center justify-center p-1 select-none transition-shadow {isDraggingSig ? 'ring-2 ring-purple-600 scale-[1.02]' : 'hover:border-purple-600'}"
            style="left: {sigBoxLeft}px; top: {sigBoxTop}px; width: {sigBoxWidth}px; height: {sigBoxHeight}px; z-index: 30;"
            on:mousedown={startSigDrag}
            title="İmza Konumu (Sürükleyin veya sayfada tıklayın)"
          >
            {#if sigImage}
              <img
                src={sigImage}
                alt="İmza"
                class="pointer-events-none max-h-full max-w-full object-contain filter drop-shadow-xs"
              />
            {:else}
              <div class="pointer-events-none flex flex-col items-center gap-0.5 text-purple-700 dark:text-purple-300">
                <Move size={14} class="opacity-70 animate-bounce" />
                <span class="text-[10px] font-bold tracking-tight">İmza Alanı</span>
                <span class="text-[9px] opacity-75">Sürükleyin</span>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>
