<script lang="ts">
  /** 0..100 */
  export let percent: number;
  export let label = "";
</script>

<div class="w-full">
  {#if label}
    <div class="mb-1 flex justify-between text-xs text-zinc-500 dark:text-zinc-400">
      <span>{label}</span>
      <span>{Math.round(percent)}%</span>
    </div>
  {/if}
  <div class="h-2 w-full overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
    <div
      class="h-full rounded-full bg-indigo-600 transition-all duration-200"
      style="width: {Math.min(100, Math.max(0, percent))}%"
    ></div>
  </div>
</div>
