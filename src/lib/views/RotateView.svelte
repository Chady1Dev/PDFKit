<script lang="ts">
  import { onDestroy } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import ThumbnailGrid, { type GridItem } from "../components/ThumbnailGrid.svelte";
  import PreviewModal from "../components/PreviewModal.svelte";
  import { openPdf, type PdfDocHandle } from "../pdf";
  import { rotatePages, baseName } from "../api";
  import { pushToast, pendingFiles } from "../stores";
  import { CheckSquare, Square, RotateCw } from "@lucide/svelte";

  $: if ($pendingFiles && $pendingFiles.targetView === "rotate") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      load(pdfs);
    }
    pendingFiles.set(null);
  }

  let doc: PdfDocHandle | null = null;
  let items: GridItem[] = [];
  let selection = new Set<number>();
  let previewPage: number | null = null;
  let busy = false;

  async function load(paths: string[]) {
    await unload();
    try {
      doc = await openPdf(paths[0]);
      items = Array.from({ length: doc.pageCount }, (_, i) => ({
        key: `p${i + 1}`,
        page: i + 1,
        rotation: 0,
      }));
    } catch (e) {
      pushToast(`Açılamadı: ${e}`, "error");
    }
  }

  async function unload() {
    if (doc) await doc.destroy();
    doc = null;
    items = [];
    selection = new Set();
    previewPage = null;
  }

  onDestroy(unload);

  function toggleSelection(index: number) {
    const next = new Set(selection);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    selection = next;
  }

  function selectAll() {
    selection = new Set(items.map((_, i) => i));
  }

  function clearSelection() {
    selection = new Set();
  }

  async function run(angle: number) {
    if (!doc) return;
    const stem = baseName(doc.path).replace(/\.pdf$/i, "");
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: `${stem}_dondurulmus_${angle}deg.pdf`,
    });
    if (!output) return;
    busy = true;
    try {
      // Empty selection = all pages (backend handles).
      const pages = [...selection].sort((a, b) => a - b).map((i) => i + 1);
      const result = await rotatePages(doc.path, pages, angle, output);
      const msg = pages.length > 0
        ? `${pages.length} sayfa ${angle}° döndürülerek kaydedildi: ${baseName(result.outputs[0])}`
        : `Tüm sayfalar ${angle}° döndürülerek kaydedildi: ${baseName(result.outputs[0])}`;
      pushToast(msg, "success");
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<ToolHeader
  title="Sayfa Döndür"
  description="Döndürmek istediğiniz sayfaları seçip açıyı belirleyerek kaydedin (seçim yapmazsanız tüm belge döndürülür)."
>
  {#if doc}
    <div class="flex items-center gap-1.5">
      <ActionButton variant="secondary" label="90° Döndür" icon={RotateCw} {busy} onClick={() => run(90)} />
      <ActionButton variant="secondary" label="180° Döndür" icon={RotateCw} {busy} onClick={() => run(180)} />
      <ActionButton variant="secondary" label="270° Döndür" icon={RotateCw} {busy} onClick={() => run(270)} />
    </div>
  {/if}
</ToolHeader>

{#if !doc}
  <FileDropzone extensions={["pdf"]} label="Döndürülecek PDF'i sürükleyin veya seçin" onFiles={load} />
{:else}
  <div class="mb-3 flex items-center justify-between rounded-xl border border-zinc-200 bg-white p-2.5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
    <div class="flex items-center gap-2">
      <button
        type="button"
        class="inline-flex items-center gap-1.5 rounded-lg border border-zinc-200 bg-zinc-50 px-2.5 py-1 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200 dark:hover:bg-zinc-700"
        on:click={selectAll}
      >
        <CheckSquare size={13} /> Tümünü Seç
      </button>

      <button
        type="button"
        disabled={selection.size === 0}
        class="inline-flex items-center gap-1.5 rounded-lg border border-zinc-200 bg-zinc-50 px-2.5 py-1 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 disabled:opacity-40 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200 dark:hover:bg-zinc-700"
        on:click={clearSelection}
      >
        <Square size={13} /> Seçimi Kaldır
      </button>

      <div class="h-4 w-px bg-zinc-200 dark:bg-zinc-800 mx-1"></div>

      {#if selection.size > 0}
        <span class="rounded-full bg-indigo-100 px-2.5 py-0.5 text-xs font-semibold text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300">
          {selection.size} / {doc.pageCount} sayfa seçildi
        </span>
      {:else}
        <span class="text-xs text-zinc-500 dark:text-zinc-400">
          Seçim yapılmadı (Tüm sayfalar döndürülecek)
        </span>
      {/if}
    </div>

    <span class="text-xs font-medium text-zinc-400 truncate max-w-[200px]" title={doc.path}>
      {baseName(doc.path)}
    </span>
  </div>

  <ThumbnailGrid
    {items}
    {selection}
    showCheckbox={true}
    renderPage={doc.renderPage}
    onToggleSelect={toggleSelection}
    onSelect={(_, i) => toggleSelection(i)}
    onPreview={(i) => (previewPage = items[i].page)}
  />
{/if}

{#if previewPage !== null && doc}
  <PreviewModal
    page={previewPage}
    pageCount={doc.pageCount}
    renderPage={doc.renderPage}
    onClose={() => (previewPage = null)}
  />
{/if}
