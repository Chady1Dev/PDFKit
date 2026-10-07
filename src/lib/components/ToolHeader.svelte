<script lang="ts">
  import { ArrowLeft } from "@lucide/svelte";
  import { currentView } from "../stores";

  export let title: string;
  export let description = "";
</script>

<header class="mb-4 flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
  <div class="flex items-center gap-3 min-w-0">
    <button
      class="shrink-0 rounded-lg p-2 text-zinc-500 hover:bg-zinc-200 dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors"
      title="Panele dön"
      on:click={() => currentView.set("dashboard")}
    >
      <ArrowLeft size={18} />
    </button>
    <div class="min-w-0">
      <h2 class="text-lg font-semibold text-zinc-900 dark:text-zinc-100 truncate">{title}</h2>
      {#if description}
        <p class="text-xs sm:text-sm text-zinc-500 dark:text-zinc-400">{description}</p>
      {/if}
    </div>
  </div>
  <div class="flex flex-wrap items-center gap-2">
    <slot />
  </div>
</header>
