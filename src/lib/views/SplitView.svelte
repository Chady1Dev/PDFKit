<script lang="ts">
  import { onDestroy } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import ThumbnailGrid, { type GridItem } from "../components/ThumbnailGrid.svelte";
  import PreviewModal from "../components/PreviewModal.svelte";
  import { openPdf, type PdfDocHandle } from "../pdf";
  import { splitPdf, organizePdf, baseName, type PageRange } from "../api";
  import { pagesToRanges } from "../selection";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";
  import { CheckSquare, Square, FolderDown, FileDown } from "@lucide/svelte";

  let doc: PdfDocHandle | null = null;
  let items: GridItem[] = [];
  let selection = new Set<number>();
  let previewPage: number | null = null;
  let rangesText = "";
  let busy = false;

  $: if ($pendingFiles && $pendingFiles.targetView === "split") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      load(pdfs);
    }
    pendingFiles.set(null);
  }

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
    rangesText = "";
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

  $: effectiveRanges = (() => {
    if (rangesText.trim()) {
      return rangesText
        .split(",")
        .map((p) => p.trim())
        .filter(Boolean)
        .map((p) => {
          const [a, b] = p.split("-").map((n) => parseInt(n, 10));
          return { start: a, end: Number.isNaN(b) ? a : b };
        });
    }
    if (selection.size > 0) {
      return pagesToRanges([...selection].map((i) => i + 1));
    }
    return [] as PageRange[];
  })();

  // 1. Extract selected pages into a single PDF
  async function extractSelectedToSinglePdf() {
    if (!doc || selection.size === 0) {
      pushToast("Lütfen çıkarılacak sayfaları seçin.", "info");
      return;
    }
    const sortedPages = [...selection].sort((a, b) => a - b).map((i) => i + 1);
    const stem = baseName(doc.path).replace(/\.pdf$/i, "");
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: `${stem}_secilen_sayfalar.pdf`,
    });
    if (!output) return;

    busy = true;
    try {
      const organizePayload = sortedPages.map((p) => ({ source_page: p, rotate_delta: 0 }));
      const result = await organizePdf(doc.path, organizePayload, output);
      logActivity([baseName(doc.path)], "Sayfalar Ayrıldı (Tek PDF)", output);
      pushToast(`${result.total_pages} sayfa içeren PDF başarıyla kaydedildi: ${baseName(output)}`, "success", {
        label: "Klasöre Git",
        path: output,
      });
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }

  // 2. Split into separate files in a selected directory
  async function runSplitToFolder() {
    if (!doc) return;
    const selected = await open({ directory: true, multiple: false });
    const outputDir = Array.isArray(selected) ? selected[0] : selected;
    if (!outputDir) return;

    busy = true;
    try {
      const result = await splitPdf(doc.path, effectiveRanges, outputDir);
      logActivity([baseName(doc.path)], "Sayfalar Ayrıldı (Klasöre)", outputDir);
      pushToast(`${result.outputs.length} adet PDF dosyası oluşturuldu: ${baseName(outputDir)}`, "success", {
        label: "Klasöre Git",
        path: outputDir,
      });
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<ToolHeader
  title="PDF Ayır"
  description="İstediğiniz sayfaları TİK atarak seçip tek bir PDF yapabilir veya sayfaları ayrı ayrı dosyalara bölebilirsiniz."
>
  {#if doc}
    <div class="flex items-center gap-2">
      {#if selection.size > 0}
        <ActionButton
          label="Seçilenleri Tek PDF Yap ({selection.size})"
          icon={FileDown}
          {busy}
          onClick={extractSelectedToSinglePdf}
        />
      {/if}
      <ActionButton
        variant={selection.size > 0 ? "secondary" : "primary"}
        label={rangesText.trim()
          ? `Aralıklara Göre Böl (${effectiveRanges.length} dosya)`
          : selection.size > 0
          ? `Seçilenleri Ayrı Dosya Yap (${selection.size} dosya)`
          : `Her Sayfayı Ayrı Dosya Yap (${doc.pageCount} dosya)`}
        icon={FolderDown}
        {busy}
        onClick={runSplitToFolder}
      />
    </div>
  {/if}
</ToolHeader>

{#if !doc}
  <FileDropzone extensions={["pdf"]} label="Ayırılacak PDF'i sürükleyin veya seçin" onFiles={load} />
{:else}
  <div class="mb-3 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-zinc-200 bg-white p-2.5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
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
          Sayfa seçimi yapılmadı (Tüm sayfalar)
        </span>
      {/if}
    </div>

    <div class="flex items-center gap-2">
      <span class="text-xs text-zinc-400">Özel Aralık:</span>
      <input
        class="w-44 rounded-lg border border-zinc-300 bg-white px-2.5 py-1 text-xs text-zinc-900 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-100"
        bind:value={rangesText}
        placeholder="örn. 1-3, 5-8"
      />
      <span class="text-xs font-medium text-zinc-400 truncate max-w-[150px]" title={doc.path}>
        {baseName(doc.path)}
      </span>
    </div>
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
