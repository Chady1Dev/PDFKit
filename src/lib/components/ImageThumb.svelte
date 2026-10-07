<script lang="ts">
  import { onDestroy } from "svelte";
  import { readFile } from "@tauri-apps/plugin-fs";
  import { Image as ImageIcon, Loader2 } from "@lucide/svelte";

  export let path: string;
  export let alt: string = "";

  let src: string | null = null;
  let loading = true;
  let error = false;
  let currentObjectUrl: string | null = null;

  function getMime(p: string): string {
    const ext = p.split(".").pop()?.toLowerCase();
    if (ext === "png") return "image/png";
    if (ext === "webp") return "image/webp";
    if (ext === "bmp") return "image/bmp";
    if (ext === "gif") return "image/gif";
    return "image/jpeg";
  }

  async function load(p: string) {
    loading = true;
    error = false;
    if (currentObjectUrl) {
      URL.revokeObjectURL(currentObjectUrl);
      currentObjectUrl = null;
      src = null;
    }
    try {
      const bytes = await readFile(p);
      const blob = new Blob([bytes], { type: getMime(p) });
      currentObjectUrl = URL.createObjectURL(blob);
      src = currentObjectUrl;
    } catch (e) {
      console.error("Görsel yüklenemedi:", p, e);
      error = true;
    } finally {
      loading = false;
    }
  }

  $: if (path) {
    load(path);
  }

  onDestroy(() => {
    if (currentObjectUrl) {
      URL.revokeObjectURL(currentObjectUrl);
      currentObjectUrl = null;
    }
  });
</script>

{#if src}
  <img {src} {alt} class="max-h-full max-w-full object-contain pointer-events-none select-none" />
{:else if loading}
  <div class="flex flex-col items-center justify-center gap-1.5 text-zinc-400">
    <Loader2 size={18} class="animate-spin text-indigo-500" />
    <span class="text-[10px]">Yükleniyor...</span>
  </div>
{:else}
  <div class="flex flex-col items-center justify-center gap-1 text-zinc-400 p-2 text-center">
    <ImageIcon size={22} class="opacity-40" />
    <span class="text-[10px] truncate max-w-[90px]">{alt}</span>
  </div>
{/if}
