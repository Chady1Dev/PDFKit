<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Search, ChevronUp, ChevronDown, X, CaseSensitive } from "@lucide/svelte";
  import type { OpenDocument } from "../docStore";
  import type { TextItemData } from "../pdf";

  export let doc: OpenDocument;
  export let onJumpToPage: (page: number) => void;
  export let onHighlightMatch: (match: SearchMatch | null) => void;
  export let onClose: () => void;

  export interface SearchMatch {
    page: number;
    text: string;
    x: number;
    y: number;
    width: number;
    height: number;
  }

  let query = "";
  let matchCase = false;
  let matches: SearchMatch[] = [];
  let currentIndex = -1;
  let searching = false;
  let searchDebounceTimer: any;
  let inputEl: HTMLInputElement;

  onMount(() => {
    tick().then(() => inputEl?.focus());
  });

  async function performSearch() {
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(async () => {
      const q = matchCase ? query.trim() : query.trim().toLowerCase();
      if (!q || !doc) {
        matches = [];
        currentIndex = -1;
        onHighlightMatch(null);
        return;
      }

      searching = true;
      const foundMatches: SearchMatch[] = [];

      try {
        for (let p = 1; p <= doc.pageCount; p++) {
          const items = await doc.handle.getPageTextItems(p);
          for (const item of items) {
            const itemText = matchCase ? item.str : item.str.toLowerCase();
            let startIdx = 0;
            while ((startIdx = itemText.indexOf(q, startIdx)) !== -1) {
              const charWidth = item.width / Math.max(1, item.str.length);
              foundMatches.push({
                page: p,
                text: item.str.substr(startIdx, q.length),
                x: item.x + startIdx * charWidth,
                y: item.y,
                width: Math.max(14, q.length * charWidth),
                height: item.height || 14,
              });
              startIdx += q.length;
            }
          }
        }

        matches = foundMatches;
        if (matches.length > 0) {
          // If current page has matches, focus the first one on or after current page
          const firstOnPage = matches.findIndex((m) => m.page >= doc.currentPage);
          currentIndex = firstOnPage !== -1 ? firstOnPage : 0;
          jumpToCurrent();
        } else {
          currentIndex = -1;
          onHighlightMatch(null);
        }
      } catch (e) {
        console.warn("Search error:", e);
      } finally {
        searching = false;
      }
    }, 150);
  }

  function jumpToCurrent() {
    if (currentIndex < 0 || currentIndex >= matches.length) return;
    const match = matches[currentIndex];
    if (doc.currentPage !== match.page) {
      onJumpToPage(match.page);
    }
    onHighlightMatch(match);
  }

  function nextMatch() {
    if (matches.length === 0) return;
    currentIndex = (currentIndex + 1) % matches.length;
    jumpToCurrent();
  }

  function prevMatch() {
    if (matches.length === 0) return;
    currentIndex = (currentIndex - 1 + matches.length) % matches.length;
    jumpToCurrent();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) {
        prevMatch();
      } else {
        nextMatch();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }

  $: if (query !== undefined || matchCase !== undefined) {
    performSearch();
  }
</script>

<div class="absolute right-4 top-4 z-40 flex items-center gap-1.5 rounded-xl border border-zinc-200/80 bg-white/95 p-1.5 shadow-xl backdrop-blur-md dark:border-zinc-700/80 dark:bg-zinc-800/95 text-zinc-800 dark:text-zinc-200 animate-in fade-in slide-in-from-top-2 duration-150">
  <Search size={15} class="ml-1.5 text-zinc-400 shrink-0" />

  <input
    bind:this={inputEl}
    bind:value={query}
    on:keydown={handleKeydown}
    placeholder="PDF içinde ara..."
    class="w-44 bg-transparent px-1.5 py-0.5 text-xs font-medium outline-none placeholder:text-zinc-400"
  />

  <div class="flex items-center text-[11px] font-mono font-semibold text-zinc-500 px-1 shrink-0 min-w-[50px] justify-center">
    {#if searching}
      <span class="text-zinc-400 animate-pulse text-[10px]">Aranıyor...</span>
    {:else if query.trim() && matches.length === 0}
      <span class="text-rose-500 text-[10px]">0 sonuç</span>
    {:else if matches.length > 0}
      <span class="text-indigo-600 dark:text-indigo-400">{currentIndex + 1}/{matches.length}</span>
    {/if}
  </div>

  <div class="h-3.5 w-px bg-zinc-200 dark:bg-zinc-700"></div>

  <!-- Case sensitive toggle -->
  <button
    class="rounded p-1 transition-colors {matchCase ? 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/60 dark:text-indigo-300' : 'text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-700'}"
    title={matchCase ? "Büyük/küçük harf duyarlı: Açık" : "Büyük/küçük harf duyarlı: Kapalı"}
    on:click={() => { matchCase = !matchCase; }}
  >
    <CaseSensitive size={14} />
  </button>

  <!-- Prev match button -->
  <button
    class="rounded p-1 text-zinc-500 hover:bg-zinc-100 hover:text-zinc-800 disabled:opacity-30 dark:text-zinc-400 dark:hover:bg-zinc-700 transition-colors"
    title="Önceki Eşleşme (Shift+Enter)"
    disabled={matches.length === 0}
    on:click={prevMatch}
  >
    <ChevronUp size={14} />
  </button>

  <!-- Next match button -->
  <button
    class="rounded p-1 text-zinc-500 hover:bg-zinc-100 hover:text-zinc-800 disabled:opacity-30 dark:text-zinc-400 dark:hover:bg-zinc-700 transition-colors"
    title="Sonraki Eşleşme (Enter)"
    disabled={matches.length === 0}
    on:click={nextMatch}
  >
    <ChevronDown size={14} />
  </button>

  <div class="h-3.5 w-px bg-zinc-200 dark:bg-zinc-700"></div>

  <!-- Close button -->
  <button
    class="rounded p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-700 transition-colors"
    title="Aramayı Kapat (Esc)"
    on:click={onClose}
  >
    <X size={14} />
  </button>
</div>
