<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { PdfDocHandle, PdfLink } from "../pdf";
  import type { Annotation } from "../docStore";
  import { FileText } from "@lucide/svelte";

  export let page: number;
  export let handle: PdfDocHandle;
  export let scale: number;
  export let isVisible: boolean = false;
  export let isCurrentPage: boolean = false;
  export let annotations: Annotation[] = [];
  export let onSelectPage: (p: number, e?: MouseEvent) => void;
  export let onLinkClick: (link: PdfLink) => void;
  export let onMouseDownPage: ((p: number, e: MouseEvent) => void) | undefined = undefined;
  export let onMouseMovePage: ((p: number, e: MouseEvent) => void) | undefined = undefined;

  let canvasEl: HTMLCanvasElement | null = null;
  let overlayEl: HTMLCanvasElement | null = null;
  let textLayerEl: HTMLDivElement | null = null;
  let pageDims = { width: 595, height: 842 }; // Standard A4 default points
  let renderedScale = 0;
  let isRendered = false;
  let lastHandle: PdfDocHandle | null = null;
  let links: PdfLink[] = [];

  async function updatePageDims() {
    if (!handle) return;
    try {
      const dims = await handle.getPageDimensions(page);
      if (dims && dims.width > 0 && dims.height > 0) {
        pageDims = dims;
      }
    } catch (e) {
      console.warn(`Dims error for page ${page}:`, e);
    }
  }

  onMount(() => {
    updatePageDims();
  });

  $: pagePixelWidth = Math.round(pageDims.width * scale);
  $: pagePixelHeight = Math.round(pageDims.height * scale);

  let lastCanvasEl: HTMLCanvasElement | null = null;

  $: if (handle !== lastHandle) {
    lastHandle = handle;
    isRendered = false;
    updatePageDims();
    if (isVisible) {
      tick().then(renderPageContent);
    }
  }

  $: if (!isVisible) {
    isRendered = false;
    renderedScale = 0;
  }

  $: if (isVisible && canvasEl && (renderedScale !== scale || !isRendered || canvasEl !== lastCanvasEl)) {
    lastCanvasEl = canvasEl;
    renderedScale = scale;
    tick().then(renderPageContent);
  }

  async function renderPageContent() {
    if (!canvasEl) return;
    try {
      const rendered = await handle.renderPage(page, scale);
      if (rendered && rendered.width > 0 && rendered.height > 0 && scale > 0) {
        const actualWidth = rendered.width / scale;
        const actualHeight = rendered.height / scale;
        if (Math.abs(pageDims.width - actualWidth) > 0.5 || Math.abs(pageDims.height - actualHeight) > 0.5) {
          pageDims = { width: actualWidth, height: actualHeight };
        }
      }
      canvasEl.width = rendered.width;
      canvasEl.height = rendered.height;
      canvasEl.style.width = `${rendered.width}px`;
      canvasEl.style.height = `${rendered.height}px`;
      const ctx = canvasEl.getContext("2d")!;
      ctx.drawImage(rendered, 0, 0);

      if (overlayEl) {
        overlayEl.width = rendered.width;
        overlayEl.height = rendered.height;
        overlayEl.style.width = `${rendered.width}px`;
        overlayEl.style.height = `${rendered.height}px`;
        if (!isCurrentPage) {
          drawAnnotations(overlayEl, annotations, scale);
        }
      }

      if (textLayerEl) {
        textLayerEl.style.width = `${rendered.width}px`;
        textLayerEl.style.height = `${rendered.height}px`;
        try {
          await handle.renderTextLayer(page, scale, textLayerEl);
        } catch {}
      }

      try {
        links = await handle.getPageLinks(page, scale);
      } catch {
        links = [];
      }

      isRendered = true;
    } catch (e) {
      console.error(`Error rendering continuous page ${page}:`, e);
    }
  }

  $: if (isRendered && overlayEl && annotations) {
    if (!isCurrentPage) {
      drawAnnotations(overlayEl, annotations, scale);
    } else {
      const ctx = overlayEl.getContext("2d");
      if (ctx) ctx.clearRect(0, 0, overlayEl.width, overlayEl.height);
    }
  }

  function drawAnnotations(canvas: HTMLCanvasElement, list: Annotation[], s: number) {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (!list || list.length === 0) return;

    for (const ann of list) {
      ctx.save();
      if (ann.opacity !== undefined) ctx.globalAlpha = ann.opacity;

      if (ann.type === "rect") {
        const x = (ann.x ?? 0) * s;
        const y = (ann.y ?? 0) * s;
        const w = (ann.width ?? 0) * s;
        const h = (ann.height ?? 0) * s;
        if (ann.fillColor && ann.fillColor !== "transparent") {
          ctx.fillStyle = ann.fillColor;
          ctx.fillRect(x, y, w, h);
        }
        if (ann.color && ann.color !== "transparent" && ann.strokeWidth) {
          ctx.strokeStyle = ann.color;
          ctx.lineWidth = Math.max(1, ann.strokeWidth * s);
          ctx.strokeRect(x, y, w, h);
        }
      } else if (ann.type === "circle") {
        const x = (ann.x ?? 0) * s;
        const y = (ann.y ?? 0) * s;
        const w = (ann.width ?? 0) * s;
        const h = (ann.height ?? 0) * s;
        ctx.beginPath();
        ctx.ellipse(x + w / 2, y + h / 2, Math.abs(w / 2), Math.abs(h / 2), 0, 0, Math.PI * 2);
        if (ann.fillColor && ann.fillColor !== "transparent") {
          ctx.fillStyle = ann.fillColor;
          ctx.fill();
        }
        if (ann.color && ann.color !== "transparent" && ann.strokeWidth) {
          ctx.strokeStyle = ann.color;
          ctx.lineWidth = Math.max(1, ann.strokeWidth * s);
          ctx.stroke();
        }
      } else if (ann.type === "line") {
        const x1 = (ann.startX ?? 0) * s;
        const y1 = (ann.startY ?? 0) * s;
        const x2 = (ann.endX ?? 0) * s;
        const y2 = (ann.endY ?? 0) * s;
        ctx.beginPath();
        ctx.moveTo(x1, y1);
        ctx.lineTo(x2, y2);
        ctx.strokeStyle = ann.color || "#000000";
        ctx.lineWidth = Math.max(1, (ann.strokeWidth || 2) * s);
        ctx.stroke();
      } else if (ann.type === "arrow") {
        const x1 = (ann.startX ?? 0) * s;
        const y1 = (ann.startY ?? 0) * s;
        const x2 = (ann.endX ?? 0) * s;
        const y2 = (ann.endY ?? 0) * s;
        const headLength = Math.max(10, 14 * s);
        const angle = Math.atan2(y2 - y1, x2 - x1);
        ctx.strokeStyle = ann.color || "#000000";
        ctx.fillStyle = ann.color || "#000000";
        ctx.lineWidth = Math.max(1, (ann.strokeWidth || 3) * s);
        ctx.beginPath();
        ctx.moveTo(x1, y1);
        ctx.lineTo(x2, y2);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(x2, y2);
        ctx.lineTo(x2 - headLength * Math.cos(angle - Math.PI / 6), y2 - headLength * Math.sin(angle - Math.PI / 6));
        ctx.lineTo(x2 - headLength * Math.cos(angle + Math.PI / 6), y2 - headLength * Math.sin(angle + Math.PI / 6));
        ctx.closePath();
        ctx.fill();
      } else if (ann.type === "stroke" || ann.type === "highlighter") {
        if (ann.points && ann.points.length > 1) {
          ctx.beginPath();
          ctx.moveTo(ann.points[0].x * s, ann.points[0].y * s);
          for (let i = 1; i < ann.points.length; i++) {
            ctx.lineTo(ann.points[i].x * s, ann.points[i].y * s);
          }
          ctx.strokeStyle = ann.color || (ann.type === "highlighter" ? "rgba(250,204,21,0.4)" : "#000");
          ctx.lineWidth = Math.max(1, (ann.strokeWidth || (ann.type === "highlighter" ? 18 : 3)) * s);
          ctx.lineCap = "round";
          ctx.lineJoin = "round";
          ctx.stroke();
        }
      } else if (ann.type === "text" && ann.text) {
        const x = (ann.x ?? 0) * s;
        const y = (ann.y ?? 0) * s;
        const fs = (ann.fontSize || 18) * s;
        ctx.font = `${fs}px sans-serif`;
        ctx.fillStyle = ann.color || "#000";
        ctx.textBaseline = "top";
        ctx.fillText(ann.text, x, y);
      } else if (ann.type === "redact") {
        const x = (ann.x ?? 0) * s;
        const y = (ann.y ?? 0) * s;
        const w = (ann.width ?? 0) * s;
        const h = (ann.height ?? 0) * s;
        ctx.fillStyle = "#000000";
        ctx.fillRect(x, y, w, h);
      } else if (ann.type === "image" && ann.imageEl) {
        const x = (ann.x ?? 0) * s;
        const y = (ann.y ?? 0) * s;
        const w = (ann.width ?? 40) * s;
        const h = (ann.height ?? 30) * s;
        try {
          ctx.drawImage(ann.imageEl, x, y, w, h);
        } catch {}
      } else if (ann.rects && ann.rects.length > 0) {
        if (ann.type === "underline") {
          ctx.strokeStyle = ann.color || "#dc2626";
          ctx.lineWidth = Math.max(1.5, (ann.strokeWidth || 2) * s);
          ctx.beginPath();
          for (const r of ann.rects) {
            const ly = (r.y + r.height - 1) * s;
            ctx.moveTo(r.x * s, ly);
            ctx.lineTo((r.x + r.width) * s, ly);
          }
          ctx.stroke();
        } else if (ann.type === "strike") {
          ctx.strokeStyle = ann.color || "#dc2626";
          ctx.lineWidth = Math.max(1.5, (ann.strokeWidth || 2) * s);
          ctx.beginPath();
          for (const r of ann.rects) {
            const ly = (r.y + r.height / 2) * s;
            ctx.moveTo(r.x * s, ly);
            ctx.lineTo((r.x + r.width) * s, ly);
          }
          ctx.stroke();
        } else if (ann.type === "highlighter") {
          ctx.fillStyle = ann.fillColor || ann.color || "rgba(250, 204, 21, 0.45)";
          for (const r of ann.rects) {
            ctx.fillRect(r.x * s, r.y * s, r.width * s, r.height * s);
          }
        }
      }
      ctx.restore();
    }
  }
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  id="continuous-page-{page}"
  data-page={page}
  class="relative mb-3 flex flex-col items-center transition-all w-fit rounded-xl p-2 {isCurrentPage ? 'ring-2 ring-indigo-500/80 bg-indigo-50/25 dark:bg-indigo-950/25 shadow-md' : 'hover:ring-1 hover:ring-indigo-400/40 cursor-pointer'}"
  style="width: {pagePixelWidth + 16}px; min-width: {pagePixelWidth + 16}px;"
  on:mousedown={(e) => {
    if (e.button === 1) return;
    if (onMouseDownPage) {
      onMouseDownPage(page, e);
    } else if (!isCurrentPage) {
      onSelectPage(page, e);
    }
  }}
  on:mousemove={(e) => {
    onMouseMovePage?.(page, e);
  }}
>
  <!-- Page Header Tag -->
  <div class="mb-1 flex items-center justify-between w-full px-1 text-[10px] text-zinc-400 dark:text-zinc-500 select-none">
    <span class="inline-flex items-center gap-1 font-semibold {isCurrentPage ? 'text-indigo-600 dark:text-indigo-400' : ''}">
      <FileText size={11} /> Sayfa {page}
    </span>
    {#if annotations.length > 0}
      <span class="text-[9px] bg-indigo-100 text-indigo-700 dark:bg-indigo-900/60 dark:text-indigo-300 px-1.5 py-0.2 rounded-full">
        {annotations.length} düzenleme
      </span>
    {/if}
  </div>

  <!-- Page Viewport Container -->
  <div
    id="continuous-page-viewport-{page}"
    class="relative rounded bg-white shadow-xl overflow-hidden"
    style="width: {pagePixelWidth}px; height: {pagePixelHeight}px; min-width: {pagePixelWidth}px; min-height: {pagePixelHeight}px;"
  >
    {#if isVisible}
      <canvas bind:this={canvasEl} class="block rounded bg-white"></canvas>

      <!-- Text Layer -->
      <div bind:this={textLayerEl} class="textLayer absolute inset-0 select-text"></div>

      <!-- Clickable Links -->
      <div class="absolute inset-0 pointer-events-none">
        {#each links as link}
          <button
            type="button"
            class="absolute pointer-events-auto cursor-pointer rounded-xs bg-indigo-500/10 hover:bg-indigo-500/25 transition-colors border border-transparent hover:border-indigo-500/40"
            style="left: {link.rect[0]}px; top: {link.rect[1]}px; width: {link.rect[2] - link.rect[0]}px; height: {link.rect[3] - link.rect[1]}px;"
            title={link.url ? link.url : link.destPage ? `Sayfa ${link.destPage}'e git` : 'Bağlantı'}
            on:click|stopPropagation={() => onLinkClick(link)}
          ></button>
        {/each}
      </div>

      <!-- Overlay Canvas for passive annotations when not active -->
      <canvas
        bind:this={overlayEl}
        class="absolute inset-0 pointer-events-none {isCurrentPage ? 'hidden' : ''}"
      ></canvas>

      <!-- Interactive slot for active page -->
      {#if isCurrentPage}
        <slot {pagePixelWidth} {pagePixelHeight} />
      {/if}
    {:else}
      <!-- Virtual Windowing Skeleton Placeholder -->
      <div class="flex h-full w-full flex-col items-center justify-center bg-zinc-50 text-zinc-300 dark:bg-zinc-900 dark:text-zinc-700 select-none">
        <FileText size={32} class="animate-pulse" />
        <span class="mt-2 text-xs font-medium">Sayfa {page} yükleniyor...</span>
      </div>
    {/if}
  </div>
</div>
