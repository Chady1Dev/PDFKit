<script lang="ts">
  import { onDestroy } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { X, CheckSquare, RotateCw, Trash2, Save } from "@lucide/svelte";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import ThumbnailGrid, { type GridItem } from "../components/ThumbnailGrid.svelte";
  import PreviewModal from "../components/PreviewModal.svelte";
  import { openPdf, type PdfDocHandle } from "../pdf";
  import { organizePdf, baseName } from "../api";
  import { applySelection } from "../selection";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";

  let doc: PdfDocHandle | null = null;
  let items: GridItem[] = [];
  let selectedIndices: number[] = [];
  $: selection = new Set(selectedIndices);
  let anchor: { value: number | null } = { value: null };
  let previewIndex: number | null = null;
  let busy = false;
  let counter = 0;

  $: if ($pendingFiles && $pendingFiles.targetView === "organize") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      load(pdfs);
    }
    pendingFiles.set(null);
  }

  function formatFileName(name: string, maxLen = 100): string {
    if (!name) return "";
    if (name.length <= maxLen) return name;
    return name.slice(0, maxLen - 3) + "...";
  }

  function getSelectionOrder(index: number): number | undefined {
    const idx = selectedIndices.indexOf(index);
    return idx >= 0 ? idx + 1 : undefined;
  }

  function handleToggleSelect(index: number) {
    const pos = selectedIndices.indexOf(index);
    if (pos >= 0) {
      selectedIndices = selectedIndices.filter((x) => x !== index);
    } else {
      selectedIndices = [...selectedIndices, index];
    }
  }

  function handleSelect(e: MouseEvent, index: number) {
    const nextSet = applySelection(e, index, selection, anchor);
    const retained = selectedIndices.filter((i) => nextSet.has(i));
    const newlyAdded = [...nextSet].filter((i) => !selectedIndices.includes(i));
    selectedIndices = [...retained, ...newlyAdded];
  }

  function clearAllSelections() {
    selectedIndices = [];
  }

  async function load(paths: string[]) {
    await unload();
    try {
      doc = await openPdf(paths[0]);
      items = Array.from({ length: doc.pageCount }, (_, i) => ({
        key: `p${i + 1}-${counter++}`,
        page: i + 1,
        rotation: 0,
      }));
      selectedIndices = [];
    } catch (e) {
      pushToast(`Açılamadı: ${e}`, "error");
    }
  }

  async function unload() {
    if (doc) await doc.destroy();
    doc = null;
    items = [];
    selectedIndices = [];
    previewIndex = null;
  }

  onDestroy(unload);

  function toggleSelectAll() {
    if (selectedIndices.length === items.length) {
      selectedIndices = [];
    } else {
      selectedIndices = items.map((_, i) => i);
    }
  }

  function rotateSelected() {
    if (selection.size === 0) return;
    items = items.map((it, i) =>
      selection.has(i) ? { ...it, rotation: (it.rotation + 90) % 360 } : it,
    );
  }

  function deleteSelected() {
    if (selectedIndices.length === 0) return;
    items = items.filter((_, i) => !selection.has(i));
    selectedIndices = [];
  }

  function reorder(from: number, to: number) {
    reorderMultiple([from], to);
  }

  function reorderMultiple(draggedIndices: number[], targetIndex: number) {
    if (draggedIndices.length === 0 || targetIndex < 0 || targetIndex >= items.length) return;
    const moving = draggedIndices.map((i) => items[i]);
    const selectedSet = new Set(draggedIndices);
    const targetItem = items[targetIndex];
    const remaining = items.filter((_, i) => !selectedSet.has(i));

    let insertPos = remaining.indexOf(targetItem);
    if (insertPos === -1) {
      insertPos = Math.min(targetIndex, remaining.length);
    }

    const next = [
      ...remaining.slice(0, insertPos),
      ...moving,
      ...remaining.slice(insertPos),
    ];

    if (draggedIndices.some((idx) => selection.has(idx))) {
      selectedIndices = Array.from({ length: moving.length }, (_, k) => insertPos + k);
    } else {
      const oldKeys = selectedIndices.map((i) => items[i]?.key).filter(Boolean);
      selectedIndices = oldKeys
        .map((k) => next.findIndex((it) => it.key === k))
        .filter((idx) => idx !== -1);
    }

    items = next;
    anchor.value = insertPos;
  }

  async function runSave() {
    if (!doc) return;
    const output = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }], defaultPath: "duzenlenmis.pdf" });
    if (!output) return;
    busy = true;
    try {
      const pages = items.map((it) => ({ source_page: it.page, rotate_delta: it.rotation }));
      const result = await organizePdf(doc.path, pages, output);
      logActivity([baseName(doc.path)], "Organize Edildi", output);
      pushToast(`Kaydedildi: ${baseName(result.outputs[0])} (${result.total_pages} sayfa)`, "success", {
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

<ToolHeader title="Organize Et" description="Sayfaları döndürün, sıralayın veya silin · Onay kutusuyla çoklu seçim yapın">
  {#if doc}
    <ActionButton
      variant="ghost"
      label={selectedIndices.length === items.length && items.length > 0 ? "Seçimi Kaldır" : "Tümünü Seç"}
      onClick={toggleSelectAll}
    >
      <CheckSquare size={15} />
    </ActionButton>
    <button
      type="button"
      disabled={selectedIndices.length === 0}
      class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 sm:px-3.5 sm:py-2 text-xs sm:text-sm font-semibold transition-all shadow-sm disabled:cursor-not-allowed disabled:opacity-40 whitespace-nowrap shrink-0 select-none
        {selectedIndices.length > 1
          ? 'bg-blue-600 text-white hover:bg-blue-500 shadow-blue-500/25 ring-2 ring-blue-500/30'
          : 'bg-zinc-200 text-zinc-700 hover:bg-zinc-300 dark:bg-zinc-800 dark:text-zinc-200 dark:hover:bg-zinc-700'}"
      title="Tüm seçimleri iptal et"
      on:click={clearAllSelections}
    >
      <X size={15} strokeWidth={2.5} />
      Seçimleri İptal Et ({selectedIndices.length})
    </button>
    <ActionButton variant="ghost" label="90° Döndür" disabled={selectedIndices.length === 0} onClick={rotateSelected}>
      <RotateCw size={15} />
    </ActionButton>
    <ActionButton variant="ghost" label="Sil" disabled={selectedIndices.length === 0} onClick={deleteSelected}>
      <Trash2 size={15} />
    </ActionButton>
    <ActionButton label="Kaydet" {busy} disabled={items.length === 0} onClick={runSave}>
      <Save size={15} />
    </ActionButton>
  {/if}
</ToolHeader>

{#if !doc}
  <FileDropzone extensions={["pdf"]} label="Düzenlenecek PDF'i sürükleyin veya seçin" onFiles={load} />
{:else}
  <div class="mb-3 flex flex-wrap items-center justify-between gap-2 text-xs">
    <div class="flex items-center gap-2 min-w-0 max-w-full">
      <span
        class="inline-flex items-center rounded-md border-2 border-zinc-900 px-3 py-1 font-semibold text-zinc-900 dark:border-zinc-200 dark:text-zinc-100 bg-transparent tracking-tight truncate max-w-[260px] sm:max-w-[420px] md:max-w-[600px] shadow-xs"
        title={baseName(doc.path)}
      >
        {formatFileName(baseName(doc.path))}
      </span>
      <span class="font-semibold text-zinc-700 dark:text-zinc-300 whitespace-nowrap shrink-0">— {items.length} sayfa</span>
    </div>
    <span class="rounded-md border border-zinc-200 bg-zinc-100/90 px-2.5 py-1 text-xs font-medium text-zinc-600 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-300 whitespace-nowrap shrink-0">
      {selection.size} seçili
    </span>
  </div>
  <ThumbnailGrid
    {items}
    {selection}
    {getSelectionOrder}
    itemWidth={275}
    itemHeight={375}
    renderPage={doc.renderPage}
    rotatable
    removable
    reorderable
    showCheckbox={true}
    onToggleSelect={handleToggleSelect}
    onSelect={handleSelect}
    onRotate={(i) => (items = items.map((it, j) => (j === i ? { ...it, rotation: (it.rotation + 90) % 360 } : it)))}
    onRemove={(i) => {
      items = items.filter((_, j) => j !== i);
      selectedIndices = selectedIndices
        .filter((idx) => idx !== i)
        .map((idx) => (idx > i ? idx - 1 : idx));
    }}
    onPreview={(i) => (previewIndex = i)}
    onReorder={reorder}
    onReorderMultiple={reorderMultiple}
  />
{/if}

{#if previewIndex !== null && doc}
  <PreviewModal
    page={items[previewIndex].page}
    pageCount={doc.pageCount}
    rotation={items[previewIndex].rotation}
    renderPage={doc.renderPage}
    onClose={() => (previewIndex = null)}
  />
{/if}
