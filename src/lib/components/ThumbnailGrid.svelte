<script lang="ts">
  import { onDestroy } from "svelte";
  import PageThumbnail from "./PageThumbnail.svelte";

  export interface GridItem {
    key: string;
    page: number;
    rotation: number;
  }

  export let items: GridItem[] = [];
  export let renderPage: (page: number, scale: number) => Promise<HTMLCanvasElement>;
  export let selection: Set<number> = new Set();
  export let rotatable = false;
  export let removable = false;
  export let reorderable = false;
  export let showCheckbox = false;
  export let getSelectionOrder: ((index: number) => number | undefined) | undefined = undefined;
  export let onSelect: (e: MouseEvent, index: number) => void = () => {};
  export let onToggleSelect: ((index: number) => void) | undefined = undefined;
  export let onRotate: (index: number) => void = () => {};
  export let onRemove: (index: number) => void = () => {};
  export let onPreview: (index: number) => void = () => {};
  export let onReorder: (from: number, to: number) => void = () => {};
  export let onReorderMultiple: ((indices: number[], targetIndex: number) => void) | undefined = undefined;
  export let itemWidth = 150;
  export let itemHeight = 200;
  export let gap = 14;

  let container: HTMLDivElement;
  let viewWidth = 800;
  let viewHeight = 500;
  let scrollTop = 0;

  // Drag & drop state
  let isDragging = false;
  let dragCandidate: { index: number; startX: number; startY: number } | null = null;
  let activeDragIndices: number[] = [];
  let dragOverIndex: number | null = null;
  let pointerX = 0;
  let pointerY = 0;
  let autoScrollSpeed = 0;
  let scrollAnimId: number | null = null;
  let wasDragging = false;

  $: actualWidth = Math.min(itemWidth, Math.max(160, viewWidth - gap * 2));
  $: actualHeight = Math.round(actualWidth * (itemHeight / itemWidth));
  $: cols = Math.max(1, Math.floor((viewWidth + gap) / (actualWidth + gap)));
  $: rows = Math.ceil(items.length / cols);
  $: totalHeight = rows * (actualHeight + gap);
  $: firstRow = Math.max(0, Math.floor(scrollTop / (actualHeight + gap)) - 2);
  $: lastRow = Math.min(rows - 1, Math.ceil((scrollTop + viewHeight) / (actualHeight + gap)) + 2);
  $: visible = items
    .map((item, index) => ({ item, index }))
    .filter(({ index }) => {
      // Keep dragged and target items permanently rendered during drag to prevent unmounting bugs
      if (isDragging && (dragOverIndex === index || activeDragIndices.includes(index))) {
        return true;
      }
      const row = Math.floor(index / cols);
      return row >= firstRow && row <= lastRow;
    });

  function pos(index: number) {
    const row = Math.floor(index / cols);
    const col = index % cols;
    return `left: ${col * (actualWidth + gap)}px; top: ${row * (actualHeight + gap)}px; width: ${actualWidth}px; height: ${actualHeight}px;`;
  }

  function handleCardPointerDown(e: PointerEvent, index: number) {
    if (!reorderable || e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (target && (target.closest("button") || target.closest("input") || target.closest("a"))) {
      return;
    }
    dragCandidate = { index, startX: e.clientX, startY: e.clientY };
  }

  function handleWindowPointerMove(e: PointerEvent) {
    if (dragCandidate && !isDragging) {
      const dist = Math.hypot(e.clientX - dragCandidate.startX, e.clientY - dragCandidate.startY);
      if (dist > 6) {
        isDragging = true;
        wasDragging = true;
        if (selection.has(dragCandidate.index) && selection.size > 1) {
          if (getSelectionOrder) {
            activeDragIndices = [...selection].sort((a, b) => {
              const orderA = getSelectionOrder(a) ?? 999999;
              const orderB = getSelectionOrder(b) ?? 999999;
              return orderA - orderB;
            });
          } else {
            activeDragIndices = [...selection].sort((a, b) => a - b);
          }
        } else {
          activeDragIndices = [dragCandidate.index];
        }
      }
    }

    if (isDragging) {
      pointerX = e.clientX;
      pointerY = e.clientY;
      updateDragTargetAndScroll(e.clientX, e.clientY);
    }
  }

  function updateDragTargetAndScroll(clientX: number, clientY: number) {
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const padding = 12;
    const relX = clientX - rect.left - padding;
    const relY = clientY - rect.top - padding;

    const col = Math.min(cols - 1, Math.max(0, Math.floor(relX / (actualWidth + gap))));
    const row = Math.max(0, Math.floor((relY + container.scrollTop) / (actualHeight + gap)));
    const target = Math.min(items.length - 1, Math.max(0, row * cols + col));
    dragOverIndex = target;

    // Boundary auto-scroll
    const threshold = 75;
    if (relY < threshold && clientY >= rect.top - 40) {
      const intensity = Math.max(0.15, (threshold - relY) / threshold);
      autoScrollSpeed = -Math.ceil(intensity * 24);
      startAutoScroll();
    } else if (relY > rect.height - threshold && clientY <= rect.bottom + 40) {
      const intensity = Math.max(0.15, (relY - (rect.height - threshold)) / threshold);
      autoScrollSpeed = Math.ceil(intensity * 24);
      startAutoScroll();
    } else {
      stopAutoScroll();
    }
  }

  function startAutoScroll() {
    if (scrollAnimId !== null) return;
    function loop() {
      if (!isDragging || autoScrollSpeed === 0 || !container) {
        stopAutoScroll();
        return;
      }
      container.scrollTop += autoScrollSpeed;
      scrollTop = container.scrollTop;
      updateDragTargetAndScroll(pointerX, pointerY);
      scrollAnimId = requestAnimationFrame(loop);
    }
    scrollAnimId = requestAnimationFrame(loop);
  }

  function stopAutoScroll() {
    if (scrollAnimId !== null) {
      cancelAnimationFrame(scrollAnimId);
      scrollAnimId = null;
    }
    autoScrollSpeed = 0;
  }

  function handleWindowPointerUp() {
    stopAutoScroll();
    if (isDragging && dragOverIndex !== null && activeDragIndices.length > 0) {
      if (onReorderMultiple) {
        onReorderMultiple(activeDragIndices, dragOverIndex);
      } else if (activeDragIndices.length > 0) {
        onReorder(activeDragIndices[0], dragOverIndex);
      }
    }
    isDragging = false;
    dragCandidate = null;
    dragOverIndex = null;
    activeDragIndices = [];
    setTimeout(() => {
      wasDragging = false;
    }, 50);
  }

  function handleWindowKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape" && isDragging) {
      stopAutoScroll();
      isDragging = false;
      dragCandidate = null;
      dragOverIndex = null;
      activeDragIndices = [];
      wasDragging = false;
    }
  }

  onDestroy(stopAutoScroll);
</script>

<svelte:window
  on:pointermove={handleWindowPointerMove}
  on:pointerup={handleWindowPointerUp}
  on:keydown={handleWindowKeyDown}
/>

<!-- Floating multi-item drag indicator following the mouse -->
{#if isDragging}
  <div
    class="fixed pointer-events-none z-50 flex items-center gap-2.5 rounded-xl bg-indigo-600/95 px-3.5 py-2 text-xs font-bold text-white shadow-2xl backdrop-blur-md border border-indigo-400/40 select-none animate-in fade-in zoom-in-95 duration-75"
    style="left: {pointerX + 16}px; top: {pointerY + 16}px;"
  >
    <span class="flex h-5 w-5 items-center justify-center rounded-full bg-white text-indigo-600 text-[11px] font-black shadow-xs">
      {activeDragIndices.length}
    </span>
    <span>
      {activeDragIndices.length > 1
        ? `${activeDragIndices.length} Sayfa Taşınıyor (Sıralı)`
        : `Sayfa ${items[activeDragIndices[0]]?.page ?? ""} Taşınıyor`}
    </span>
  </div>
{/if}

<div
  bind:this={container}
  bind:clientWidth={viewWidth}
  bind:clientHeight={viewHeight}
  class="min-h-0 flex-1 overflow-y-auto rounded-xl border border-zinc-200 bg-zinc-50 p-3 dark:border-zinc-800 dark:bg-zinc-950 select-none"
  on:scroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
>
  <div class="relative mx-auto" style="height: {totalHeight}px; width: {cols * (actualWidth + gap) - gap}px;">
    {#each visible as { item, index } (item.key)}
      {@const isDragged = isDragging && activeDragIndices.includes(index)}
      {@const isTarget = isDragging && dragOverIndex === index && !isDragged}
      <div
        class="absolute transition-all duration-150
          {isDragged ? 'opacity-35 scale-95 ring-2 ring-indigo-400/40 rounded-lg' : ''}
          {isTarget ? 'ring-4 ring-indigo-500 scale-105 shadow-2xl rounded-lg z-30' : ''}"
        style={pos(index)}
        on:pointerdown={(e) => handleCardPointerDown(e, index)}
        role="listitem"
      >
        <PageThumbnail
          page={item.page}
          rotation={item.rotation}
          selected={selection.has(index)}
          selectionOrder={getSelectionOrder ? getSelectionOrder(index) : undefined}
          width={actualWidth}
          height={actualHeight}
          {rotatable}
          {removable}
          {reorderable}
          canMoveLeft={index > 0}
          canMoveRight={index < items.length - 1}
          {showCheckbox}
          {renderPage}
          onSelect={(e) => {
            if (wasDragging) return;
            onSelect(e, index);
          }}
          onToggleSelect={() => {
            if (wasDragging) return;
            if (onToggleSelect) onToggleSelect(index);
            else onSelect(new MouseEvent("click"), index);
          }}
          onRotate={() => onRotate(index)}
          onRemove={() => onRemove(index)}
          onPreview={() => onPreview(index)}
          onMoveLeft={() => onReorder(index, index - 1)}
          onMoveRight={() => onReorder(index, index + 1)}
        />
      </div>
    {/each}
  </div>
</div>
