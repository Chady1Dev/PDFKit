<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import {
    compressPdf,
    getPdfInfo,
    baseName,
    fmtSize,
    revealInExplorer,
    type CompressResult,
    type PdfInfo,
  } from "../api";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";
  import {
    FileArchive,
    CheckCircle2,
    FolderOpen,
    RefreshCw,
    X,
    FileText,
    ArrowDownRight,
    Sparkles,
    ShieldCheck,
  } from "@lucide/svelte";

  let input: string | null = null;
  let fileInfo: PdfInfo | null = null;
  let busy = false;
  let result: CompressResult | null = null;

  let level: "lossless" | "balanced" | "maximum" = "balanced";

  $: if ($pendingFiles && $pendingFiles.targetView === "compress") {
    const p = $pendingFiles.paths.filter((x) => x.toLowerCase().endsWith(".pdf"));
    if (p.length > 0) {
      setInput(p[0]);
    }
    pendingFiles.set(null);
  }

  $: savedPct =
    result && result.before_bytes > 0
      ? Math.max(0, 100 - (result.after_bytes / result.before_bytes) * 100)
      : 0;

  async function setInput(path: string) {
    input = path;
    result = null;
    try {
      fileInfo = await getPdfInfo(path);
    } catch {
      fileInfo = null;
    }
  }

  function clearFile() {
    input = null;
    fileInfo = null;
    result = null;
  }

  function formatFileName(name: string, maxLen = 100): string {
    if (!name) return "";
    if (name.length <= maxLen) return name;
    return name.slice(0, maxLen - 3) + "...";
  }

  async function run() {
    if (!input) return;
    const defaultName = baseName(input).replace(/\.pdf$/i, "") + "_sikistirilmis.pdf";
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: defaultName,
    });
    if (!output) return;
    busy = true;
    result = null;
    try {
      result = await compressPdf(input, output, level);
      if (result.already_optimized) {
        pushToast(
          "Bu belge zaten maksimum düzeyde optimize edilmiş. Orijinal dosya boyutu korundu.",
          "info"
        );
      } else {
        logActivity([baseName(input)], "PDF Sıkıştırıldı", output);
        pushToast(
          `Sıkıştırma tamam: %${savedPct.toFixed(0)} küçülme (${fmtSize(result.before_bytes)} ➔ ${fmtSize(result.after_bytes)})`,
          "success",
          {
            label: "Klasöre Git",
            path: output,
          }
        );
      }
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }

</script>

<ToolHeader
  title="PDF Sıkıştır"
  description="Gereksiz meta verileri ve akışları temizleyerek dosya boyutunu kayıpsız küçültür"
>
  {#if input && !result}
    <ActionButton variant="ghost" label="Dosyayı Değiştir" disabled={busy} onClick={clearFile}>
      <RefreshCw size={15} />
    </ActionButton>
  {/if}
  {#if result}
    <ActionButton variant="ghost" label="Yeni Dosya" onClick={clearFile}>
      <RefreshCw size={15} />
    </ActionButton>
  {/if}
  <ActionButton label="Sıkıştır" {busy} disabled={!input || busy || !!result} onClick={run}>
    <FileArchive size={16} />
  </ActionButton>
</ToolHeader>

{#if !input}
  <!-- Sürükle-Bırak Alanı (Yalnızca henüz dosya seçilmediğinde görünür) -->
  <FileDropzone
    extensions={["pdf"]}
    label="Sıkıştırılacak PDF belgesini sürükleyin veya seçin"
    onFiles={(p) => setInput(p[0])}
  />
{:else}
  <!-- Seçilen Dosya Kartı (Tek Dosya İşlemi - Dropzone kapalı) -->
  <div class="rounded-2xl border border-zinc-200 bg-white p-5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900 transition-all">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex items-center gap-3.5 min-w-0">
        <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 dark:bg-indigo-950/60 dark:text-indigo-400">
          <FileText size={24} />
        </div>
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-2">
            <span
              class="inline-flex items-center rounded-md border-2 border-zinc-900 px-3 py-0.5 text-xs font-semibold text-zinc-900 dark:border-zinc-200 dark:text-zinc-100 bg-transparent tracking-tight truncate max-w-[280px] sm:max-w-[450px] md:max-w-[650px] shadow-xs"
              title={baseName(input)}
            >
              {formatFileName(baseName(input))}
            </span>
            {#if fileInfo}
              <span class="rounded bg-zinc-100 px-2 py-0.5 text-xs font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                {fileInfo.page_count} sayfa
              </span>
            {/if}
          </div>
          <p class="mt-1 text-xs text-zinc-500 dark:text-zinc-400">
            {#if fileInfo}
              Mevcut Boyut: <strong class="text-zinc-700 dark:text-zinc-200">{fmtSize(fileInfo.file_size_bytes)}</strong>
            {:else}
              PDF belgesi hazırlandı
            {/if}
            &bull; Kayıpsız yerel sıkıştırma
          </p>
        </div>
      </div>

      <div class="flex items-center gap-2">
        {#if !result}
          <button
            type="button"
            class="flex items-center gap-1.5 rounded-lg border border-zinc-300 px-3 py-1.5 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors"
            disabled={busy}
            on:click={clearFile}
            title="Başka bir dosya seç"
          >
            <X size={14} />
            Kaldır
          </button>
        {/if}
      </div>
    </div>
  </div>

  {#if !result}
    <!-- Sıkıştırma Seviyesi Seçimi -->
    <div class="mt-4 rounded-2xl border border-zinc-200 bg-white p-5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900 transition-all">
      <div class="mb-3 flex items-center justify-between">
        <div>
          <h4 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Sıkıştırma Seviyesi</h4>
          <p class="text-xs text-zinc-500 dark:text-zinc-400">İhtiyacınıza en uygun kalite ve dosya boyutu dengesini belirleyin</p>
        </div>
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <!-- Kayıpsız -->
        <button
          type="button"
          class="flex flex-col text-left p-3.5 rounded-xl border-2 transition-all cursor-pointer {level === 'lossless' ? 'border-indigo-600 bg-indigo-50/60 dark:border-indigo-500 dark:bg-indigo-950/30' : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
          on:click={() => (level = "lossless")}
        >
          <div class="flex items-center justify-between w-full mb-1.5">
            <span class="text-xs font-bold text-zinc-900 dark:text-zinc-100">Kayıpsız</span>
            {#if level === 'lossless'}
              <span class="h-2 w-2 rounded-full bg-indigo-600 dark:bg-indigo-400"></span>
            {/if}
          </div>
          <span class="text-[11px] text-zinc-500 dark:text-zinc-400 leading-snug">
            Meta veri temizleme ve akış optimizasyonu. Görsellerde sıfır kalite kaybı.
          </span>
        </button>

        <!-- Dengeli (Önerilen) -->
        <button
          type="button"
          class="flex flex-col text-left p-3.5 rounded-xl border-2 transition-all cursor-pointer relative {level === 'balanced' ? 'border-indigo-600 bg-indigo-50/60 dark:border-indigo-500 dark:bg-indigo-950/30' : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
          on:click={() => (level = "balanced")}
        >
          <div class="flex items-center justify-between w-full mb-1.5">
            <span class="text-xs font-bold text-zinc-900 dark:text-zinc-100 flex items-center gap-1.5">
              Dengeli
              <span class="rounded bg-indigo-100 px-1.5 py-0.2 text-[10px] font-semibold text-indigo-700 dark:bg-indigo-900/60 dark:text-indigo-300">Önerilen</span>
            </span>
            {#if level === 'balanced'}
              <span class="h-2 w-2 rounded-full bg-indigo-600 dark:bg-indigo-400"></span>
            {/if}
          </div>
          <span class="text-[11px] text-zinc-500 dark:text-zinc-400 leading-snug">
            Ekran ve e-posta için ideal. Görseller yüksek kalitede yeniden örneklendirilir.
          </span>
        </button>

        <!-- Yüksek Sıkıştırma -->
        <button
          type="button"
          class="flex flex-col text-left p-3.5 rounded-xl border-2 transition-all cursor-pointer {level === 'maximum' ? 'border-indigo-600 bg-indigo-50/60 dark:border-indigo-500 dark:bg-indigo-950/30' : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
          on:click={() => (level = "maximum")}
        >
          <div class="flex items-center justify-between w-full mb-1.5">
            <span class="text-xs font-bold text-zinc-900 dark:text-zinc-100">Yüksek</span>
            {#if level === 'maximum'}
              <span class="h-2 w-2 rounded-full bg-indigo-600 dark:bg-indigo-400"></span>
            {/if}
          </div>
          <span class="text-[11px] text-zinc-500 dark:text-zinc-400 leading-snug">
            Maksimum boyut tasarrufu. Büyük belgeler, arşiv ve web yüklemeleri için en iyisi.
          </span>
        </button>
      </div>
    </div>
  {/if}

  <!-- Sıkıştırma Sonrası: Önce - Sonra Kazanç Ekranı VEYA Güvenlik Koruması -->

  {#if result}
    {#if result.already_optimized}
      <!-- Güvenlik Koruması: Zaten Optimize Edilmiş Belge -->
      <div class="mt-4 rounded-2xl border-2 border-amber-500/30 bg-gradient-to-b from-amber-50/70 via-white to-white p-6 shadow-md dark:border-amber-500/30 dark:from-amber-950/20 dark:via-zinc-900 dark:to-zinc-900 animate-in fade-in zoom-in-95 duration-200">
        <div class="mb-5 flex flex-wrap items-center justify-between gap-3 border-b border-zinc-200/80 pb-4 dark:border-zinc-800">
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-500 text-white shadow-md shadow-amber-500/30">
              <ShieldCheck size={22} />
            </div>
            <div>
              <h3 class="text-base font-bold text-zinc-900 dark:text-zinc-100 flex items-center gap-2">
                Belge Zaten Optimize Edilmiş
                <span class="rounded-full bg-amber-100 px-2.5 py-0.5 text-[11px] font-semibold text-amber-800 dark:bg-amber-950 dark:text-amber-300">
                  Güvenlik Koruması
                </span>
              </h3>
              <p class="text-xs text-zinc-500 dark:text-zinc-400">
                Bu belge zaten maksimum düzeyde optimize edilmiş. Orijinal dosya boyutu korundu.
              </p>
            </div>
          </div>

          <div class="flex items-center gap-2">
            <button
              type="button"
              class="flex items-center gap-1.5 rounded-xl border border-zinc-300 bg-white px-3.5 py-2 text-xs font-semibold text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200 dark:hover:bg-zinc-700 transition-colors"
              on:click={clearFile}
            >
              <RefreshCw size={14} />
              Yeni Dosya
            </button>
          </div>
        </div>

        <!-- Bilgi ve Koruma Kartları -->
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 mb-2">
          <div class="rounded-xl border border-zinc-200 bg-zinc-50 p-4 dark:border-zinc-800 dark:bg-zinc-800/50">
            <p class="text-[11px] font-medium text-zinc-500 dark:text-zinc-400">Mevcut Dosya Boyutu</p>
            <p class="mt-1 text-xl font-black text-zinc-800 dark:text-zinc-100">
              {fmtSize(result.before_bytes)}
            </p>
            <p class="mt-2 text-xs text-zinc-500 dark:text-zinc-400">
              Tüm veri akışları ve nesneler zaten azami derecede sıkıştırılmış durumda.
            </p>
          </div>

          <div class="rounded-xl border border-amber-200/80 bg-amber-50/60 p-4 dark:border-amber-900/60 dark:bg-amber-950/20">
            <p class="text-[11px] font-medium text-amber-800 dark:text-amber-300 flex items-center gap-1.5">
              <ShieldCheck size={14} /> Boyut Artışı Engellendi
            </p>
            <p class="mt-1 text-xs text-amber-900/90 dark:text-amber-200/90 leading-relaxed">
              PDF nesne tabloları sebebiyle dosyanın büyümesini engellemek için işlem güvenle sonlandırıldı. Diske daha büyük bir kopya kaydedilmedi.
            </p>
          </div>
        </div>
      </div>
    {:else}
      <!-- Normal Sıkıştırma Sonucu: Önce - Sonra Kazanç Ekranı -->
      <div class="mt-4 rounded-2xl border-2 border-emerald-500/30 bg-gradient-to-b from-emerald-50/60 via-white to-white p-6 shadow-md dark:border-emerald-500/30 dark:from-emerald-950/20 dark:via-zinc-900 dark:to-zinc-900 animate-in fade-in zoom-in-95 duration-200">
        <div class="mb-5 flex flex-wrap items-center justify-between gap-3 border-b border-zinc-200/80 pb-4 dark:border-zinc-800">
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500 text-white shadow-md shadow-emerald-500/30">
              <CheckCircle2 size={22} />
            </div>
            <div>
              <h3 class="text-base font-bold text-zinc-900 dark:text-zinc-100 flex items-center gap-2">
                Sıkıştırma Tamamlandı!
                <Sparkles size={16} class="text-emerald-500" />
              </h3>
              <p class="text-xs text-zinc-500 dark:text-zinc-400">
                Dosya yapısı ve görsel netliği korunarak optimize edildi.
              </p>
            </div>
          </div>

          <div class="flex items-center gap-2">
            <button
              type="button"
              class="flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-bold text-white shadow-md shadow-indigo-600/25 hover:bg-indigo-500 transition-all active:scale-95"
              on:click={() => revealInExplorer(result.output)}
            >
              <FolderOpen size={15} />
              Klasörde Göster
            </button>
            <button
              type="button"
              class="flex items-center gap-1.5 rounded-xl border border-zinc-300 bg-white px-3.5 py-2 text-xs font-semibold text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200 dark:hover:bg-zinc-700 transition-colors"
              on:click={clearFile}
            >
              <RefreshCw size={14} />
              Yeni Dosya
            </button>
          </div>
        </div>

        <!-- Karşılaştırma Kutuları (Önce - Sonra) -->
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-3 mb-5">
          <div class="rounded-xl border border-zinc-200 bg-zinc-50 p-3.5 dark:border-zinc-800 dark:bg-zinc-800/50">
            <p class="text-[11px] font-medium text-zinc-500 dark:text-zinc-400">Önceki Boyut</p>
            <p class="mt-1 text-lg font-black text-zinc-800 dark:text-zinc-100">
              {fmtSize(result.before_bytes)}
            </p>
          </div>

          <div class="rounded-xl border border-emerald-200 bg-emerald-50/70 p-3.5 dark:border-emerald-900/50 dark:bg-emerald-950/30">
            <p class="text-[11px] font-medium text-emerald-700 dark:text-emerald-300">Yeni Boyut</p>
            <p class="mt-1 text-lg font-black text-emerald-600 dark:text-emerald-400">
              {fmtSize(result.after_bytes)}
            </p>
          </div>

          <div class="rounded-xl border border-indigo-200 bg-indigo-50/70 p-3.5 dark:border-indigo-900/50 dark:bg-indigo-950/30">
            <p class="text-[11px] font-medium text-indigo-700 dark:text-indigo-300 flex items-center gap-1">
              <ArrowDownRight size={13} /> Sağlanan Kazanç
            </p>
            <p class="mt-1 text-lg font-black text-indigo-600 dark:text-indigo-400">
              %{savedPct.toFixed(0)} Küçülme
            </p>
          </div>
        </div>

        <!-- İlerleme Çubuğu -->
        <div class="mb-3">
          <ProgressBar percent={savedPct} label={`Kazanç: %${savedPct.toFixed(0)} (${fmtSize(Math.max(0, result.before_bytes - result.after_bytes))} tasarruf)`} />
        </div>

        <!-- Çıktı Dosya Yolu -->
        <div class="flex items-center gap-2 rounded-lg bg-zinc-100 px-3 py-2 text-xs text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400">
          <span class="font-semibold text-zinc-800 dark:text-zinc-200 shrink-0">Kaydedilen Konum:</span>
          <span class="truncate font-mono text-[11px]">{result.output}</span>
        </div>
      </div>
    {/if}
  {/if}
{/if}
