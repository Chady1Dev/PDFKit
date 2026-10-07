<script lang="ts">
  import { AlertTriangle, X } from "@lucide/svelte";

  export let title = "Değişiklikleri Kaydetmek İstiyor musunuz?";
  export let message = "Bu belgede kaydedilmemiş düzenlemeler var. Kaydetmeden çıkmak istediğinizden emin misiniz?";
  export let confirmLabel = "Kaydetmeden Çık";
  export let cancelLabel = "Vazgeç";
  export let onConfirm: () => void;
  export let onCancel: () => void;
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4" on:click|self={onCancel}>
  <div class="w-full max-w-md rounded-2xl border border-zinc-200 bg-white p-6 shadow-2xl dark:border-zinc-800 dark:bg-zinc-900">
    <div class="flex items-start gap-3">
      <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-50 text-amber-600 dark:bg-amber-950/60 dark:text-amber-400">
        <AlertTriangle size={20} />
      </div>
      <div class="flex-1">
        <h3 class="text-base font-bold text-zinc-900 dark:text-zinc-100">{title}</h3>
        <p class="mt-1 text-xs leading-relaxed text-zinc-500 dark:text-zinc-400">{message}</p>
      </div>
      <button class="rounded-lg p-1 text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800" on:click={onCancel}>
        <X size={16} />
      </button>
    </div>

    <div class="mt-6 flex justify-end gap-2 border-t border-zinc-100 pt-3 dark:border-zinc-800">
      <button
        class="rounded-xl border border-zinc-200 px-4 py-2 text-xs font-semibold text-zinc-600 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
        on:click={onCancel}
      >
        {cancelLabel}
      </button>
      <button
        class="rounded-xl bg-rose-600 px-4 py-2 text-xs font-semibold text-white hover:bg-rose-500 shadow-sm"
        on:click={onConfirm}
      >
        {confirmLabel}
      </button>
    </div>
  </div>
</div>
