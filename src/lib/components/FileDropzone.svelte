<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onMount, onDestroy } from "svelte";
  import { UploadCloud } from "@lucide/svelte";

  /** Allowed extensions without dot, e.g. ["pdf"] or ["png","jpg"]. */
  export let extensions: string[] = ["pdf"];
  export let multiple = false;
  export let label = "Dosyaları buraya sürükleyin veya tıklayın";
  export let onFiles: (paths: string[]) => void;

  let dragging = false;
  let unlisten: (() => void) | undefined;

  const matches = (p: string) =>
    extensions.some((ext) => p.toLowerCase().endsWith(`.${ext.toLowerCase()}`));

  async function pick() {
    const selected = await open({
      multiple,
      filters: [{ name: "Dosyalar", extensions }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    onFiles(paths.filter(matches));
  }

  onMount(async () => {
    unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") dragging = true;
      else if (event.payload.type === "leave") dragging = false;
      else if (event.payload.type === "drop") {
        dragging = false;
        const paths = event.payload.paths.filter(matches);
        const limited = multiple ? paths : paths.slice(0, 1);
        if (limited.length) onFiles(limited);
      }
    });
  });

  onDestroy(() => unlisten?.());
</script>

<button
  class="flex w-full flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed p-8 text-sm transition-colors
    {dragging
      ? 'border-indigo-500 bg-indigo-500/10 text-indigo-500'
      : 'border-zinc-300 text-zinc-500 hover:border-indigo-400 hover:text-indigo-500 dark:border-zinc-700 dark:text-zinc-400'}"
  on:click={pick}
>
  <UploadCloud size={28} />
  <span>{label}</span>
  <span class="text-xs opacity-60">{extensions.map((e) => `.${e}`).join(", ")}</span>
</button>
