<script lang="ts">
  import { currentView, type ViewId } from "../stores";
  import { openPdfDocument } from "../docStore";
  import { recentFiles, removeRecentFile, clearRecentFiles, type RecentFile } from "../recentFiles";
  import {
    FilePlus2, Scissors, RotateCw, FileArchive, Image,
    Lock, Stamp, FileSignature, PenLine, Sparkles, History,
    FileText, Clock, Trash2, X, ChevronRight,
  } from "@lucide/svelte";

  interface ToolCard {
    id: ViewId;
    title: string;
    description: string;
    icon: typeof FilePlus2;
    phase: 1 | 2;
  }

  function formatTime(timestamp: number): string {
    const diff = Date.now() - timestamp;
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return "Az önce";
    if (mins < 60) return `${mins} dk önce`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours} saat önce`;
    const days = Math.floor(hours / 24);
    if (days === 1) return "Dün";
    if (days < 7) return `${days} gün önce`;
    return new Date(timestamp).toLocaleDateString("tr-TR");
  }

  async function handleOpenRecent(file: RecentFile) {
    try {
      await openPdfDocument(file.path);
      currentView.set("editor");
    } catch (e) {
      console.warn("Could not open recent file:", e);
    }
  }

  const tools: ToolCard[] = [
    { id: "editor", title: "PDF Düzenle", description: "Doğrudan sayfa üzerinde metin, çizim, vurgu, sansür ve imza", icon: PenLine, phase: 1 },
    { id: "merge", title: "Birleştir", description: "Birden çok PDF'i tek dosyada birleştir", icon: FilePlus2, phase: 1 },
    { id: "split", title: "Ayır", description: "Sayfa aralıklarını ayrı dosyalara böl", icon: Scissors, phase: 1 },
    { id: "organize", title: "Organize Et", description: "Sayfaları döndür, sürükle-bırak ile sırala ve sil", icon: RotateCw, phase: 1 },
    { id: "compress", title: "Sıkıştır", description: "Gereksiz verileri temizle, dosya boyutunu küçült", icon: FileArchive, phase: 1 },
    { id: "images", title: "Dönüştür (Office & Görsel)", description: "PDF'i Word, Excel, PPT, TXT veya görsellere dönüştürün; ya da görsellerden PDF yapın", icon: Image, phase: 1 },
    { id: "activity", title: "Son Değişiklikler", description: "Yapılan işlemlerin yerel geçmişi ve kayıtları", icon: History, phase: 1 },
    { id: "security", title: "Şifrele", description: "AES şifreleme ve şifre kaldırma", icon: Lock, phase: 2 },
    { id: "annotate", title: "Filigran & Numara", description: "Filigran ve sayfa numarası ekle", icon: Stamp, phase: 2 },
    { id: "forms", title: "Form & İmza", description: "Form doldur, imza görseli ekle", icon: FileSignature, phase: 2 },
  ];
</script>

<div class="mx-auto max-w-4xl">
  <div class="mb-6 flex flex-col md:flex-row md:items-center justify-between gap-4 rounded-2xl bg-gradient-to-r from-indigo-600 to-violet-600 p-6 text-white shadow-lg">
    <div class="max-w-xl">
      <div class="mb-2 inline-flex items-center gap-1.5 rounded-full bg-white/20 px-3 py-1 text-xs font-semibold backdrop-blur-md">
        <Sparkles size={13} /> Tam Koruma & %100 Yerli
      </div>
      <h2 class="text-2xl font-bold tracking-tight">Görsel PDF Düzenleyici</h2>
      <p class="mt-1 text-sm text-indigo-100">
        Sayfalar üzerinde özgürce düzenleme yapın, Dosyalarınız yerelde çalışır, dış sunuculara yüklenmez. 🔒
      </p>
    </div>
    <button
      class="inline-flex shrink-0 items-center justify-center gap-2 rounded-xl bg-white px-5 py-3 text-sm font-bold text-indigo-700 shadow-md transition-all hover:bg-indigo-50 hover:shadow-lg active:scale-95"
      on:click={() => currentView.set("editor")}
    >
      <PenLine size={16} /> Editörü Başlat
    </button>
  </div>

  {#if $recentFiles.length > 0}
    <div class="mb-6">
      <div class="mb-2 flex items-center justify-between">
        <h3 class="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-zinc-400 dark:text-zinc-500">
          <Clock size={13} /> Son Açılan Dosyalar
        </h3>
        <button
          class="flex items-center gap-1 text-xs text-zinc-400 hover:text-red-500 transition-colors"
          on:click={clearRecentFiles}
          title="Son açılanlar listesini temizle"
        >
          <Trash2 size={12} /> Temizle
        </button>
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
        {#each $recentFiles as file}
          <div
            class="group relative flex items-center justify-between gap-3 rounded-xl border border-zinc-200 bg-white p-3 text-left transition-all hover:border-indigo-400 hover:shadow-sm dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-indigo-600"
          >
            <button
              class="flex min-w-0 flex-1 items-center gap-3 text-left"
              on:click={() => handleOpenRecent(file)}
            >
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-red-50 text-red-600 dark:bg-red-950/60 dark:text-red-400">
                <FileText size={18} />
              </div>
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm font-medium text-zinc-800 group-hover:text-indigo-600 dark:text-zinc-200 dark:group-hover:text-indigo-400">
                  {file.name}
                </p>
                <div class="flex items-center gap-2 text-[11px] text-zinc-400 dark:text-zinc-500">
                  <span>{formatTime(file.lastOpened)}</span>
                  {#if file.pageCount}
                    <span>•</span>
                    <span>{file.pageCount} sayfa</span>
                  {/if}
                </div>
              </div>
            </button>
            <button
              class="p-1 rounded text-zinc-400 hover:text-red-500 hover:bg-zinc-100 dark:hover:bg-zinc-800 opacity-0 group-hover:opacity-100 transition-opacity"
              on:click|stopPropagation={() => removeRecentFile(file.path)}
              title="Listeden kaldır"
            >
              <X size={14} />
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <h2 class="mb-4 text-xl font-bold text-zinc-900 dark:text-zinc-100">PDF Araçları</h2>

  {#each [1, 2] as phase}
    <h3 class="mb-2 text-xs font-semibold uppercase tracking-wider text-zinc-400 dark:text-zinc-500">
      {phase === 1 ? "Temel Araçlar" : "Gelişmiş Araçlar"}
    </h3>
    <div class="mb-6 grid grid-cols-2 gap-3 md:grid-cols-3">
      {#each tools.filter((t) => t.phase === phase) as tool}
        <button
          class="group flex flex-col gap-2 rounded-xl border border-zinc-200 bg-white p-4 text-left transition-all hover:border-indigo-400 hover:shadow-md dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-indigo-600"
          on:click={() => currentView.set(tool.id)}
        >
          <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-indigo-50 text-indigo-600 group-hover:bg-indigo-600 group-hover:text-white dark:bg-indigo-950 dark:text-indigo-400">
            <svelte:component this={tool.icon} size={18} />
          </div>
          <div>
            <p class="font-medium text-zinc-900 dark:text-zinc-100">{tool.title}</p>
            <p class="text-xs text-zinc-500 dark:text-zinc-400">{tool.description}</p>
          </div>
        </button>
      {/each}
    </div>
  {/each}
</div>
