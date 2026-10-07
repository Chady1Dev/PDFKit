<script lang="ts">
  import { onMount } from "svelte";
  import { X, ChevronLeft, ChevronRight } from "@lucide/svelte";

  export let page: number;
  export let pageCount: number;
  export let rotation = 0;
  export let renderPage: (page: number, scale: number) => Promise<HTMLCanvasElement>;
  export let onClose: () => void;

  let canvas: HTMLCanvasElement;
  let loading = true;

  async function render() {
    loading = true;
    try {
      const rendered = await renderPage(page, 1.4);
      const ctx = canvas.getContext("2d")!;
      canvas.width = rendered.width;
      canvas.height = rendered.height;
      ctx.drawImage(rendered, 0, 0);
    } finally {
      loading = false;
    }
  }

  $: if (page) render();

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowLeft" && page > 1) page -= 1;
      if (e.key === "ArrowRight" && page < pageCount) page += 1;
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div
  class="fixed inset-0 z-40 flex items-center justify-center bg-black/70 p-6"
  on:click|self={onClose}
>
  <div class="relative flex max-h-full flex-col items-center gap-2">
    <div class="flex items-center gap-3 text-white">
      <button class="rounded bg-white/10 p-1.5 hover:bg-white/20 disabled:opacity-30"
        disabled={page <= 1} on:click={() => (page -= 1)}>
        <ChevronLeft size={18} />
      </button>
      <span class="text-sm">{page} / {pageCount}</span>
      <button class="rounded bg-white/10 p-1.5 hover:bg-white/20 disabled:opacity-30"
        disabled={page >= pageCount} on:click={() => (page += 1)}>
        <ChevronRight size={18} />
      </button>
      <button class="ml-4 rounded bg-white/10 p-1.5 hover:bg-red-500" on:click={onClose}>
        <X size={18} />
      </button>
    </div>

    {#if loading}
      <div class="h-[70vh] w-[50vw] animate-pulse rounded bg-zinc-700"></div>
    {/if}
    <canvas
      bind:this={canvas}
      class="max-h-[78vh] rounded bg-white shadow-2xl {loading ? 'hidden' : ''}"
      style="transform: rotate({rotation}deg);"
    ></canvas>
  </div>
</div>
