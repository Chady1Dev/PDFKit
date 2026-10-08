<script lang="ts">
  import { onDestroy } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import {
    X,
    FileText,
    FileSpreadsheet,
    Presentation,
    Image,
    AlignLeft,
    RotateCw,
    Info,
  } from "@lucide/svelte";
  import type { OpenDocument } from "../docStore";
  import {
    baseName,
    writeBinaryFile,
    exportOfficeDocument,
    cancelConversion,
    type ConversionProgress,
  } from "../api";
  import { pushToast } from "../stores";
  import ActionButton from "./ActionButton.svelte";

  export let doc: OpenDocument;
  export let onClose: () => void;

  let selectedFormat: "docx" | "xlsx" | "pptx" | "jpeg" | "txt" = "docx";
  let imageSubFormat: "jpeg" | "png" = "png";
  let language = "tr";
  let busy = false;
  let progress = -1;
  let progressText = "";
  let unlistenProgress: (() => void) | null = null;

  let isCancelled = false;

  onDestroy(() => {
    if (unlistenProgress) {
      unlistenProgress();
      unlistenProgress = null;
    }
  });

  async function handleCancel() {
    if (busy) {
      isCancelled = true;
      busy = false;
      progress = -1;
      progressText = "";
      if (unlistenProgress) {
        unlistenProgress();
        unlistenProgress = null;
      }
      try {
        await cancelConversion();
      } catch (e) {
        console.error("Cancel conversion error:", e);
      }
      pushToast("Dönüştürme işlemi durduruldu ve iptal edildi", "info");
      onClose();
    } else {
      onClose();
    }
  }

  async function handleConvert() {
    if (!doc) return;
    isCancelled = false;
    busy = true;
    progress = 0;
    progressText = "Başlatılıyor...";

    try {
      unlistenProgress = await listen<ConversionProgress>("conversion-progress", (event) => {
        const p = event.payload;
        progress = p.percent;
        if (p.current > 0 && p.total > 0) {
          progressText = `Sayfa ${p.current} / ${p.total} (%${p.percent})`;
        } else {
          progressText = `Dönüştürme başlatılıyor...`;
        }
      });

      const stem = baseName(doc.path).replace(/\.pdf$/i, "");

      if (selectedFormat === "jpeg") {
        const outPath = await save({
          filters: [{ name: "Görsel", extensions: [imageSubFormat] }],
          defaultPath: `${stem}.${imageSubFormat}`,
        });
        if (!outPath) {
          busy = false;
          progress = -1;
          progressText = "";
          return;
        }

        // Render current page as sample/image
        const canvas = await doc.handle.renderPage(doc.currentPage, 3.0);
        const mime = imageSubFormat === "png" ? "image/png" : "image/jpeg";
        const blob = await new Promise<Blob | null>((res) => canvas.toBlob(res, mime, 0.95));
        if (blob) {
          const bytes = new Uint8Array(await blob.arrayBuffer());
          await writeBinaryFile(outPath, bytes);
          if (isCancelled) return;
          pushToast(`Sayfa ${doc.currentPage} görsel olarak kaydedildi: ${baseName(outPath)}`, "success", {
            label: "Klasöre Git",
            path: outPath,
          });
          onClose();
        }
      } else {
        // DOCX, XLSX, PPTX, TXT
        const ext = selectedFormat;
        const formatName =
          ext === "docx"
            ? "Microsoft Word"
            : ext === "xlsx"
            ? "Microsoft Excel"
            : ext === "pptx"
            ? "Microsoft PowerPoint"
            : "Metin Belgesi";

        const outPath = await save({
          filters: [{ name: formatName, extensions: [ext] }],
          defaultPath: `${stem}.${ext}`,
        });
        if (!outPath) {
          busy = false;
          progress = -1;
          progressText = "";
          return;
        }

        // Extract text across all pages with real-time smooth progress tracking
        const pagesText: { page: number; text: string }[] = [];
        for (let p = 1; p <= doc.pageCount; p++) {
          if (!busy || isCancelled) break;
          try {
            const t = await doc.handle.getPageText(p);
            pagesText.push({ page: p, text: t });
          } catch (err) {
            console.warn(`Sayfa ${p} metin çıkarma hatası:`, err);
            pagesText.push({ page: p, text: "" });
          }
          progress = Math.round((p / doc.pageCount) * 100);
          progressText = `Sayfa ${p} / ${doc.pageCount} dönüştürülüyor... (%${progress})`;
        }

        if (!busy || isCancelled) return;

        await exportOfficeDocument(outPath, ext, stem, pagesText, language, doc.path);

        if (!busy || isCancelled) return;

        pushToast(`${formatName} olarak başarıyla dönüştürüldü: ${baseName(outPath)}`, "success", {
          label: "Klasöre Git",
          path: outPath,
        });
        onClose();
      }
    } catch (e) {
      if (isCancelled) return;
      if (busy) {
        const msg = String(e || "");
        if (msg.includes("iptal edildi") || msg.includes("cancelled")) {
          return;
        }
        if (msg.includes("başka bir programda açık") || msg.includes("Dosya başka bir programda açık")) {
          pushToast("Dosya başka bir programda açık (!)", "error");
        } else {
          pushToast(`Dönüştürme hatası: ${msg}`, "error");
        }
      }
    } finally {
      if (!isCancelled) {
        busy = false;
        progress = -1;
        progressText = "";
        if (unlistenProgress) {
          unlistenProgress();
          unlistenProgress = null;
        }
      }
    }
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4"
  on:click|self={handleCancel}
>
  <div class="relative w-full max-w-2xl rounded-2xl border border-zinc-200 bg-white p-6 shadow-2xl dark:border-zinc-800 dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100 animate-in fade-in zoom-in-95 duration-150">
    <!-- Active Progress State (Shown exclusively while busy) -->
    {#if busy && progress >= 0}
      <div>
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-center gap-2.5 text-sm font-bold text-zinc-900 dark:text-zinc-100">
            <RotateCw size={18} class="animate-spin text-indigo-600 dark:text-indigo-400" />
            <span>{progressText || "Dönüştürülüyor..."}</span>
          </div>
          <span class="rounded-lg bg-indigo-600 px-2.5 py-1 text-xs font-bold text-white shadow-xs">
            {Math.round(progress)}%
          </span>
        </div>

        <div class="h-3 w-full overflow-hidden rounded-full bg-zinc-100 dark:bg-zinc-800">
          <div
            class="h-full rounded-full bg-gradient-to-r from-indigo-500 via-blue-500 to-indigo-600 transition-all duration-300"
            style="width: {Math.min(100, Math.max(0, progress))}%"
          ></div>
        </div>

        <div class="mt-5 flex items-center justify-between border-t border-zinc-100 pt-4 text-xs text-zinc-500 dark:border-zinc-800 dark:text-zinc-400">
          <span>İşlem büyük belgelerde daha uzun sürebilir.</span>
          <button
            type="button"
            class="rounded-xl bg-rose-600 hover:bg-rose-700 active:scale-95 px-4 py-2 font-bold text-white shadow transition-all"
            on:click={handleCancel}
          >
            Vazgeç / İptal Et
          </button>
        </div>
      </div>
    {:else}
      <!-- Header -->
      <div class="flex items-center justify-between pb-3 border-b border-zinc-100 dark:border-zinc-800">
        <h3 class="text-sm font-bold uppercase tracking-wider text-zinc-500 dark:text-zinc-400">
          Hedef Çıktı Formatı Seçin
        </h3>
        <button
          class="rounded-lg p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200 transition-colors"
          on:click={handleCancel}
          aria-label="Kapat"
        >
          <X size={18} />
        </button>
      </div>

      <!-- Format Cards Grid (Matches ImagesView styling) -->
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 my-5">
        <!-- DOCX -->
        <label
          class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all
            {selectedFormat === 'docx'
            ? 'border-blue-600 bg-blue-50/60 dark:bg-blue-950/30 ring-2 ring-blue-500/30 shadow-xs'
            : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
        >
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-2.5">
              <div class="rounded-lg bg-blue-100 p-2 text-blue-600 dark:bg-blue-900/40 dark:text-blue-400">
                <FileText size={20} />
              </div>
              <div>
                <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Microsoft Word</h5>
                <p class="text-[11px] text-zinc-500 dark:text-zinc-400">.docx formatı</p>
              </div>
            </div>
            <input type="radio" bind:group={selectedFormat} value="docx" class="accent-blue-600 h-4 w-4 mt-1" />
          </div>
          <div class="mt-3 flex items-center justify-between text-[11px]">
            <span class="text-zinc-500 dark:text-zinc-400">Gelişmiş Düzen Analizi</span>
            <span class="rounded bg-blue-100 px-2 py-0.5 font-bold text-blue-700 dark:bg-blue-900/60 dark:text-blue-300">
              %99 Uyumlu
            </span>
          </div>
        </label>

        <!-- PPTX -->
        <label
          class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all
            {selectedFormat === 'pptx'
            ? 'border-orange-600 bg-orange-50/60 dark:bg-orange-950/30 ring-2 ring-orange-500/30 shadow-xs'
            : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
        >
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-2.5">
              <div class="rounded-lg bg-orange-100 p-2 text-orange-600 dark:bg-orange-900/40 dark:text-orange-400">
                <Presentation size={20} />
              </div>
              <div>
                <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Microsoft PowerPoint</h5>
                <p class="text-[11px] text-zinc-500 dark:text-zinc-400">.pptx sunum slaytları</p>
              </div>
            </div>
            <input type="radio" bind:group={selectedFormat} value="pptx" class="accent-orange-600 h-4 w-4 mt-1" />
          </div>
          <div class="mt-3 text-[11px] text-zinc-500 dark:text-zinc-400">
            16:9 Sunum Slaytları
          </div>
        </label>

        <!-- XLSX -->
        <label
          class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all
            {selectedFormat === 'xlsx'
            ? 'border-emerald-600 bg-emerald-50/60 dark:bg-emerald-950/30 ring-2 ring-emerald-500/30 shadow-xs'
            : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
        >
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-2.5">
              <div class="rounded-lg bg-emerald-100 p-2 text-emerald-600 dark:bg-emerald-900/40 dark:text-emerald-400">
                <FileSpreadsheet size={20} />
              </div>
              <div>
                <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Microsoft Excel</h5>
                <p class="text-[11px] text-zinc-500 dark:text-zinc-400">.xlsx hesap tablosu</p>
              </div>
            </div>
            <input type="radio" bind:group={selectedFormat} value="xlsx" class="accent-emerald-600 h-4 w-4 mt-1" />
          </div>
          <div class="mt-3 text-[11px] text-zinc-500 dark:text-zinc-400">
            Tablo & Veri Ayrıştırma
          </div>
        </label>

        <!-- TXT -->
        <label
          class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all
            {selectedFormat === 'txt'
            ? 'border-indigo-600 bg-indigo-50/60 dark:bg-indigo-950/30 ring-2 ring-indigo-500/30 shadow-xs'
            : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
        >
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-2.5">
              <div class="rounded-lg bg-zinc-200 p-2 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                <AlignLeft size={20} />
              </div>
              <div>
                <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Düz Metin</h5>
                <p class="text-[11px] text-zinc-500 dark:text-zinc-400">.txt metin dosyası</p>
              </div>
            </div>
            <input type="radio" bind:group={selectedFormat} value="txt" class="accent-indigo-600 h-4 w-4 mt-1" />
          </div>
          <div class="mt-3 text-[11px] text-zinc-500 dark:text-zinc-400">
            Evrensel Metin Standartı
          </div>
        </label>

        <!-- Image (Sayfa Görseli) -->
        <label
          class="relative flex flex-col justify-between rounded-xl border p-4 cursor-pointer transition-all sm:col-span-2
            {selectedFormat === 'jpeg'
            ? 'border-violet-600 bg-violet-50/60 dark:bg-violet-950/30 ring-2 ring-violet-500/30 shadow-xs'
            : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-800 dark:hover:border-zinc-700'}"
        >
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-2.5">
              <div class="rounded-lg bg-violet-100 p-2 text-violet-600 dark:bg-violet-900/40 dark:text-violet-400">
                <Image size={20} />
              </div>
              <div>
                <h5 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">Sayfa Görseli</h5>
                <p class="text-[11px] text-zinc-500 dark:text-zinc-400">Mevcut sayfayı yüksek çözünürlüklü görsel olarak kaydet</p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <select
                bind:value={imageSubFormat}
                on:click|stopPropagation
                class="rounded-lg border border-zinc-300 bg-white px-2 py-1 text-xs font-bold text-zinc-800 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200"
              >
                <option value="png">PNG (Kayıpsız)</option>
                <option value="jpeg">JPEG (Kompakt)</option>
              </select>
              <input type="radio" bind:group={selectedFormat} value="jpeg" class="accent-violet-600 h-4 w-4 mt-1" />
            </div>
          </div>
        </label>
      </div>

      <!-- Language selector -->
      <div class="flex flex-wrap items-center justify-between gap-3 pt-4 border-t border-zinc-100 dark:border-zinc-800">
        <div class="flex items-center gap-2 text-xs text-zinc-600 dark:text-zinc-400">
          <Info size={15} />
          <span>Tanınan Belge Dili (Karakter Boşluk Düzeltmesi):</span>
        </div>
        <select
          bind:value={language}
          class="rounded-xl border border-zinc-300 bg-white px-3 py-1.5 text-xs font-semibold text-zinc-800 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200 shadow-xs"
        >
          <option value="tr">Türkçe (ç, ğ, ı, ö, ş, ü otomatik koruma)</option>
          <option value="en">İngilizce - US</option>
          <option value="de">Almanca - DE</option>
          <option value="fr">Fransızca - FR</option>
        </select>
      </div>

      <!-- Action Buttons -->
      <div class="mt-6 flex items-center justify-end gap-3 pt-4 border-t border-zinc-100 dark:border-zinc-800">
        <button
          type="button"
          class="rounded-xl border border-zinc-200 bg-zinc-50 px-4 py-2.5 text-xs font-bold text-zinc-700 hover:bg-zinc-100 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700 transition-colors"
          on:click={handleCancel}
        >
          Vazgeç
        </button>
        <ActionButton
          label="Dönüştür ve Dışa Aktar"
          icon={FileText}
          {busy}
          onClick={handleConvert}
        />
      </div>
    {/if}
  </div>
</div>
