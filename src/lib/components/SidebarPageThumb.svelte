<script lang="ts">
  import type { PdfDocHandle } from "../pdf";
  import type { Annotation } from "../docStore";

  export let page: number;
  export let handle: PdfDocHandle;
  export let docVersion: number = 0;
  export let pageVersion: number = 0;
  export let annotations: Annotation[] = [];

  let canvas: HTMLCanvasElement;
  let loading = true;
  let error = false;
  let renderedKey = "";

  function drawThumbItem(ctx: CanvasRenderingContext2D, item: Annotation) {
    ctx.save();
    if (item.blendMode === "destination-out" && item.points) {
      ctx.globalCompositeOperation = "destination-out";
      ctx.strokeStyle = "rgba(0,0,0,1)";
      ctx.lineWidth = item.strokeWidth || 20;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.beginPath();
      ctx.moveTo(item.points[0].x, item.points[0].y);
      for (let i = 1; i < item.points.length; i++) {
        ctx.lineTo(item.points[i].x, item.points[i].y);
      }
      ctx.stroke();
    } else if (item.type === "stroke" && item.points) {
      ctx.strokeStyle = item.color || "#000";
      ctx.lineWidth = item.strokeWidth || 3;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.beginPath();
      ctx.moveTo(item.points[0].x, item.points[0].y);
      for (let i = 1; i < item.points.length; i++) {
        ctx.lineTo(item.points[i].x, item.points[i].y);
      }
      ctx.stroke();
    } else if (item.type === "highlighter" && item.points) {
      ctx.strokeStyle = item.color || "#eab308";
      ctx.lineWidth = 18;
      ctx.globalAlpha = 0.45;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.beginPath();
      ctx.moveTo(item.points[0].x, item.points[0].y);
      for (let i = 1; i < item.points.length; i++) {
        ctx.lineTo(item.points[i].x, item.points[i].y);
      }
      ctx.stroke();
    } else if (item.type === "text" && item.text && item.x !== undefined && item.y !== undefined) {
      ctx.font = `${item.fontSize || 18}px sans-serif`;
      ctx.fillStyle = item.color || "#9333ea";
      ctx.textBaseline = "top";
      ctx.fillText(item.text, item.x, item.y);
    } else if (item.type === "rect" && item.x !== undefined && item.y !== undefined) {
      if (item.opacity !== undefined) ctx.globalAlpha = item.opacity;
      if (item.fillColor && item.fillColor !== "transparent") {
        ctx.fillStyle = item.fillColor;
        ctx.fillRect(item.x, item.y, item.width || 0, item.height || 0);
      }
      ctx.strokeStyle = item.color || "#000";
      ctx.lineWidth = item.strokeWidth || 3;
      ctx.strokeRect(item.x, item.y, item.width || 0, item.height || 0);
    } else if (item.type === "redact" && item.x !== undefined && item.y !== undefined) {
      ctx.fillStyle = item.fillColor || "#000000";
      ctx.fillRect(item.x, item.y, item.width || 0, item.height || 0);
    } else if (item.type === "circle" && item.x !== undefined && item.y !== undefined) {
      if (item.opacity !== undefined) ctx.globalAlpha = item.opacity;
      const rx = (item.width || 0) / 2;
      const ry = (item.height || 0) / 2;
      ctx.beginPath();
      ctx.ellipse(item.x + rx, item.y + ry, rx, ry, 0, 0, Math.PI * 2);
      if (item.fillColor && item.fillColor !== "transparent") {
        ctx.fillStyle = item.fillColor;
        ctx.fill();
      }
      ctx.strokeStyle = item.color || "#000";
      ctx.lineWidth = item.strokeWidth || 3;
      ctx.stroke();
    } else if (item.type === "line" && item.startX !== undefined && item.startY !== undefined && item.endX !== undefined && item.endY !== undefined) {
      if (item.opacity !== undefined) ctx.globalAlpha = item.opacity;
      ctx.strokeStyle = item.color || "#000";
      ctx.lineWidth = item.strokeWidth || 3;
      ctx.beginPath();
      ctx.moveTo(item.startX, item.startY);
      ctx.lineTo(item.endX, item.endY);
      ctx.stroke();
    } else if (item.type === "arrow" && item.startX !== undefined && item.startY !== undefined && item.endX !== undefined && item.endY !== undefined) {
      const dist = Math.hypot(item.endX - item.startX, item.endY - item.startY);
      if (dist >= 2) {
        ctx.save();
        if (item.opacity !== undefined) ctx.globalAlpha = item.opacity;
        const angle = Math.atan2(item.endY - item.startY, item.endX - item.startX);
        const w = Math.max(1, item.strokeWidth || 3);
        const headLen = Math.min(dist * 0.75, Math.max(15, w * 3.8));
        const headAngle = Math.PI / 6;
        const shaftEndX = item.endX - headLen * 0.75 * Math.cos(angle);
        const shaftEndY = item.endY - headLen * 0.75 * Math.sin(angle);

        ctx.strokeStyle = item.color || "#000";
        ctx.lineWidth = w;
        ctx.lineCap = "butt";
        ctx.beginPath();
        ctx.moveTo(item.startX, item.startY);
        ctx.lineTo(shaftEndX, shaftEndY);
        ctx.stroke();

        ctx.fillStyle = item.color || "#000";
        ctx.beginPath();
        ctx.moveTo(item.endX, item.endY);
        ctx.lineTo(item.endX - headLen * Math.cos(angle - headAngle), item.endY - headLen * Math.sin(angle - headAngle));
        ctx.lineTo(item.endX - headLen * Math.cos(angle + headAngle), item.endY - headLen * Math.sin(angle + headAngle));
        ctx.closePath();
        ctx.fill();
        ctx.restore();
      }
    } else if (item.type === "image" && item.x !== undefined && item.y !== undefined) {
      if (item.imageEl instanceof HTMLImageElement && item.imageEl.complete && item.imageEl.naturalWidth > 0) {
        ctx.drawImage(item.imageEl, item.x, item.y, item.width || 120, item.height || 60);
      } else if (item.imageData && typeof item.imageData === "string") {
        const img = new Image();
        img.onload = () => {
          item.imageEl = img;
          loadThumb();
        };
        img.src = item.imageData;
      }
    }
    ctx.restore();
  }

  async function loadThumb() {
    if (!handle || !page || !canvas) return;
    const currentKey = `${handle.path}_${page}_${docVersion}_${pageVersion}`;
    if (renderedKey === currentKey) return;
    renderedKey = currentKey;

    loading = true;
    error = false;
    try {
      const thumbScale = 0.28;
      const rendered = await handle.renderPage(page, thumbScale);
      if (canvas) {
        canvas.width = rendered.width;
        canvas.height = rendered.height;
        const ctx = canvas.getContext("2d");
        if (ctx) {
          ctx.drawImage(rendered, 0, 0);
          if (annotations && annotations.length > 0) {
            ctx.save();
            ctx.scale(thumbScale, thumbScale);
            for (const item of annotations) {
              drawThumbItem(ctx, item);
            }
            ctx.restore();
          }
        }
      }
    } catch (e) {
      console.warn("Sidebar thumbnail error for page", page, e);
      error = true;
    } finally {
      loading = false;
    }
  }

  $: if (handle && page && canvas && docVersion !== undefined && pageVersion !== undefined) {
    loadThumb();
  }
</script>

<div class="relative flex w-full items-center justify-center overflow-hidden rounded bg-white shadow-xs dark:bg-zinc-800 min-h-[75px] border border-zinc-200/50 dark:border-zinc-700/50">
  {#if loading}
    <div class="flex h-20 w-full animate-pulse items-center justify-center bg-zinc-100 text-[11px] font-bold text-zinc-400 dark:bg-zinc-800">
      {page}
    </div>
  {:else if error}
    <div class="flex h-20 w-full items-center justify-center bg-zinc-100 text-[10px] text-zinc-400 dark:bg-zinc-800">
      Sayfa {page}
    </div>
  {/if}
  <canvas
    bind:this={canvas}
    class="w-full h-auto object-contain block rounded transition-opacity {loading ? 'opacity-0 absolute' : 'opacity-100'}"
  ></canvas>
</div>
