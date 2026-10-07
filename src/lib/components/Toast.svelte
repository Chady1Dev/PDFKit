<script lang="ts">
  import { toasts, dismissToast } from "../stores";
  import { revealInExplorer } from "../api";
  import { CheckCircle2, AlertCircle, Info, FolderOpen, X } from "@lucide/svelte";

  const icons = { success: CheckCircle2, error: AlertCircle, info: Info };
  const colors = {
    success: "border-emerald-500/40 bg-emerald-50/95 text-emerald-900 dark:bg-emerald-950/95 dark:text-emerald-200 dark:border-emerald-500/30",
    error: "border-red-500/40 bg-red-50/95 text-red-900 dark:bg-red-950/95 dark:text-red-200 dark:border-red-500/30",
    info: "border-indigo-500/40 bg-indigo-50/95 text-indigo-900 dark:bg-indigo-950/95 dark:text-indigo-200 dark:border-indigo-500/30",
  };

  async function handleAction(path: string) {
    try {
      await revealInExplorer(path);
    } catch (e) {
      console.error("revealInExplorer error:", e);
    }
  }

  // En fazla 25 karakter dosya adı sınırı ile taşmaları ve buton çakışmalarını önler
  function formatMessage(msg: string): string {
    if (!msg) return "";
    return msg.replace(/([^\s:/\\]+\.(?:pdf|docx|xlsx|pptx|txt|png|jpg|jpeg|webp|bmp|tiff))/gi, (match) => {
      if (match.length > 25) {
        const lastDot = match.lastIndexOf(".");
        const ext = match.slice(lastDot);
        const stem = match.slice(0, lastDot);
        const maxStem = Math.max(8, 25 - ext.length - 1);
        return stem.slice(0, maxStem) + "…" + ext;
      }
      return match;
    });
  }
</script>

<div class="pointer-events-none fixed bottom-4 right-4 z-50 flex w-[380px] max-w-[calc(100vw-2rem)] flex-col gap-2.5">
  {#each $toasts as toast (toast.id)}
    <div
      class="pointer-events-auto relative flex items-start gap-3 rounded-2xl border p-3.5 text-xs shadow-2xl backdrop-blur-md transition-all animate-in fade-in slide-in-from-bottom-2 duration-200 {colors[toast.kind]}"
    >
      <svelte:component this={icons[toast.kind]} size={18} class="shrink-0 mt-0.5 text-emerald-600 dark:text-emerald-400" />

      <!-- Metin ve Buton Bölümü (Dikey Ayrılmış) -->
      <div class="flex flex-col gap-2 min-w-0 flex-1 pr-6">
        <span class="break-words font-semibold leading-relaxed select-text">
          {formatMessage(toast.message)}
        </span>

        {#if toast.actionLabel && toast.actionPath}
          <div class="pt-0.5">
            <button
              type="button"
              class="inline-flex items-center gap-1.5 rounded-lg border border-emerald-600/30 bg-emerald-600 px-3 py-1.5 text-[11px] font-bold text-white shadow-xs transition-all hover:bg-emerald-700 active:scale-95 dark:bg-emerald-500 dark:text-zinc-950 dark:hover:bg-emerald-400"
              on:click={() => handleAction(toast.actionPath!)}
            >
              <FolderOpen size={13} />
              <span>{toast.actionLabel}</span>
            </button>
          </div>
        {/if}
      </div>

      <!-- Kapat Butonu (Sağ Üst Köşede Sabit) -->
      <button
        type="button"
        class="absolute top-3 right-3 rounded-lg p-1 opacity-60 hover:opacity-100 hover:bg-black/10 dark:hover:bg-white/10 transition-all text-zinc-500 dark:text-zinc-400"
        title="Kapat"
        on:click={() => dismissToast(toast.id)}
      >
        <X size={14} />
      </button>
    </div>
  {/each}
</div>
