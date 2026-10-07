<script lang="ts">
  import { Loader2 } from "@lucide/svelte";

  export let label: string;
  export let disabled = false;
  export let busy = false;
  export let variant: "primary" | "ghost" = "primary";
  export let onClick: () => void;
</script>

<button
  class="flex items-center gap-1.5 sm:gap-2 rounded-lg px-3 py-1.5 sm:px-4 sm:py-2 text-xs sm:text-sm font-medium transition-all disabled:cursor-not-allowed disabled:opacity-40 whitespace-nowrap shrink-0 select-none
    {variant === 'primary'
      ? 'bg-indigo-600 text-white hover:bg-indigo-500 shadow-sm shadow-indigo-600/20 active:scale-[0.98]'
      : 'bg-zinc-200 text-zinc-700 hover:bg-zinc-300 dark:bg-zinc-800 dark:text-zinc-200 dark:hover:bg-zinc-700 active:scale-[0.98]'}"
  {disabled}
  on:click={onClick}
>
  {#if busy}
    <Loader2 class="animate-spin" size={15} />
  {/if}
  <slot />{label}
</button>
