<script lang="ts">
  import { onMount } from "svelte";
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
  export let onFieldValueChange: ((fieldName: string, value: string) => void) | undefined = undefined;
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

  $: pagePixelWidth = Math.round(pageDims.width * scale);
  $: pagePixelHeight = Math.round(pageDims.height * scale);

  $: if (doc && currentPage) {
    loadPage();
  }

  $: if (scale && doc && currentPage && canvas) {
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

  // --- Checkbox & Radio State Handlers ---
  function isFieldChecked(name: string, defaultExportVal: string = "On"): boolean {
    const val = formValues[name];
    if (val === undefined) {
      const initial = (pageFields.find((f) => f.name === name)?.value || "").trim();
      return (
        initial === "On" ||
        initial === "Yes" ||
        initial === "1" ||
        initial === "true" ||
        initial === defaultExportVal
      );
    }
    const norm = (val || "").trim().toLowerCase();
    if (norm === "off" || norm === "false" || norm === "0" || norm === "" || norm === "/") {
      return false;
    }
    return true;
  }

  function toggleCheckboxField(f: PdfFormField) {
    const checked = isFieldChecked(f.name, f.buttonValue || "On");
    const onVal = f.buttonValue || "On";
    const newVal = checked ? "Off" : onVal;
    formValues[f.name] = newVal;
    formValues = { ...formValues };

    if (onFieldValueChange) {
      onFieldValueChange(f.name, newVal);
    }
    if (onFieldClick) {
      onFieldClick(f.name);
    }
  }

  function isRadioActive(f: PdfFormField): boolean {
    const current = (formValues[f.name] !== undefined ? formValues[f.name] : f.value || "")
      .trim()
      .toLowerCase();
    const btnVal = (f.buttonValue || "").trim().toLowerCase();
    return Boolean(btnVal && current === btnVal);
  }

  function selectRadioField(f: PdfFormField) {
    const newVal = f.buttonValue || "On";
    formValues[f.name] = newVal;
    formValues = { ...formValues };

    if (onFieldValueChange) {
      onFieldValueChange(f.name, newVal);
    }
    if (onFieldClick) {
      onFieldClick(f.name);
    }
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
        style="width: {pagePixelWidth}px; height: {pagePixelHeight}px;"
        on:click={handleCanvasClick}
      >
        <canvas
          bind:this={canvas}
          class="block"
          style="width: {pagePixelWidth}px; height: {pagePixelHeight}px;"
        ></canvas>

        {#if loading}
          <div class="absolute inset-0 flex items-center justify-center bg-white/50 backdrop-blur-xs dark:bg-zinc-900/50 z-30">
            <span class="text-xs font-semibold text-indigo-600 dark:text-indigo-400 animate-pulse">Sayfa Yükleniyor...</span>
          </div>
        {/if}

        <!-- Interactive Form Fields Overlay: Scaled linearly via rectPt -->
        {#if mode === "forms" && pageFields.length > 0}
          <div class="absolute inset-0 pointer-events-none z-10">
            {#each pageFields as f}
              {#if f.type === "checkbox"}
                <!-- Checkbox: Live, Clickable with Tick Mark -->
                <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
                <div
                  class="absolute pointer-events-auto cursor-pointer rounded transition-all flex items-center justify-center {isFieldChecked(f.name, f.buttonValue || 'On') ? 'bg-emerald-500/25 border-2 border-emerald-600 shadow-2xs' : 'bg-white/50 hover:bg-white/80 border border-zinc-400 hover:border-indigo-600'} {highlightFieldName === f.name ? 'ring-2 ring-indigo-500 scale-105' : ''}"
                  style="
                    left: {f.rectPt[0] * scale}px;
                    top: {f.rectPt[1] * scale}px;
                    width: {Math.max(14, (f.rectPt[2] - f.rectPt[0]) * scale)}px;
                    height: {Math.max(14, (f.rectPt[3] - f.rectPt[1]) * scale)}px;
                  "
                  title="{f.name}: {isFieldChecked(f.name, f.buttonValue || 'On') ? 'İşaretli (Kaldırmak için tıkla)' : 'İşaretsiz (İşaretlemek için tıkla)'}"
                  on:click={(e) => {
                    e.stopPropagation();
                    toggleCheckboxField(f);
                  }}
                >
                  {#if isFieldChecked(f.name, f.buttonValue || 'On')}
                    <Check
                      size={Math.max(11, (f.rectPt[3] - f.rectPt[1]) * scale * 1.15)}
                      class="text-emerald-700 dark:text-emerald-300 stroke-[3.5] drop-shadow-2xs"
                    />
                  {/if}
                </div>
              {:else if f.type === "radio"}
                <!-- Radio Option: Live, Clickable with Active Dot -->
                <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
                <div
                  class="absolute pointer-events-auto cursor-pointer rounded-full transition-all flex items-center justify-center {isRadioActive(f) ? 'bg-purple-500/25 border-2 border-purple-600 shadow-2xs' : 'bg-white/50 hover:bg-white/80 border border-zinc-400 hover:border-purple-600'} {highlightFieldName === f.name ? 'ring-2 ring-purple-500 scale-105' : ''}"
                  style="
                    left: {f.rectPt[0] * scale}px;
                    top: {f.rectPt[1] * scale}px;
                    width: {Math.max(14, (f.rectPt[2] - f.rectPt[0]) * scale)}px;
                    height: {Math.max(14, (f.rectPt[3] - f.rectPt[1]) * scale)}px;
                  "
                  title="{f.name} ({f.buttonValue || 'Seçenek'})"
                  on:click={(e) => {
                    e.stopPropagation();
                    selectRadioField(f);
                  }}
                >
                  {#if isRadioActive(f)}
                    <div
                      class="rounded-full bg-purple-700 dark:bg-purple-300 drop-shadow-2xs"
                      style="
                        width: {Math.max(6, (f.rectPt[2] - f.rectPt[0]) * scale * 0.55)}px;
                        height: {Math.max(6, (f.rectPt[3] - f.rectPt[1]) * scale * 0.55)}px;
                      "
                    ></div>
                  {/if}
                </div>
              {:else if f.type === "text" || f.type === "multiline"}
                <!-- Text Field: Live, Editable and Bi-directionally Synced -->
                <input
                  type="text"
                  class="absolute pointer-events-auto rounded bg-blue-50/45 hover:bg-white/95 focus:bg-white px-1 text-zinc-900 border transition-all {highlightFieldName === f.name ? 'border-indigo-600 ring-2 ring-indigo-500/40 bg-white font-medium' : 'border-blue-400/40 hover:border-indigo-400 focus:border-indigo-600'} focus:outline-none shadow-2xs"
                  style="
                    left: {f.rectPt[0] * scale}px;
                    top: {f.rectPt[1] * scale}px;
                    width: {(f.rectPt[2] - f.rectPt[0]) * scale}px;
                    height: {(f.rectPt[3] - f.rectPt[1]) * scale}px;
                    font-size: {Math.max(8, Math.min(18, (f.rectPt[3] - f.rectPt[1]) * scale * 0.72))}px;
                    line-height: 1;
                  "
                  value={formValues[f.name] !== undefined ? formValues[f.name] : f.value}
                  placeholder={f.name}
                  disabled={f.readOnly}
                  on:focus={() => onFieldClick?.(f.name)}
                  on:input={(e) => {
                    const val = e.currentTarget.value;
                    formValues[f.name] = val;
                    onFieldValueChange?.(f.name, val);
                  }}
                />
              {:else if f.type === "choice" && f.options && f.options.length > 0}
                <!-- Dropdown / Combo Choice -->
                <select
                  class="absolute pointer-events-auto rounded bg-blue-50/60 hover:bg-white text-zinc-900 border border-blue-400/50 focus:border-indigo-600 text-xs px-1"
                  style="
                    left: {f.rectPt[0] * scale}px;
                    top: {f.rectPt[1] * scale}px;
                    width: {(f.rectPt[2] - f.rectPt[0]) * scale}px;
                    height: {(f.rectPt[3] - f.rectPt[1]) * scale}px;
                    font-size: {Math.max(8, Math.min(14, (f.rectPt[3] - f.rectPt[1]) * scale * 0.7))}px;
                  "
                  value={formValues[f.name] !== undefined ? formValues[f.name] : f.value}
                  disabled={f.readOnly}
                  on:focus={() => onFieldClick?.(f.name)}
                  on:change={(e) => {
                    const val = e.currentTarget.value;
                    formValues[f.name] = val;
                    onFieldValueChange?.(f.name, val);
                  }}
                >
                  <option value="">--</option>
                  {#each f.options as opt}
                    <option value={opt}>{opt}</option>
                  {/each}
                </select>
              {:else if f.type === "pushbutton"}
                <!-- Pushbutton -->
                <div
                  class="absolute pointer-events-auto rounded bg-zinc-200/40 border border-zinc-400/50 text-[10px] text-zinc-600 font-semibold flex items-center justify-center select-none"
                  style="
                    left: {f.rectPt[0] * scale}px;
                    top: {f.rectPt[1] * scale}px;
                    width: {(f.rectPt[2] - f.rectPt[0]) * scale}px;
                    height: {(f.rectPt[3] - f.rectPt[1]) * scale}px;
                  "
                >
                  {f.name}
                </div>
              {/if}
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
