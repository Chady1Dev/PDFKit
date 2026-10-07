<script lang="ts">
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import { activityLogs, clearActivityLogs, type ActivityLog } from "../activityStore";
  import { revealInExplorer, baseName } from "../api";
  import { pushToast } from "../stores";
  import {
    History, Trash2, FolderOpen, ChevronLeft, ChevronRight,
    FileText, Calendar, CheckCircle2, Clock
  } from "@lucide/svelte";

  const PAGE_SIZE = 15;
  let currentPage = 1;

  $: totalLogs = $activityLogs.length;
  $: totalPages = Math.max(1, Math.ceil(totalLogs / PAGE_SIZE));
  $: if (currentPage > totalPages) currentPage = totalPages;

  $: pagedLogs = $activityLogs.slice(
    (currentPage - 1) * PAGE_SIZE,
    currentPage * PAGE_SIZE
  );

  async function handleShowInExplorer(path: string) {
    try {
      await revealInExplorer(path);
    } catch (e) {
      pushToast(`Dosya konumu açılamadı: ${e}`, "error");
    }
  }

  function handleClear() {
    if (confirm("Son işlemler geçmişini temizlemek istediğinizden emin misiniz?")) {
      clearActivityLogs();
      currentPage = 1;
      pushToast("İşlem geçmişi başarıyla temizlendi.", "info");
    }
  }
</script>

<ToolHeader
  title="Son Değişiklikler"
  description="Bu cihazda ve oturumunuzda gerçekleştirdiğiniz PDF işlemlerinin yerel kayıt listesi."
>
  {#if totalLogs > 0}
    <ActionButton
      variant="ghost"
      label="Son İşlemler Listesini Temizle"
      icon={Trash2}
      onClick={handleClear}
    />
  {/if}
</ToolHeader>

{#if totalLogs === 0}
  <div class="flex flex-1 flex-col items-center justify-center rounded-2xl border-2 border-dashed border-zinc-200 p-12 text-center dark:border-zinc-800">
    <div class="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-950 dark:text-indigo-400">
      <History size={28} />
    </div>
    <h3 class="text-base font-semibold text-zinc-900 dark:text-zinc-100">Henüz Kayıtlı Bir İşlem Yok</h3>
    <p class="mt-1 max-w-sm text-xs text-zinc-500 dark:text-zinc-400">
      PDF düzenleme, birleştirme, ayırma veya dönüştürme işlemlerini tamamlayıp kaydettiğinizde, yapılan işlemler burada listelenecektir.
    </p>
  </div>
{:else}
  <div class="flex flex-1 flex-col overflow-hidden rounded-xl border border-zinc-200 bg-white shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
    <!-- Log list table / items -->
    <div class="flex-1 overflow-y-auto divide-y divide-zinc-100 dark:divide-zinc-800/60">
      {#each pagedLogs as log, i (log.id)}
        <div class="flex flex-wrap items-center justify-between gap-3 p-3.5 transition-colors hover:bg-zinc-50/80 dark:hover:bg-zinc-800/40">
          <div class="flex items-start gap-3 min-w-0">
            <div class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-indigo-50 text-indigo-600 dark:bg-indigo-950/80 dark:text-indigo-400">
              <CheckCircle2 size={16} />
            </div>

            <div class="flex flex-col min-w-0">
              <p class="text-xs leading-relaxed text-zinc-700 dark:text-zinc-300">
                {#if log.files.length === 1}
                  <span class="font-bold text-zinc-900 dark:text-zinc-100">{log.files[0]}</span>
                  dosyasına
                  <span class="inline-flex items-center rounded-md bg-indigo-50 px-1.5 py-0.5 font-semibold text-indigo-700 dark:bg-indigo-950/90 dark:text-indigo-300">
                    {log.action}
                  </span>
                  işlemi uygulandı.
                {:else}
                  <span class="font-bold text-zinc-900 dark:text-zinc-100">
                    {log.files.slice(0, 2).join(", ")}{log.files.length > 2 ? ` (+${log.files.length - 2} dosya)` : ""}
                  </span>
                  dosyalarına
                  <span class="inline-flex items-center rounded-md bg-indigo-50 px-1.5 py-0.5 font-semibold text-indigo-700 dark:bg-indigo-950/90 dark:text-indigo-300">
                    {log.action}
                  </span>
                  işlemi uygulandı.
                {/if}
              </p>

              {#if log.outputPath}
                <span class="mt-0.5 truncate text-[11px] text-zinc-400 font-mono" title={log.outputPath}>
                  Çıktı: {baseName(log.outputPath)}
                </span>
              {/if}
            </div>
          </div>

          <div class="flex items-center gap-3 shrink-0 ml-auto">
            <div class="flex items-center gap-1.5 text-xs text-zinc-400 font-medium">
              <Clock size={13} />
              <span>{log.timestamp}</span>
            </div>

            {#if log.outputPath}
              <button
                type="button"
                class="inline-flex items-center gap-1 rounded-lg border border-zinc-200 bg-white px-2.5 py-1 text-xs font-semibold text-zinc-700 shadow-sm transition-all hover:bg-zinc-100 hover:text-indigo-600 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700"
                title="Dosyayı klasörde göster"
                on:click={() => handleShowInExplorer(log.outputPath!)}
              >
                <FolderOpen size={13} /> Klasörde Göster
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    <!-- Pagination Controls (15 per page) -->
    <div class="flex items-center justify-between border-t border-zinc-200 bg-zinc-50/60 px-4 py-2 text-xs dark:border-zinc-800 dark:bg-zinc-900/60">
      <span class="text-zinc-500 dark:text-zinc-400">
        Toplam <strong class="text-zinc-800 dark:text-zinc-200">{totalLogs}</strong> işlem kaydı ({PAGE_SIZE}'erli)
      </span>

      <div class="flex items-center gap-1">
        <button
          type="button"
          class="rounded p-1 text-zinc-500 hover:bg-zinc-200 disabled:opacity-30 dark:hover:bg-zinc-800"
          disabled={currentPage <= 1}
          on:click={() => (currentPage = Math.max(1, currentPage - 1))}
          title="Önceki Sayfa"
        >
          <ChevronLeft size={16} />
        </button>

        <!-- Page Numbers -->
        <div class="flex items-center gap-1 px-1">
          {#each Array.from({ length: totalPages }, (_, i) => i + 1) as p}
            {#if p === 1 || p === totalPages || (p >= currentPage - 2 && p <= currentPage + 2)}
              <button
                type="button"
                class="h-6 min-w-[24px] rounded px-1.5 font-medium transition-all {currentPage === p ? 'bg-indigo-600 text-white shadow-sm' : 'text-zinc-600 hover:bg-zinc-200 dark:text-zinc-400 dark:hover:bg-zinc-800'}"
                on:click={() => (currentPage = p)}
              >
                {p}
              </button>
            {:else if p === currentPage - 3 || p === currentPage + 3}
              <span class="px-1 text-zinc-400">...</span>
            {/if}
          {/each}
        </div>

        <button
          type="button"
          class="rounded p-1 text-zinc-500 hover:bg-zinc-200 disabled:opacity-30 dark:hover:bg-zinc-800"
          disabled={currentPage >= totalPages}
          on:click={() => (currentPage = Math.min(totalPages, currentPage + 1))}
          title="Sonraki Sayfa"
        >
          <ChevronRight size={16} />
        </button>
      </div>
    </div>
  </div>
{/if}
