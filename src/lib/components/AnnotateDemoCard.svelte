<script lang="ts">
  import { onMount } from "svelte";

  export let page: number;
  export let pageCount: number;
  export let renderPage: (p: number, scale: number) => Promise<HTMLCanvasElement>;
  export let activeTab: "watermark" | "numbers" = "watermark";
  export let wmText = "";
  export let wmSize = 48;
  export let wmOpacity = 0.25;
  export let numSize = 10;
  export let numMargin = 24;

  let canvas: HTMLCanvasElement;
  let loading = true;
  const scale = 0.35;

  onMount(async () => {
    try {
      const rendered = await renderPage(page, scale);
      canvas.width = rendered.width;
      canvas.height = rendered.height;
      const ctx = canvas.getContext("2d")!;
      ctx.drawImage(rendered, 0, 0);
    } finally {
      loading = false;
    }
  });
</script>

<div class="relative flex flex-col items-center rounded-xl border border-zinc-200 bg-white p-2.5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
  <div class="relative overflow-hidden rounded-lg border border-zinc-200 bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-800 flex items-center justify-center">
    {#if loading}
      <div class="flex h-[260px] w-[180px] animate-pulse items-center justify-center text-xs text-zinc-400">
        Yükleniyor...
      </div>
    {/if}
    <canvas bind:this={canvas} class="max-h-[280px] max-w-full rounded shadow-inner {loading ? 'hidden' : 'block'}"></canvas>

    <!-- Live Watermark Overlay -->
    {#if !loading && activeTab === "watermark" && wmText}
      <div class="pointer-events-none absolute inset-0 flex items-center justify-center overflow-hidden">
        <span
          class="select-none font-bold uppercase tracking-wider transition-all"
          style="
            transform: rotate(-45deg);
            font-size: {Math.max(10, Math.round(wmSize * scale))}px;
            opacity: {wmOpacity};
            color: #555555;
            white-space: nowrap;
          "
        >
          {wmText}
        </span>
      </div>
    {/if}

    <!-- Live Page Number Overlay -->
    {#if !loading && activeTab === "numbers"}
      <div
        class="pointer-events-none absolute inset-x-0 flex justify-center transition-all"
        style="bottom: {Math.max(4, Math.round(numMargin * scale))}px;"
      >
        <span
          class="select-none font-sans font-medium text-zinc-700 dark:text-zinc-300 transition-all"
          style="font-size: {Math.max(8, Math.round(numSize * 0.95))}px; line-height: 1;"
        >
          {page} / {pageCount}
        </span>
      </div>
    {/if}
  </div>

  <span class="mt-2 text-xs font-semibold text-zinc-500 dark:text-zinc-400">
    Sayfa {page} / {pageCount}
  </span>
</div>
