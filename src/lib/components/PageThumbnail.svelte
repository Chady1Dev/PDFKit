<script lang="ts">
  import { onMount } from "svelte";
  import { RotateCw, Trash2, Maximize2, Check, ChevronLeft, ChevronRight } from "@lucide/svelte";

  export let page: number;
  export let rotation = 0;
  export let selected = false;
  export let label = "";
  export let width = 140;
  export let height = 190;
  export let rotatable = false;
  export let removable = false;
  export let reorderable = false;
  export let canMoveLeft = false;
  export let canMoveRight = false;
  export let showCheckbox = false;
  export let selectionOrder: number | undefined = undefined;
  export let renderPage: (page: number, scale: number) => Promise<HTMLCanvasElement>;
  export let onSelect: (e: MouseEvent) => void = () => {};
  export let onToggleSelect: () => void = () => {};
  export let onRotate: () => void = () => {};
  export let onRemove: () => void = () => {};
  export let onPreview: () => void = () => {};
  export let onMoveLeft: () => void = () => {};
  export let onMoveRight: () => void = () => {};

  let canvas: HTMLCanvasElement;
  let loading = true;

  onMount(async () => {
    try {
      const scale = Math.max(0.4, Math.min(1.2, (width / 200) * 0.55));
      const rendered = await renderPage(page, scale);
      const ctx = canvas.getContext("2d")!;
      canvas.width = rendered.width;
      canvas.height = rendered.height;
      ctx.drawImage(rendered, 0, 0);
    } finally {
      loading = false;
    }
  });
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div
  class="group relative flex flex-col items-center rounded-lg border-2 p-1 transition-colors cursor-pointer select-none
    {selected
      ? 'border-indigo-500 bg-indigo-500/10 shadow-sm'
      : 'border-zinc-200 bg-white hover:border-zinc-400 dark:border-zinc-700 dark:bg-zinc-900 dark:hover:border-zinc-500'}"
  style="width: {width}px; height: {height}px;"
  on:click={(e) => {
    if (showCheckbox) {
      onToggleSelect();
    } else {
      onSelect(e);
    }
  }}
>
  <div class="relative flex flex-1 items-center justify-center overflow-hidden w-full">
    {#if loading}
      <div class="animate-pulse rounded bg-zinc-200 dark:bg-zinc-800" style="width: {width - 30}px; height: {height - 60}px;"></div>
    {/if}
    <canvas
      bind:this={canvas}
      class="max-h-full max-w-full rounded shadow-sm transition-transform duration-200 {loading ? 'hidden' : ''}"
      style="transform: rotate({rotation}deg);"
    ></canvas>

    <!-- Top Action Overlay: Select Checkbox & Action Icons (Büyüt, Döndür, Sil) -->
    <div class="absolute inset-x-1 top-1 flex items-center justify-between pointer-events-none z-10">
      {#if showCheckbox}
        <button
          type="button"
          class="pointer-events-auto flex h-6 min-w-6 px-1 items-center justify-center rounded-md border-2 transition-all shadow-sm
            {selected
              ? 'border-indigo-600 bg-indigo-600 text-white scale-105 shadow-indigo-500/40 z-20 font-bold'
              : 'border-zinc-400 bg-white/95 text-transparent hover:border-indigo-500 hover:scale-105 dark:border-zinc-500 dark:bg-zinc-800/95'}"
          title={selected ? (selectionOrder !== undefined ? `Seçim Sırası: #${selectionOrder} (Kaldırmak için tıkla)` : "Seçimi Kaldır") : "Sayfayı Seç"}
          on:click|stopPropagation={onToggleSelect}
        >
          {#if selected}
            {#if selectionOrder !== undefined}
              <span class="text-xs font-black leading-none text-white tracking-tight">{selectionOrder}</span>
            {:else}
              <Check size={13} strokeWidth={3} />
            {/if}
          {:else}
            <span class="opacity-0 text-xs">0</span>
          {/if}
        </button>
      {:else}
        <div></div>
      {/if}

      <div class="pointer-events-auto flex items-center gap-1">
        {#if rotatable}
          <button
            class="rounded bg-zinc-900/70 p-1 text-white hover:bg-indigo-600 shadow-sm transition-colors"
            title="90° döndür"
            on:click|stopPropagation={onRotate}
          >
            <RotateCw size={12} />
          </button>
        {/if}
        {#if removable}
          <button
            class="rounded bg-zinc-900/70 p-1 text-white hover:bg-red-600 shadow-sm transition-colors"
            title="Sil"
            on:click|stopPropagation={onRemove}
          >
            <Trash2 size={12} />
          </button>
        {/if}
        <button
          class="rounded bg-zinc-900/70 p-1 text-white hover:bg-indigo-600 shadow-sm transition-colors"
          title="Büyük Önizleme"
          on:click|stopPropagation={onPreview}
        >
          <Maximize2 size={12} />
        </button>
      </div>
    </div>
  </div>

  <div class="mt-1 flex w-full items-center justify-between px-0.5">
    {#if reorderable}
      <button
        type="button"
        class="flex h-5 w-5 items-center justify-center rounded text-zinc-400 hover:bg-zinc-200 hover:text-zinc-800 disabled:opacity-20 disabled:hover:bg-transparent dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
        title="Sola Taşı"
        disabled={!canMoveLeft}
        on:click|stopPropagation={onMoveLeft}
      >
        <ChevronLeft size={13} />
      </button>
    {/if}
    <span class="truncate text-[11px] font-medium {selected ? 'text-indigo-600 dark:text-indigo-400 font-semibold' : 'text-zinc-500 dark:text-zinc-400'} {reorderable ? '' : 'mx-auto'}">
      {label || `Sayfa ${page}`}
    </span>
    {#if reorderable}
      <button
        type="button"
        class="flex h-5 w-5 items-center justify-center rounded text-zinc-400 hover:bg-zinc-200 hover:text-zinc-800 disabled:opacity-20 disabled:hover:bg-transparent dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
        title="Sağa Taşı"
        disabled={!canMoveRight}
        on:click|stopPropagation={onMoveRight}
      >
        <ChevronRight size={13} />
      </button>
    {/if}
  </div>
</div>
