<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { currentView, theme, pendingFiles, type ViewId } from "./lib/stores";
  import { openPdfDocument, hasAnyUnsavedChanges } from "./lib/docStore";
  import ConfirmModal from "./lib/components/ConfirmModal.svelte";
  import {
    LayoutGrid, FilePlus2, Scissors, RotateCw, Layers, FileArchive,
    Image, Lock, Stamp, Sun, Moon, PenLine, History,
  } from "@lucide/svelte";
  import Toast from "./lib/components/Toast.svelte";
  import Dashboard from "./lib/views/Dashboard.svelte";
  import EditorView from "./lib/views/EditorView.svelte";
  import MergeView from "./lib/views/MergeView.svelte";
  import SplitView from "./lib/views/SplitView.svelte";
  import RotateView from "./lib/views/RotateView.svelte";
  import OrganizeView from "./lib/views/OrganizeView.svelte";
  import CompressView from "./lib/views/CompressView.svelte";
  import ImagesView from "./lib/views/ImagesView.svelte";
  import SecurityView from "./lib/views/SecurityView.svelte";
  import AnnotateView from "./lib/views/AnnotateView.svelte";
  import ActivityView from "./lib/views/ActivityView.svelte";

  let showExitModal = false;
  let unlistenDrop: (() => void) | undefined;
  let unlistenClose: (() => void) | undefined;
  let unlistenSingleInstance: (() => void) | undefined;
  let hoveredDropView: ViewId | null = null;

  onMount(async () => {
    // 1. Single instance listener (files opened externally while running)
    try {
      unlistenSingleInstance = await listen<string[]>("open-files", async (event) => {
        const rawArgs = event.payload || [];
        const pdfs = rawArgs.filter((p) => typeof p === "string" && p.toLowerCase().endsWith(".pdf"));
        for (const pdf of pdfs) {
          await openPdfDocument(pdf);
        }
        if (pdfs.length > 0) {
          currentView.set("editor");
        }
      });
    } catch (e) {
      console.warn("Single-instance listener failed:", e);
    }

    // 2. Initial launch arguments (files opened on startup)
    try {
      const initialArgs = await invoke<string[]>("get_cli_args");
      const pdfs = (initialArgs || []).filter((p) => typeof p === "string" && p.toLowerCase().endsWith(".pdf"));
      for (const pdf of pdfs) {
        await openPdfDocument(pdf);
      }
      if (pdfs.length > 0) {
        currentView.set("editor");
      }
    } catch (e) {
      console.warn("Initial CLI args fetch failed:", e);
    }

    try {
      unlistenDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
        if (event.payload.type === "over") {
          const dpr = window.devicePixelRatio || 1;
          const lx = event.payload.position.x / dpr;
          const ly = event.payload.position.y / dpr;
          const el = document.elementFromPoint(lx, ly);
          const btn = el?.closest("[data-view]");
          hoveredDropView = (btn?.getAttribute("data-view") as ViewId | null) ?? null;
        } else if (event.payload.type === "leave") {
          hoveredDropView = null;
        } else if (event.payload.type === "drop") {
          const dpr = window.devicePixelRatio || 1;
          const lx = event.payload.position.x / dpr;
          const ly = event.payload.position.y / dpr;
          const el = document.elementFromPoint(lx, ly);
          const btn = el?.closest("[data-view]");
          const target = (btn?.getAttribute("data-view") as ViewId | null) ?? hoveredDropView;
          hoveredDropView = null;

          const paths = event.payload.paths;
          if (!paths || paths.length === 0) return;

          let destinationView: ViewId;
          if (target) {
            destinationView = target;
          } else {
            let active: ViewId = "dashboard";
            currentView.subscribe((v) => (active = v))();
            destinationView = active === "dashboard" ? "editor" : active;
          }

          if (destinationView === "editor") {
            const pdfs = paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
            for (const pdf of pdfs) {
              await openPdfDocument(pdf);
            }
            currentView.set("editor");
          } else {
            currentView.set(destinationView);
            pendingFiles.set({ targetView: destinationView, paths });
          }
        }
      });
    } catch (e) {
      console.warn("Global drag-drop listener failed:", e);
    }

    try {
      unlistenClose = await getCurrentWindow().onCloseRequested(async (event) => {
        if (hasAnyUnsavedChanges()) {
          event.preventDefault();
          showExitModal = true;
        }
      });
    } catch (e) {
      console.warn("Close guard listener failed:", e);
    }
  });

  onDestroy(() => {
    unlistenDrop?.();
    unlistenClose?.();
    unlistenSingleInstance?.();
  });

  async function forceExitApp() {
    showExitModal = false;
    try {
      if (unlistenClose) {
        unlistenClose();
        unlistenClose = undefined;
      }
      await invoke("exit_app");
    } catch {
      try {
        await getCurrentWindow().destroy();
      } catch {
        window.close();
      }
    }
  }

  function handleGlobalKeyDown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && (e.key === "q" || e.key === "Q")) {
      e.preventDefault();
      if (hasAnyUnsavedChanges()) {
        showExitModal = true;
      } else {
        forceExitApp();
      }
    }
  }

  const views: Record<Exclude<ViewId, "dashboard">, typeof Dashboard> = {
    editor: EditorView,
    merge: MergeView,
    split: SplitView,
    rotate: RotateView,
    organize: OrganizeView,
    compress: CompressView,
    images: ImagesView,
    security: SecurityView,
    annotate: AnnotateView,
    activity: ActivityView,
  };

  const nav: { group: string; items: { id: ViewId; label: string; icon: typeof FilePlus2 }[] }[] = [
    {
      group: "Editör",
      items: [
        { id: "editor", label: "PDF Düzenle", icon: PenLine },
      ],
    },
    {
      group: "Temel",
      items: [
        { id: "merge", label: "Birleştir", icon: FilePlus2 },
        { id: "split", label: "Ayır", icon: Scissors },
        { id: "organize", label: "Organize Et", icon: RotateCw },
        { id: "compress", label: "Sıkıştır", icon: FileArchive },
        { id: "images", label: "Dönüştür", icon: Image },
        { id: "activity", label: "Son Değişiklikler", icon: History },
      ],
    },
    {
      group: "Gelişmiş",
      items: [
        { id: "security", label: "Şifrele", icon: Lock },
        { id: "annotate", label: "Filigran & Numara", icon: Stamp },
      ],
    },
  ];
</script>

<svelte:window on:keydown={handleGlobalKeyDown} />

<div class="flex h-full bg-zinc-100 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
  <nav class="flex w-56 shrink-0 flex-col border-r border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900">
    <button
      class="flex items-center gap-2 px-4 py-4 text-left text-lg font-bold tracking-tight"
      on:click={() => currentView.set("dashboard")}
    >
      <LayoutGrid size={20} class="text-indigo-600" /> PDFkit
    </button>

    <div class="flex-1 overflow-y-auto px-3 pb-3">
      {#each nav as section}
        <p class="mb-1 mt-4 px-2 text-[11px] font-semibold uppercase tracking-wider text-zinc-400 dark:text-zinc-500">
          {section.group}
        </p>
        {#each section.items as item}
          <button
            data-view={item.id}
            class="mb-0.5 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm transition-all
              {hoveredDropView === item.id
                ? 'bg-indigo-500/25 text-indigo-600 ring-2 ring-indigo-500 scale-[1.02] shadow-sm'
                : $currentView === item.id
                ? 'bg-indigo-600 text-white'
                : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800'}"
            on:click={() => currentView.set(item.id)}
          >
            <svelte:component this={item.icon} size={15} class="shrink-0" />
            <span class="truncate">{item.label}</span>
          </button>
        {/each}
      {/each}
    </div>

    <button
      class="m-3 flex items-center gap-2 rounded-lg border border-zinc-200 px-3 py-2 text-sm text-zinc-500 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-400 dark:hover:bg-zinc-800"
      on:click={theme.toggle}
    >
      {#if $theme === "dark"}<Sun size={15} /> Açık Mod{:else}<Moon size={15} /> Koyu Mod{/if}
    </button>
  </nav>

  <main class="flex min-w-0 flex-1 flex-col overflow-hidden {$currentView === 'editor' ? 'p-0' : 'overflow-y-auto p-6'}">
    {#if $currentView === "dashboard"}
      <Dashboard />
    {:else}
      <svelte:component this={views[$currentView]} />
    {/if}
  </main>
</div>

<Toast />

{#if showExitModal}
  <ConfirmModal
    title="Uygulamadan Çıkmak İstiyor musunuz?"
    message="Açık belgelerinizde kaydedilmemiş düzenlemeler var. Kaydetmeden çıkarsanız bu değişiklikler kaybolacaktır."
    confirmLabel="Kaydetmeden Çık"
    cancelLabel="Vazgeç"
    onConfirm={forceExitApp}
    onCancel={() => (showExitModal = false)}
  />
{/if}
