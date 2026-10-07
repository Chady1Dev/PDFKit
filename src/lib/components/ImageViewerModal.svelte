<script lang="ts">
  import { onMount } from "svelte";
  import { X, ChevronLeft, ChevronRight } from "@lucide/svelte";
  import ImageThumb from "./ImageThumb.svelte";
  import { baseName } from "../api";

  export let images: string[] = [];
  export let currentIndex: number = 0;
  export let onClose: () => void;

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowLeft" && currentIndex > 0) currentIndex -= 1;
      if (e.key === "ArrowRight" && currentIndex < images.length - 1) currentIndex += 1;
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/85 backdrop-blur-sm p-4 md:p-8"
  on:click|self={onClose}
>
  <div class="relative flex h-full max-h-[92vh] w-full max-w-5xl flex-col items-center gap-3">
    <!-- Top toolbar -->
    <div class="flex w-full items-center justify-between text-white px-1">
      <div class="flex items-center gap-2">
        <span class="rounded bg-indigo-600 px-2 py-0.5 text-xs font-bold shadow">
          {currentIndex + 1} / {images.length}
        </span>
        <span class="text-xs text-zinc-300 font-medium truncate max-w-sm md:max-w-md">
          {baseName(images[currentIndex])}
        </span>
      </div>

      <div class="flex items-center gap-2">
        <button
          class="rounded-lg bg-white/10 p-1.5 hover:bg-white/20 disabled:opacity-30 transition-colors"
          disabled={currentIndex <= 0}
          on:click={() => (currentIndex -= 1)}
          title="Önceki Görsel"
        >
          <ChevronLeft size={18} />
        </button>
        <button
          class="rounded-lg bg-white/10 p-1.5 hover:bg-white/20 disabled:opacity-30 transition-colors"
          disabled={currentIndex >= images.length - 1}
          on:click={() => (currentIndex += 1)}
          title="Sonraki Görsel"
        >
          <ChevronRight size={18} />
        </button>
        <button
          class="ml-3 rounded-lg bg-white/10 p-1.5 hover:bg-red-600 transition-colors"
          on:click={onClose}
          title="Kapat (Esc)"
        >
          <X size={18} />
        </button>
      </div>
    </div>

    <!-- Image preview area -->
    <div class="relative flex flex-1 w-full items-center justify-center overflow-hidden rounded-2xl bg-zinc-950/80 p-4 border border-zinc-800 shadow-2xl">
      {#key images[currentIndex]}
        <ImageThumb path={images[currentIndex]} alt={baseName(images[currentIndex])} />
      {/key}
    </div>
  </div>
</div>
