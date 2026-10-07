<script lang="ts">
  import { onDestroy } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { AlertCircle, Stamp, Hash, Eraser, Check, Save, ShieldCheck, ChevronDown } from "@lucide/svelte";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import AnnotateDemoCard from "../components/AnnotateDemoCard.svelte";
  import { openPdf, type PdfDocHandle } from "../pdf";
  import { addWatermark, addPageNumbers, removeWatermark, cleanWatermarkBytes, writePdfBytes, baseName, type WatermarkZones } from "../api";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";

  $: if ($pendingFiles && $pendingFiles.targetView === "annotate") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      loadPdf(pdfs);
    }
    pendingFiles.set(null);
  }

  let input: string | null = null;
  let doc: PdfDocHandle | null = null;
  let demoPages: number[] = [];
  let busy = false;
  let previewMode: "watermark" | "numbers" = "watermark";
  let wmMode: "add" | "remove" = "add";

  let wmText = "GİZLİ";
  let wmSize = 48;
  let wmOpacity = 0.25;

  let numSize = 10;
  let numMargin = 24;

  let previewRenderKey = 0;
  let cleanedPreviewActive = false;
  let cleanedPdfBytes: Uint8Array | null = null;
  let showAllDeselectedWarning = false;

  // Regional Watermark Zone Selection (Default: All active)
  let zoneTop = true;
  let zoneCenter = true;
  let zoneBottom = true;

  function switchToMode(mode: "watermark" | "numbers") {
    if (previewMode === mode) return;
    if (cleanedPreviewActive && mode === "numbers") {
      pushToast(
        "Lütfen önce devam eden filigran kaldırma işlemini tamamlayınız (Kaydet veya Vazgeç seçiniz).",
        "warning"
      );
      return;
    }
    previewMode = mode;
  }

  $: allZonesSelected = zoneTop && zoneCenter && zoneBottom;
  $: noneSelected = !zoneTop && !zoneCenter && !zoneBottom;
  $: activeZoneCount = (zoneTop ? 1 : 0) + (zoneCenter ? 1 : 0) + (zoneBottom ? 1 : 0);

  async function toggleZone(zone: "top" | "center" | "bottom") {
    if (cleanedPreviewActive) {
      const isCurrentlyActive =
        zone === "top" ? zoneTop : zone === "center" ? zoneCenter : zoneBottom;

      // Önizleme modunda son kalan aktif seçimi kapatmaya izin verme (bug önleme)
      if (isCurrentlyActive && activeZoneCount <= 1) {
        showAllDeselectedWarning = true;
        return;
      }
    }

    if (zone === "top") zoneTop = !zoneTop;
    else if (zone === "center") zoneCenter = !zoneCenter;
    else if (zone === "bottom") zoneBottom = !zoneBottom;

    if (activeZoneCount > 0) {
      showAllDeselectedWarning = false;
    }

    if (cleanedPreviewActive) {
      await handlePreviewRemoveWatermark();
    }
  }

  async function toggleAllZones() {
    if (cleanedPreviewActive) {
      if (allZonesSelected) {
        showAllDeselectedWarning = true;
        return;
      }
      zoneTop = true;
      zoneCenter = true;
      zoneBottom = true;
      showAllDeselectedWarning = false;
      await handlePreviewRemoveWatermark();
      return;
    }

    if (allZonesSelected) {
      zoneTop = false;
      zoneCenter = false;
      zoneBottom = false;
    } else {
      zoneTop = true;
      zoneCenter = true;
      zoneBottom = true;
    }
  }

  const inputCls =
    "rounded-lg border border-zinc-300 bg-white px-3 py-2 text-sm text-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100";

  async function loadPdf(paths: string[]) {
    await unloadPdf();
    try {
      input = paths[0];
      doc = await openPdf(input);
      cleanedPreviewActive = false;
      cleanedPdfBytes = null;
      demoPages = Array.from({ length: Math.min(3, doc.pageCount) }, (_, i) => i + 1);
      previewRenderKey++;
    } catch (e) {
      pushToast(`PDF açılamadı: ${e}`, "error");
    }
  }

  async function unloadPdf() {
    if (doc) await doc.destroy();
    doc = null;
    demoPages = [];
    input = null;
    cleanedPreviewActive = false;
    cleanedPdfBytes = null;
  }

  onDestroy(unloadPdf);

  async function handlePreviewRemoveWatermark() {
    if (!input) return;
    if (noneSelected) {
      pushToast("Lütfen temizlemek için en az bir bölge seçiniz.", "warning");
      return;
    }
    busy = true;
    try {
      const zones: WatermarkZones = {
        top: zoneTop,
        center: zoneCenter,
        bottom: zoneBottom,
      };
      const cleanBytes = await cleanWatermarkBytes(input, null, null, zones);
      cleanedPdfBytes = cleanBytes.slice(0);
      const cleanedDoc = await openPdf(input, cleanBytes.slice(0));
      if (doc) await doc.destroy();
      doc = cleanedDoc;
      cleanedPreviewActive = true;
      demoPages = Array.from({ length: Math.min(3, doc.pageCount) }, (_, i) => i + 1);
      previewRenderKey++;
      pushToast("Filigran seçilen bölgeler için güncellendi! Canlı önizlemeyi kontrol edebilirsiniz.", "info");
    } catch (e) {
      pushToast(`Filigran temizlenirken hata oluştu: ${e}`, "error");
    } finally {
      busy = false;
    }
  }

  async function handleRevertPreview() {
    if (!input) return;
    busy = true;
    try {
      const origDoc = await openPdf(input);
      if (doc) await doc.destroy();
      doc = origDoc;
      cleanedPdfBytes = null;
      cleanedPreviewActive = false;
      showAllDeselectedWarning = false;
      demoPages = Array.from({ length: Math.min(3, doc.pageCount) }, (_, i) => i + 1);
      previewRenderKey++;
      pushToast("İşlem iptal edildi, orijinal belgeye geri dönüldü.", "info");
    } catch (e) {
      pushToast(`Geri alınamadı: ${e}`, "error");
    } finally {
      busy = false;
    }
  }

  async function handleSaveCleanedPdf() {
    if (!input) return;
    const defaultName = baseName(input).replace(/\.pdf$/i, "") + "_filigransiz.pdf";
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: defaultName,
    });
    if (!output) return;

    busy = true;
    try {
      const zones: WatermarkZones = {
        top: zoneTop,
        center: zoneCenter,
        bottom: zoneBottom,
      };
      if (cleanedPdfBytes && cleanedPdfBytes.length > 0) {
        await writePdfBytes(output, cleanedPdfBytes);
      } else {
        await removeWatermark(input, output, null, zones);
      }
      logActivity([baseName(input)], "Filigran Kaldırıldı", output);
      pushToast(`Temizlenmiş belge kaydedildi: ${baseName(output)}`, "success", {
        label: "Klasöre Git",
        path: output,
      });
      cleanedPreviewActive = false;
      cleanedPdfBytes = null;
      showAllDeselectedWarning = false;

      // Temizlenmiş yeni belgeyi ana girdi olarak yükle (böylece sayfa numarası ekleme gibi işlemler yeni belge üzerinde devam eder)
      input = output;
      const cleanDoc = await openPdf(output);
      if (doc) await doc.destroy();
      doc = cleanDoc;
      demoPages = Array.from({ length: Math.min(3, doc.pageCount) }, (_, i) => i + 1);
      previewRenderKey++;
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }

  async function run(kind: "watermark" | "numbers") {
    if (!input) return;
    const defaultName =
      kind === "watermark"
        ? baseName(input).replace(/\.pdf$/i, "") + "_filigranli.pdf"
        : baseName(input).replace(/\.pdf$/i, "") + "_numarali.pdf";
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: defaultName,
    });
    if (!output) return;
    busy = true;
    try {
      const result =
        kind === "watermark"
          ? await addWatermark(input, output, wmText, wmSize, wmOpacity)
          : await addPageNumbers(input, output, numSize, numMargin);

      logActivity([baseName(input)], kind === "watermark" ? "Filigran Eklendi" : "Sayfa Numarası Eklendi", output);
      pushToast(`Kaydedildi: ${baseName(result.outputs[0])}`, "success", {
        label: "Klasöre Git",
        path: output,
      });

      // Yeni üretilen dosyayı aktif belgeye dönüştür (art arda işlemler için)
      input = output;
      const newDoc = await openPdf(output);
      if (doc) await doc.destroy();
      doc = newDoc;
      demoPages = Array.from({ length: Math.min(3, doc.pageCount) }, (_, i) => i + 1);
      previewRenderKey++;
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<ToolHeader
  title="Filigran & Sayfa Numarası"
  description="Her sayfaya çapraz filigran veya alt-orta numara ekler ve kaldırır"
/>

{#if !doc}
  <FileDropzone extensions={["pdf"]} label="PDF dosyasını sürükleyin veya seçin" onFiles={loadPdf} />
{:else}
  {#if cleanedPreviewActive}
    <!-- Filigran Kaldırıldı Önizleme Bildirimi -->
    <div class="mb-4 flex items-center gap-2.5 rounded-xl border border-emerald-400/40 bg-emerald-500/10 px-4 py-3 text-xs font-semibold text-emerald-900 dark:border-emerald-400/20 dark:bg-emerald-400/10 dark:text-emerald-300 shadow-sm animate-in fade-in duration-200">
      <Check size={18} class="shrink-0 text-emerald-600 dark:text-emerald-400" />
      <span>FİLİGRAN TEMİZLENDİ: Belgenin temizlenmiş halini yukarıdaki kartlarda görebilirsiniz. Beğendiyseniz "Kaydet", istemezseniz "Vazgeç" butonuna tıklayabilirsiniz.</span>
    </div>
  {:else}
    <!-- Amber / Yellow Banner -->
    <div class="mb-4 flex items-center gap-2.5 rounded-xl border border-amber-400/40 bg-amber-500/10 px-4 py-3 text-xs font-semibold text-amber-900 dark:border-amber-400/20 dark:bg-amber-400/10 dark:text-amber-300 shadow-sm">
      <AlertCircle size={18} class="shrink-0 text-amber-600 dark:text-amber-400" />
      <span>DEMO GÖRÜNÜM: Filigran ve sayfa numarası modunda maksimum ilk 3 sayfa simüle edilir.</span>
    </div>
  {/if}

  <div class="mb-2 flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
    <span class="font-medium text-zinc-700 dark:text-zinc-300">{baseName(input || "")} — {doc.pageCount} sayfa</span>
    <div class="flex items-center gap-1.5">
      <span class="text-zinc-400 dark:text-zinc-500">Aktif İşlem / Önizleme:</span>
      <div class="flex rounded-md bg-zinc-200 p-0.5 dark:bg-zinc-800">
        <button
          type="button"
          class="rounded px-2.5 py-0.5 text-[11px] font-semibold transition-all {previewMode === 'watermark' ? 'bg-white text-zinc-900 shadow-xs dark:bg-zinc-700 dark:text-white' : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-800'} cursor-pointer"
          on:click={() => switchToMode("watermark")}
        >
          Filigran
        </button>
        <button
          type="button"
          class="rounded px-2.5 py-0.5 text-[11px] font-semibold transition-all {previewMode === 'numbers' ? 'bg-white text-zinc-900 shadow-xs dark:bg-zinc-700 dark:text-white' : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-800'} cursor-pointer"
          on:click={() => switchToMode("numbers")}
        >
          Sayfa Numarası
        </button>
      </div>
    </div>
  </div>

  <!-- Live Demo Preview Cards -->
  <div class="mb-6 grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3">
    {#key previewRenderKey}
      {#each demoPages as p (p)}
        <AnnotateDemoCard
          page={p}
          pageCount={doc.pageCount}
          renderPage={doc.renderPage}
          activeTab={previewMode}
          wmText={wmMode === "add" && !cleanedPreviewActive ? wmText : ""}
          {wmSize}
          {wmOpacity}
          {numSize}
          {numMargin}
        />
      {/each}
    {/key}
  </div>

  <!-- İki Ayar Kartı: Normal/Büyük Ekranlarda Yan Yana, Küçük Ekranlarda Alt Alta -->
  <!-- Akordiyon Sistemi: Sadece Biri Açık, Diğeri Kompakt Kapalı -->
  <div class="grid grid-cols-1 lg:grid-cols-2 gap-4 items-start max-w-6xl">
    <!-- 1. Ayar Kartı: Filigran İşlemleri -->
    {#if previewMode === 'watermark'}
      <!-- 1. Ayar Açık / Genişletilmiş -->
      <div
        class="flex flex-col justify-between rounded-2xl border-2 p-5 bg-white shadow-md ring-1 ring-indigo-500/30 border-indigo-500/80 dark:bg-zinc-900 transition-all"
      >
        <div class="flex flex-col gap-4">
          <!-- Kart Başlığı -->
          <div class="flex items-center justify-between border-b border-zinc-100 pb-3 dark:border-zinc-800">
            <div class="flex items-center gap-3">
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-indigo-600 text-white shadow-xs">
                {#if wmMode === 'remove'}
                  <Eraser size={18} />
                {:else}
                  <Stamp size={18} />
                {/if}
              </div>
              <div class="flex flex-col">
                <div class="flex items-center gap-2">
                  <span class="rounded-md bg-indigo-50 px-2 py-0.5 text-[11px] font-bold text-indigo-700 dark:bg-indigo-950/60 dark:text-indigo-300">
                    1. Ayar
                  </span>
                  <h3 class="font-bold text-sm text-zinc-900 dark:text-zinc-100">
                    Filigran İşlemleri
                  </h3>
                </div>
                <span class="text-xs text-zinc-500 dark:text-zinc-400">
                  {wmMode === 'remove' ? 'Bölgesel filigranları temizleyin' : 'Çapraz filigran metni ekleyin'}
                </span>
              </div>
            </div>

            <span class="flex items-center gap-1.5 rounded-full bg-indigo-50 px-2.5 py-1 text-[11px] font-bold text-indigo-700 dark:bg-indigo-950/60 dark:text-indigo-300">
              <span class="h-1.5 w-1.5 rounded-full bg-indigo-600 animate-pulse"></span>
              Aktif
            </span>
          </div>

          <!-- İç Switch / Sekmeler: Ekle | Kaldır (Geniş 2 Sütunlu Segmented Control - Kesinlikle Taşmaz) -->
          <div class="grid grid-cols-2 gap-1.5 rounded-xl bg-zinc-100 p-1.5 dark:bg-zinc-800/80">
            <button
              type="button"
              class="flex items-center justify-center gap-2 rounded-lg py-2 px-3 text-xs font-bold transition-all {wmMode === 'add' ? 'bg-white text-indigo-700 shadow-xs dark:bg-zinc-700 dark:text-white' : 'text-zinc-500 hover:text-zinc-700 dark:text-zinc-400'} {cleanedPreviewActive ? 'opacity-60 cursor-not-allowed' : 'cursor-pointer'}"
              title={cleanedPreviewActive ? "Önce filigran kaldırma işlemini tamamlayınız (Kaydet veya Vazgeç seçiniz)" : "Filigran Ekle"}
              on:click={(e) => {
                e.stopPropagation();
                if (cleanedPreviewActive) {
                  pushToast("Lütfen önce devam eden filigran kaldırma işlemini tamamlayınız (Kaydet veya Vazgeç seçiniz).", "warning");
                  return;
                }
                wmMode = 'add';
              }}
            >
              {#if cleanedPreviewActive}
                <span class="text-xs select-none" title="Engellendi">🚫</span>
              {:else}
                <Stamp size={15} />
              {/if}
              <span class="truncate">Filigran Ekle</span>
            </button>
            <button
              type="button"
              class="flex items-center justify-center gap-2 rounded-lg py-2 px-3 text-xs font-bold transition-all {wmMode === 'remove' ? 'bg-white text-rose-600 shadow-xs dark:bg-zinc-700 dark:text-rose-400' : 'text-zinc-500 hover:text-zinc-700 dark:text-zinc-400'} cursor-pointer"
              on:click={(e) => {
                e.stopPropagation();
                wmMode = 'remove';
              }}
            >
              <Eraser size={15} />
              <span class="truncate">Filigranı Kaldır</span>
            </button>
          </div>

          {#if wmMode === 'add'}
            <label class="flex flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
              Filigran Metni
              <input
                class={inputCls}
                bind:value={wmText}
                placeholder="Örn: GİZLİ, ÖRNEK, TASLAK"
              />
            </label>

            <div class="flex gap-3">
              <label class="flex flex-1 flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
                Yazı Boyutu ({wmSize} pt)
                <input
                  type="range"
                  min="16"
                  max="120"
                  class="accent-indigo-600"
                  bind:value={wmSize}
                />
              </label>
              <label class="flex flex-1 flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
                Opaklık (%{Math.round(wmOpacity * 100)})
                <input
                  type="range"
                  min="0.05"
                  max="0.8"
                  step="0.05"
                  class="accent-indigo-600"
                  bind:value={wmOpacity}
                />
              </label>
            </div>
            <p class="text-[11px] text-zinc-400 dark:text-zinc-500">
              Tüm sayfalara 45° açıyla yarı saydam filigran metni ekler.
            </p>
          {:else}
            <!-- Filigranı Kaldır sekmesi: Bölgesel Seçicilik Düğmeleri (Zone Toggles) -->
            <div class="space-y-3">
              <div class="flex items-center justify-between pb-1">
                <span class="text-xs font-bold text-zinc-800 dark:text-zinc-200">
                  Temizlenecek Bölgeler:
                </span>
                <button
                  type="button"
                  class="flex items-center gap-1 rounded-lg px-2.5 py-1 text-[11px] font-bold transition-all cursor-pointer {allZonesSelected ? 'bg-indigo-600 text-white shadow-xs' : 'bg-zinc-200 text-zinc-700 hover:bg-zinc-300 dark:bg-zinc-800 dark:text-zinc-300'}"
                  on:click={(e) => { e.stopPropagation(); toggleAllZones(); }}
                >
                  <span>{allZonesSelected ? '✓ Tümü Seçili' : 'Tümünü Seç'}</span>
                </button>
              </div>

              {#if showAllDeselectedWarning && cleanedPreviewActive}
                <!-- Sadece kullanıcı tüm seçimleri kapatmak istediğinde açılan uyarı -->
                <div class="flex items-center justify-between gap-2 rounded-xl border border-amber-400/40 bg-amber-500/10 p-2.5 text-xs font-semibold text-amber-900 dark:border-amber-400/20 dark:bg-amber-400/10 dark:text-amber-300 animate-in fade-in duration-150">
                  <div class="flex items-center gap-2">
                    <AlertCircle size={15} class="shrink-0 text-amber-600 dark:text-amber-400" />
                    <span>Önizleme modunda tüm seçimler kapatılamaz (en az 1 bölge seçili kalmalıdır). Orijinal belgeye dönmek için <strong>Vazgeç</strong> butonunu kullanabilirsiniz.</span>
                  </div>
                  <button
                    type="button"
                    class="text-amber-700 hover:text-amber-900 dark:text-amber-400 text-xs px-1 cursor-pointer"
                    on:click={(e) => { e.stopPropagation(); showAllDeselectedWarning = false; }}
                  >
                    ✕
                  </button>
                </div>
              {/if}

              <!-- 3 Bölgesel Switch Kartı -->
              <div class="flex flex-col gap-2">
                <!-- 1. Üst Bölge (Header) -->
                <div
                  class="flex items-center justify-between rounded-xl border p-2.5 transition-all {zoneTop ? 'border-indigo-400/60 bg-indigo-50/50 dark:border-indigo-500/40 dark:bg-indigo-950/20' : 'border-zinc-200 bg-zinc-50/50 opacity-75 dark:border-zinc-800 dark:bg-zinc-800/40'}"
                >
                  <div class="flex flex-col pr-2">
                    <div class="flex items-center gap-1.5">
                      <span class="rounded bg-indigo-100 px-1.5 py-0.5 text-[10px] font-bold text-indigo-700 dark:bg-indigo-900/60 dark:text-indigo-300">
                        ÜST
                      </span>
                      <span class="text-xs font-semibold text-zinc-900 dark:text-zinc-100">
                        Üst Bölge (Header)
                      </span>
                    </div>
                    <span class="text-[11px] text-zinc-500 dark:text-zinc-400">
                      Üst duyuru, link (URL) ve sistem bildirimleri
                    </span>
                  </div>
                  <!-- iOS Style Toggle Switch -->
                  <button
                    type="button"
                    role="switch"
                    aria-checked={zoneTop}
                    aria-label="Üst Bölge filigran temizliğini aç/kapat"
                    title={cleanedPreviewActive && activeZoneCount <= 1 && zoneTop ? "Önizleme modunda son aktif bölge kapatılamaz (Vazgeç butonunu kullanınız)" : "Üst Bölge (Header)"}
                    class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none {zoneTop ? 'bg-indigo-600' : 'bg-zinc-300 dark:bg-zinc-700'}"
                    on:click={(e) => { e.stopPropagation(); toggleZone('top'); }}
                  >
                    <span
                      class="pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {zoneTop ? 'translate-x-4' : 'translate-x-0'}"
                    ></span>
                  </button>
                </div>

                <!-- 2. Orta / Çapraz Gövde (Center / Diagonal) -->
                <div
                  class="flex items-center justify-between rounded-xl border p-2.5 transition-all {zoneCenter ? 'border-indigo-400/60 bg-indigo-50/50 dark:border-indigo-500/40 dark:bg-indigo-950/20' : 'border-zinc-200 bg-zinc-50/50 opacity-75 dark:border-zinc-800 dark:bg-zinc-800/40'}"
                >
                  <div class="flex flex-col pr-2">
                    <div class="flex items-center gap-1.5">
                      <span class="rounded bg-rose-100 px-1.5 py-0.5 text-[10px] font-bold text-rose-700 dark:bg-rose-900/60 dark:text-rose-300">
                        ORTA
                      </span>
                      <span class="text-xs font-semibold text-zinc-900 dark:text-zinc-100">
                        Orta / Çapraz Gövde
                      </span>
                    </div>
                    <span class="text-[11px] text-zinc-500 dark:text-zinc-400">
                      45° çapraz yazılar, filigran damgaları ve şeffaf katmanlar
                    </span>
                  </div>
                  <!-- iOS Style Toggle Switch -->
                  <button
                    type="button"
                    role="switch"
                    aria-checked={zoneCenter}
                    aria-label="Orta ve çapraz filigran temizliğini aç/kapat"
                    title={cleanedPreviewActive && activeZoneCount <= 1 && zoneCenter ? "Önizleme modunda son aktif bölge kapatılamaz (Vazgeç butonunu kullanınız)" : "Orta / Çapraz Gövde"}
                    class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none {zoneCenter ? 'bg-indigo-600' : 'bg-zinc-300 dark:bg-zinc-700'}"
                    on:click={(e) => { e.stopPropagation(); toggleZone('center'); }}
                  >
                    <span
                      class="pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {zoneCenter ? 'translate-x-4' : 'translate-x-0'}"
                    ></span>
                  </button>
                </div>

                <!-- 3. Alt Bölge (Footer) -->
                <div
                  class="flex items-center justify-between rounded-xl border p-2.5 transition-all {zoneBottom ? 'border-indigo-400/60 bg-indigo-50/50 dark:border-indigo-500/40 dark:bg-indigo-950/20' : 'border-zinc-200 bg-zinc-50/50 opacity-75 dark:border-zinc-800 dark:bg-zinc-800/40'}"
                >
                  <div class="flex flex-col pr-2">
                    <div class="flex items-center gap-1.5">
                      <span class="rounded bg-amber-100 px-1.5 py-0.5 text-[10px] font-bold text-amber-800 dark:bg-amber-900/60 dark:text-amber-300">
                        ALT
                      </span>
                      <span class="text-xs font-semibold text-zinc-900 dark:text-zinc-100">
                        Alt Bölge (Footer)
                      </span>
                    </div>
                    <span class="text-[11px] text-zinc-500 dark:text-zinc-400">
                      Telif hakkı, kanuni uyarı ve yasal sorumluluk notları
                    </span>
                  </div>
                  <!-- iOS Style Toggle Switch -->
                  <button
                    type="button"
                    role="switch"
                    aria-checked={zoneBottom}
                    aria-label="Alt Bölge filigran temizliğini aç/kapat"
                    title={cleanedPreviewActive && activeZoneCount <= 1 && zoneBottom ? "Önizleme modunda son aktif bölge kapatılamaz (Vazgeç butonunu kullanınız)" : "Alt Bölge (Footer)"}
                    class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none {zoneBottom ? 'bg-indigo-600' : 'bg-zinc-300 dark:bg-zinc-700'}"
                    on:click={(e) => { e.stopPropagation(); toggleZone('bottom'); }}
                  >
                    <span
                      class="pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out {zoneBottom ? 'translate-x-4' : 'translate-x-0'}"
                    ></span>
                  </button>
                </div>
              </div>

              <!-- Akıllı Koruma Güvencesi -->
              <div class="flex items-center gap-1.5 pt-1 text-[11px] font-medium text-emerald-700 dark:text-emerald-400">
                <ShieldCheck size={14} class="shrink-0" />
                <span>Akıllı Koruma: Sorular, kutular ve cevap şıkları asla silinmez.</span>
              </div>
            </div>
          {/if}
        </div>

        <!-- Kartın Altındaki Aksiyon Butonu / Butonları -->
        <div class="pt-4 mt-3 border-t border-zinc-100 dark:border-zinc-800">
          {#if wmMode === 'add'}
            <ActionButton label="Filigranı PDF'e Uygula ve Kaydet" {busy} disabled={!wmText.trim()} onClick={() => run("watermark")} />
          {:else}
            {#if !cleanedPreviewActive}
              <button
                type="button"
                class="flex items-center justify-center gap-2 w-full rounded-xl bg-rose-600 py-2.5 text-xs font-bold text-white shadow-md shadow-rose-600/25 hover:bg-rose-500 disabled:opacity-50 transition-all active:scale-95 cursor-pointer"
                disabled={busy || noneSelected}
                title={noneSelected ? "En az 1 bölge seçilmelidir" : "Filigranı Kaldır"}
                on:click={handlePreviewRemoveWatermark}
              >
                <Eraser size={15} />
                {busy ? "Filigran Temizleniyor..." : "Filigranı Kaldır"}
              </button>
            {:else}
              <!-- Onay Aşamasında: Vazgeç ve Kaydet Butonları -->
              <div class="flex items-center gap-2.5">
                <button
                  type="button"
                  class="flex-1 rounded-xl border border-zinc-300 py-2.5 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
                  disabled={busy}
                  on:click={handleRevertPreview}
                >
                  Vazgeç
                </button>
                <button
                  type="button"
                  class="flex-1 flex items-center justify-center gap-1.5 rounded-xl bg-emerald-600 py-2.5 text-xs font-bold text-white shadow-md shadow-emerald-600/25 hover:bg-emerald-500 disabled:opacity-50 transition-all active:scale-95 cursor-pointer"
                  disabled={busy}
                  on:click={handleSaveCleanedPdf}
                >
                  <Save size={15} /> Kaydet
                </button>
              </div>
            {/if}
          {/if}
        </div>
      </div>
    {:else}
      <!-- 1. Ayar Kapalı / Kompakt Akordiyon Kartı -->
      <div
        role="button"
        tabindex="0"
        class="group flex flex-col justify-between rounded-2xl border-2 p-5 bg-zinc-50/70 hover:bg-white hover:border-indigo-400/70 dark:bg-zinc-900/50 dark:hover:bg-zinc-900 border-zinc-200 dark:border-zinc-800 transition-all cursor-pointer shadow-xs hover:shadow-md"
        on:click={() => switchToMode('watermark')}
        on:keydown={(e) => e.key === 'Enter' && switchToMode('watermark')}
      >
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-zinc-200/80 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400 group-hover:bg-indigo-600 group-hover:text-white transition-colors">
              {#if wmMode === 'remove'}
                <Eraser size={18} />
              {:else}
                <Stamp size={18} />
              {/if}
            </div>
            <div class="flex flex-col">
              <div class="flex items-center gap-2">
                <span class="rounded-md bg-zinc-200/80 px-2 py-0.5 text-[11px] font-bold text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400">
                  1. Ayar
                </span>
                <h3 class="font-bold text-sm text-zinc-700 dark:text-zinc-300 group-hover:text-zinc-900 dark:group-hover:text-white transition-colors">
                  Filigran İşlemleri
                </h3>
              </div>
              <span class="text-xs text-zinc-500 dark:text-zinc-400">
                {wmMode === 'remove' ? 'Bölgesel filigranları temizleyin' : 'Çapraz filigran metni ekleyin'}
              </span>
            </div>
          </div>

          <div class="flex items-center gap-1.5 rounded-lg bg-zinc-200/70 px-3 py-1.5 text-xs font-bold text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 group-hover:bg-indigo-600 group-hover:text-white transition-all shrink-0">
            <span>Aç</span>
            <ChevronDown size={14} />
          </div>
        </div>
      </div>
    {/if}

    <!-- 2. Ayar Kartı: Sayfa Numarası Ayarları -->
    {#if previewMode === 'numbers'}
      <!-- 2. Ayar Açık / Genişletilmiş -->
      <div
        class="flex flex-col justify-between rounded-2xl border-2 p-5 bg-white shadow-md ring-1 ring-indigo-500/30 border-indigo-500/80 dark:bg-zinc-900 transition-all"
      >
        <div class="flex flex-col gap-4">
          <!-- Kart Başlığı -->
          <div class="flex items-center justify-between border-b border-zinc-100 pb-3 dark:border-zinc-800">
            <div class="flex items-center gap-3">
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-indigo-600 text-white shadow-xs">
                <Hash size={18} />
              </div>
              <div class="flex flex-col">
                <div class="flex items-center gap-2">
                  <span class="rounded-md bg-indigo-50 px-2 py-0.5 text-[11px] font-bold text-indigo-700 dark:bg-indigo-950/60 dark:text-indigo-300">
                    2. Ayar
                  </span>
                  <h3 class="font-bold text-sm text-zinc-900 dark:text-zinc-100">
                    Sayfa Numarası Ayarları
                  </h3>
                </div>
                <span class="text-xs text-zinc-500 dark:text-zinc-400">
                  Alt-orta kısma sayfa numarası (Örn: 1 / N) ekleyin
                </span>
              </div>
            </div>

            <span class="flex items-center gap-1.5 rounded-full bg-indigo-50 px-2.5 py-1 text-[11px] font-bold text-indigo-700 dark:bg-indigo-950/60 dark:text-indigo-300">
              <span class="h-1.5 w-1.5 rounded-full bg-indigo-600 animate-pulse"></span>
              Aktif
            </span>
          </div>

          <div class="flex flex-col gap-3">
            <div class="flex gap-3">
              <label class="flex flex-1 flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
                Yazı Boyutu ({numSize} pt)
                <input
                  type="range"
                  min="8"
                  max="24"
                  class="accent-indigo-600"
                  bind:value={numSize}
                />
              </label>
              <label class="flex flex-1 flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
                Alt Boşluk ({numMargin} pt)
                <input
                  type="range"
                  min="10"
                  max="80"
                  class="accent-indigo-600"
                  bind:value={numMargin}
                />
              </label>
            </div>
            <p class="text-[11px] text-zinc-400 dark:text-zinc-500">
              Biçim: "1 / {doc.pageCount}" şeklinde sayfa altında ortalanır.
            </p>

            {#if cleanedPreviewActive}
              <p class="text-[11px] text-amber-600 dark:text-amber-400 font-medium">
                * Sayfa numarası eklemeden önce devam eden filigran temizleme işlemini (Kaydet veya Vazgeç) tamamlayınız.
              </p>
            {/if}
          </div>
        </div>

        <!-- Kartın Altındaki Aksiyon Butonu -->
        <div class="pt-4 mt-3 border-t border-zinc-100 dark:border-zinc-800">
          <ActionButton
            label="Sayfa Numarasını Uygula ve Kaydet"
            {busy}
            disabled={cleanedPreviewActive}
            onClick={() => run("numbers")}
          />
        </div>
      </div>
    {:else}
      <!-- 2. Ayar Kapalı / Kompakt Akordiyon Kartı -->
      <div
        role="button"
        tabindex="0"
        class="group flex flex-col justify-between rounded-2xl border-2 p-5 bg-zinc-50/70 hover:bg-white hover:border-indigo-400/70 dark:bg-zinc-900/50 dark:hover:bg-zinc-900 border-zinc-200 dark:border-zinc-800 transition-all cursor-pointer shadow-xs hover:shadow-md"
        on:click={() => switchToMode('numbers')}
        on:keydown={(e) => e.key === 'Enter' && switchToMode('numbers')}
      >
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-zinc-200/80 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400 group-hover:bg-indigo-600 group-hover:text-white transition-colors">
              <Hash size={18} />
            </div>
            <div class="flex flex-col">
              <div class="flex items-center gap-2">
                <span class="rounded-md bg-zinc-200/80 px-2 py-0.5 text-[11px] font-bold text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400">
                  2. Ayar
                </span>
                <h3 class="font-bold text-sm text-zinc-700 dark:text-zinc-300 group-hover:text-zinc-900 dark:group-hover:text-white transition-colors">
                  Sayfa Numarası Ayarları
                </h3>
              </div>
              <span class="text-xs text-zinc-500 dark:text-zinc-400">
                Alt-orta kısma sayfa numarası (Örn: 1 / N) ekleyin
              </span>
            </div>
          </div>

          {#if cleanedPreviewActive}
            <span class="rounded-full bg-amber-500/10 px-2.5 py-1 text-[11px] font-bold text-amber-700 dark:bg-amber-400/10 dark:text-amber-300">
              ⚡ Filigran İşlemi Sürüyor
            </span>
          {:else}
            <div class="flex items-center gap-1.5 rounded-lg bg-zinc-200/70 px-3 py-1.5 text-xs font-bold text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 group-hover:bg-indigo-600 group-hover:text-white transition-all shrink-0">
              <span>Aç</span>
              <ChevronDown size={14} />
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
{/if}
