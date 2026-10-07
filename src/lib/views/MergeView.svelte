<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { GripVertical, Trash2 } from "@lucide/svelte";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import { mergePdfs, getPdfInfo, baseName, fmtSize } from "../api";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";

  interface MergeEntry {
    path: string;
    pages: number | null;
    size: number;
  }

  let files: MergeEntry[] = [];
  let busy = false;
  let dragIndex: number | null = null;

  $: if ($pendingFiles && $pendingFiles.targetView === "merge") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      add(pdfs);
    }
    pendingFiles.set(null);
  }

  async function add(paths: string[]) {
    for (const path of paths) {
      if (files.some((f) => f.path === path)) continue;
      const entry: MergeEntry = { path, pages: null, size: 0 };
      files = [...files, entry];
      try {
        const info = await getPdfInfo(path);
        files = files.map((f) =>
          f.path === path ? { ...f, pages: info.page_count, size: info.file_size_bytes } : f,
        );
      } catch {
        files = files.map((f) => (f.path === path ? { ...f, pages: -1 } : f));
      }
    }
  }

  function reorder(from: number, to: number) {
    const next = [...files];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    files = next;
  }

  async function run() {
    const output = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }], defaultPath: "birlesik.pdf" });
    if (!output) return;
    busy = true;
    try {
      const result = await mergePdfs(files.map((f) => f.path), output);
      logActivity(files.map((f) => baseName(f.path)), "PDF Birleştirildi", output);
      pushToast(`Kaydedildi: ${baseName(result.outputs[0])} — ${result.total_pages} sayfa, ${fmtSize(result.total_size_bytes)}`, "success", {
        label: "Klasöre Git",
        path: output,
      });
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<ToolHeader title="PDF Birleştir" description="Dosyaları ekle, tutamaçla sırala, birleştir">
  <ActionButton label="Birleştir ve Kaydet" {busy} disabled={files.length < 2} onClick={run} />
</ToolHeader>

<FileDropzone extensions={["pdf"]} multiple label="PDF'leri sürükleyin veya seçin (en az 2)" onFiles={add} />

{#if files.length}
  <ul class="mt-3 flex flex-col gap-1">
    {#each files as f, i (f.path)}
      <li
        class="flex items-center gap-2 rounded-lg border border-zinc-200 bg-white px-3 py-2 dark:border-zinc-800 dark:bg-zinc-900"
        draggable="true"
        on:dragstart={(e) => {
          dragIndex = i;
          if (e.dataTransfer) {
            e.dataTransfer.setData("text/plain", `${i}`);
            e.dataTransfer.effectAllowed = "move";
          }
        }}
        on:dragover|preventDefault
        on:drop|preventDefault={() => {
          if (dragIndex !== null && dragIndex !== i) reorder(dragIndex, i);
          dragIndex = null;
        }}
      >
        <GripVertical size={14} class="cursor-grab text-zinc-400" />
        <span class="w-6 text-center text-xs text-zinc-400">{i + 1}</span>
        <span class="flex-1 truncate text-sm text-zinc-800 dark:text-zinc-200">{baseName(f.path)}</span>
        <span class="text-xs text-zinc-500">
          {f.pages === null ? "…" : f.pages < 0 ? "okunamadı" : `${f.pages} sayfa · ${fmtSize(f.size)}`}
        </span>
        <button class="text-zinc-400 hover:text-red-500" on:click={() => (files = files.filter((x) => x.path !== f.path))}>
          <Trash2 size={14} />
        </button>
      </li>
    {/each}
  </ul>
{/if}
