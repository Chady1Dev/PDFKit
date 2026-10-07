<script lang="ts">
  import { currentView, type ViewId } from "../stores";
  import {
    FilePlus2, Scissors, RotateCw, FileArchive, Image,
    Lock, Stamp, FileSignature, PenLine, Sparkles, History,
  } from "@lucide/svelte";

  interface ToolCard {
    id: ViewId;
    title: string;
    description: string;
    icon: typeof FilePlus2;
    phase: 1 | 2;
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
