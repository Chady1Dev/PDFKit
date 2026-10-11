<script lang="ts">
  import { onDestroy } from "svelte";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import {
    FileSignature,
    Check,
    Square,
    CheckSquare,
    Search,
    RotateCcw,
    Stamp,
    PenTool,
    Upload,
    X,
    FileText,
    CircleDot,
    SlidersHorizontal,
    Trash2,
  } from "@lucide/svelte";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import PdfPreviewPane from "../components/PdfPreviewPane.svelte";
  import SignaturePadModal from "../components/SignaturePadModal.svelte";
  import { openPdf, type PdfDocHandle } from "../pdf";
  import {
    listFormFields,
    fillForm,
    stampImage,
    writeBinaryFile,
    baseName,
    type FormField,
  } from "../api";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";

  $: if ($pendingFiles && $pendingFiles.targetView === "forms") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      load(pdfs);
    }
    pendingFiles.set(null);
  }

  let input: string | null = null;
  let doc: PdfDocHandle | null = null;
  let fields: FormField[] = [];
  let fieldValues: Record<string, string> = {};
  let initialFieldValues: Record<string, string> = {};
  let busy = false;

  // Tabs & Filters
  let activeTab: "forms" | "signature" = "forms";
  let activePage = 1;
  let searchQuery = "";
  let typeFilter: "all" | "checkbox" | "radio" | "text" = "all";
  let highlightedField: string | null = null;

  // Signature stamp state
  let showSignatureModal = false;
  let sigImagePath: string | null = null;
  let sigPreviewUrl: string | null = null;
  let sigPage = 1;
  let sigX = 350;
  let sigY = 80;
  let sigW = 160;
  let sigH = 65;

  const inputCls =
    "w-full rounded-xl border border-zinc-200 bg-white px-3 py-2 text-xs text-zinc-900 transition-colors focus:border-indigo-500 focus:outline-none dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-100 shadow-2xs";

  async function load(paths: string[]) {
    await unload();
    input = paths[0];
    fields = [];
    fieldValues = {};
    initialFieldValues = {};

    try {
      doc = await openPdf(input);
      fields = await listFormFields(input);

      // Populate initial values
      for (const f of fields) {
        fieldValues[f.name] = f.value || "";
        initialFieldValues[f.name] = f.value || "";
      }

      if (fields.length === 0) {
        pushToast("Bu PDF belgesinde doldurulabilir AcroForm alanı bulunamadı.", "info");
      }
    } catch (e) {
      pushToast(`PDF açılamadı: ${e}`, "error");
    }
  }

  async function unload() {
    if (doc) {
      await doc.destroy();
      doc = null;
    }
    input = null;
    fields = [];
    fieldValues = {};
    initialFieldValues = {};
    highlightedField = null;
    sigImagePath = null;
    sigPreviewUrl = null;
  }

  onDestroy(unload);

  // Field type counts for filter pills
  $: checkboxCount = fields.filter((f) => f.field_type === "checkbox").length;
  $: radioCount = fields.filter((f) => f.field_type === "radio").length;
  $: textCount = fields.filter((f) => f.field_type === "text" || f.field_type === "multiline").length;

  // Filtered fields based on search & category
  $: filteredFields = fields.filter((f) => {
    const matchesSearch =
      !searchQuery ||
      f.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (f.value && f.value.toLowerCase().includes(searchQuery.toLowerCase()));

    if (!matchesSearch) return false;

    if (typeFilter === "checkbox") return f.field_type === "checkbox";
    if (typeFilter === "radio") return f.field_type === "radio";
    if (typeFilter === "text") return f.field_type === "text" || f.field_type === "multiline";

    return true;
  });

  function isCheckboxChecked(f: FormField): boolean {
    const val = fieldValues[f.name];
    if (val === undefined) {
      const initial = (f.value || "").trim();
      return (
        initial === "On" ||
        initial === "Yes" ||
        initial === "1" ||
        initial === "true" ||
        (f.export_value !== null && f.export_value !== undefined && initial === f.export_value)
      );
    }
    const norm = (val || "").trim().toLowerCase();
    if (norm === "off" || norm === "false" || norm === "0" || norm === "" || norm === "/") {
      return false;
    }
    return true;
  }

  function toggleCheckbox(f: FormField) {
    const currentlyChecked = isCheckboxChecked(f);
    const onVal = f.export_value || "On";
    fieldValues[f.name] = currentlyChecked ? "Off" : onVal;
    fieldValues = { ...fieldValues };
  }

  function selectRadioOption(f: FormField, opt: string) {
    fieldValues[f.name] = opt;
    fieldValues = { ...fieldValues };
  }

  function resetFormFields() {
    fieldValues = { ...initialFieldValues };
    pushToast("Form alanları belgedeki ilk haline döndürüldü.", "info");
  }

  function clearAllFields() {
    for (const f of fields) {
      if (f.field_type === "checkbox") {
        fieldValues[f.name] = "Off";
      } else {
        fieldValues[f.name] = "";
      }
    }
    fieldValues = { ...fieldValues };
    pushToast("Tüm form alanları temizlendi.", "info");
  }

  function focusField(fieldName: string) {
    highlightedField = fieldName;
    const el = document.getElementById(`field-card-${fieldName}`);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }

  async function runFill() {
    if (!input) return;
    const defaultName = baseName(input).replace(/\.pdf$/i, "") + "_doldurulmus.pdf";
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: defaultName,
    });
    if (!output) return;

    busy = true;
    try {
      const result = await fillForm(input, output, fieldValues);
      logActivity([baseName(input)], "Form Dolduruldu", output);
      pushToast(`Doldurulmuş form kaydedildi: ${baseName(result.outputs[0])}`, "success", {
        label: "Klasöre Git",
        path: output,
      });

      // Reload newly filled PDF
      await load([output]);
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }

  async function pickSignatureFile() {
    const selected = await open({
      filters: [{ name: "Görsel Dosyaları", extensions: ["png", "jpg", "jpeg"] }],
    });
    if (typeof selected === "string") {
      sigImagePath = selected;
      sigPreviewUrl = null;
      activeTab = "signature";
    }
  }

  async function handleSaveDrawnSignature(dataUrl: string) {
    showSignatureModal = false;
    try {
      // Decode data URL to bytes
      const base64 = dataUrl.split(",")[1];
      const binaryStr = atob(base64);
      const len = binaryStr.length;
      const bytes = new Uint8Array(len);
      for (let i = 0; i < len; i++) {
        bytes[i] = binaryStr.charCodeAt(i);
      }

      // Write signature image to a temp location
      const tempSigPath = `${input || "doc"}_temp_signature_${Date.now()}.png`;
      await writeBinaryFile(tempSigPath, bytes);

      sigImagePath = tempSigPath;
      sigPreviewUrl = dataUrl;
      activeTab = "signature";
      pushToast("Çizilen imza başarıyla hazırlandı.", "success");
    } catch (e) {
      pushToast(`İmza kaydedilemedi: ${e}`, "error");
    }
  }

  async function runStamp() {
    if (!input || !sigImagePath) {
      pushToast("Lütfen önce bir imza görseli seçin veya çizin.", "warning");
      return;
    }

    const defaultName = baseName(input).replace(/\.pdf$/i, "") + "_imzali.pdf";
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: defaultName,
    });
    if (!output) return;

    busy = true;
    try {
      const result = await stampImage(
        input,
        output,
        sigPage,
        sigImagePath,
        sigX,
        sigY,
        sigW,
        sigH
      );
      logActivity([baseName(input)], "İmza Damgalandı", output);
      pushToast(`İmzalı belge kaydedildi: ${baseName(result.outputs[0])}`, "success", {
        label: "Klasöre Git",
        path: output,
      });

      // Reload stamped PDF
      await load([output]);
    } catch (e) {
      pushToast(`Damgalama hatası: ${e}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<div class="flex flex-col gap-4">
  <ToolHeader
    title="Form Doldur & İmza"
    description="AcroForm alanlarını ve onay kutularını doldurun, canlı önizleme üzerinden imzanızı damgalayın"
  />

  {#if !input}
    <FileDropzone extensions={["pdf"]} label="Formlu veya İmzalanacak PDF'i sürükleyin veya seçin" onFiles={load} />
  {:else}
    <!-- Top Document Status & Action Bar -->
    <div class="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-zinc-200 bg-white px-4 py-3 dark:border-zinc-800 dark:bg-zinc-900 shadow-xs">
      <div class="flex items-center gap-3">
        <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 dark:bg-indigo-950/60 dark:text-indigo-400">
          <FileSignature size={20} />
        </div>
        <div class="flex flex-col">
          <span class="text-xs font-bold text-zinc-900 dark:text-zinc-100 truncate max-w-sm">
            {baseName(input)}
          </span>
          <span class="text-[11px] text-zinc-500 dark:text-zinc-400">
            {doc ? doc.pageCount : 1} sayfa • {fields.length} form alanı tespit edildi
          </span>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <!-- Segmented Tab Switcher -->
        <div class="flex rounded-xl bg-zinc-100 p-1 dark:bg-zinc-800">
          <button
            type="button"
            class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-semibold transition-all {activeTab === 'forms' ? 'bg-white text-indigo-700 shadow-xs dark:bg-zinc-700 dark:text-white' : 'text-zinc-600 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-200'} cursor-pointer"
            on:click={() => (activeTab = "forms")}
          >
            <FileText size={14} />
            <span>Form Alanları ({fields.length})</span>
          </button>
          <button
            type="button"
            class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-semibold transition-all {activeTab === 'signature' ? 'bg-white text-purple-700 shadow-xs dark:bg-zinc-700 dark:text-white' : 'text-zinc-600 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-200'} cursor-pointer"
            on:click={() => (activeTab = "signature")}
          >
            <PenTool size={14} />
            <span>İmza Damgası</span>
          </button>
        </div>

        <button
          type="button"
          class="rounded-xl border border-zinc-200 px-3 py-1.5 text-xs font-semibold text-zinc-600 hover:bg-zinc-50 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
          on:click={unload}
        >
          Farklı PDF Aç
        </button>
      </div>
    </div>

    <!-- 50% / 50% Split Layout -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4 items-start">
      <!-- SOL TARAF: Canlı PDF Önizleme Penceresi (50%) -->
      <div class="h-[calc(100vh-210px)] min-h-[580px] sticky top-4">
        <PdfPreviewPane
          {doc}
          bind:currentPage={activePage}
          mode={activeTab}
          highlightFieldName={highlightedField}
          {sigPage}
          {sigX}
          {sigY}
          {sigW}
          {sigH}
          sigImage={sigPreviewUrl || sigImagePath}
          bind:formValues={fieldValues}
          onFieldValueChange={(fieldName, val) => {
            fieldValues[fieldName] = val;
            fieldValues = { ...fieldValues };
          }}
          onSigChange={(pos) => {
            sigX = pos.x;
            sigY = pos.y;
            sigW = pos.width;
            sigH = pos.height;
            sigPage = pos.page;
          }}
          onFieldClick={(name) => focusField(name)}
        />
      </div>

      <!-- SAĞ TARAF: Form Alanları veya İmza Yönetimi (50%) -->
      <div class="flex flex-col h-[calc(100vh-210px)] min-h-[580px] rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900 shadow-xs overflow-hidden">
        {#if activeTab === "forms"}
          <!-- TAB 1: FORM ALANLARI DOLDURMA -->
          <div class="flex items-center justify-between border-b border-zinc-100 pb-3 dark:border-zinc-800">
            <div>
              <h3 class="font-bold text-sm text-zinc-900 dark:text-zinc-100">
                Form Alanlarını Doldur
              </h3>
              <p class="text-[11px] text-zinc-500 dark:text-zinc-400">
                Onay kutularını (checkbox), radyo butonlarını ve metin alanlarını düzenleyin
              </p>
            </div>

            <div class="flex items-center gap-1.5">
              <button
                type="button"
                class="flex items-center gap-1 rounded-lg px-2.5 py-1 text-[11px] font-semibold text-zinc-500 hover:bg-zinc-100 hover:text-zinc-800 dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
                title="Tüm alanları boşalt"
                on:click={clearAllFields}
              >
                <Trash2 size={13} />
                <span>Temizle</span>
              </button>
              <button
                type="button"
                class="flex items-center gap-1 rounded-lg px-2.5 py-1 text-[11px] font-semibold text-zinc-500 hover:bg-zinc-100 hover:text-zinc-800 dark:text-zinc-400 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
                title="İlk değerlere dön"
                on:click={resetFormFields}
              >
                <RotateCcw size={13} />
                <span>Sıfırla</span>
              </button>
            </div>
          </div>

          <!-- Search & Filter Bar -->
          <div class="mt-3 flex flex-col gap-2">
            <div class="relative">
              <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400" />
              <input
                type="text"
                placeholder="Alan adı veya değer ara..."
                bind:value={searchQuery}
                class="w-full rounded-xl border border-zinc-200 bg-zinc-50/70 pl-8 pr-3 py-1.5 text-xs text-zinc-900 focus:border-indigo-500 focus:bg-white focus:outline-none dark:border-zinc-700 dark:bg-zinc-800/80 dark:text-zinc-100"
              />
              {#if searchQuery}
                <button
                  type="button"
                  class="absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 cursor-pointer"
                  on:click={() => (searchQuery = "")}
                >
                  <X size={13} />
                </button>
              {/if}
            </div>

            <!-- Type Filter Tabs -->
            <div class="flex items-center gap-1 overflow-x-auto pb-1 text-[11px]">
              <button
                type="button"
                class="rounded-lg px-2.5 py-1 font-semibold transition-all {typeFilter === 'all' ? 'bg-indigo-600 text-white' : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-300'} cursor-pointer"
                on:click={() => (typeFilter = "all")}
              >
                Tümü ({fields.length})
              </button>
              {#if checkboxCount > 0}
                <button
                  type="button"
                  class="rounded-lg px-2.5 py-1 font-semibold transition-all {typeFilter === 'checkbox' ? 'bg-indigo-600 text-white' : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-300'} cursor-pointer"
                  on:click={() => (typeFilter = "checkbox")}
                >
                  Onay Kutuları ({checkboxCount})
                </button>
              {/if}
              {#if radioCount > 0}
                <button
                  type="button"
                  class="rounded-lg px-2.5 py-1 font-semibold transition-all {typeFilter === 'radio' ? 'bg-indigo-600 text-white' : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-300'} cursor-pointer"
                  on:click={() => (typeFilter = "radio")}
                >
                  Radyo Grupları ({radioCount})
                </button>
              {/if}
              {#if textCount > 0}
                <button
                  type="button"
                  class="rounded-lg px-2.5 py-1 font-semibold transition-all {typeFilter === 'text' ? 'bg-indigo-600 text-white' : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-300'} cursor-pointer"
                  on:click={() => (typeFilter = "text")}
                >
                  Metin ({textCount})
                </button>
              {/if}
            </div>
          </div>

          <!-- Form Fields List (Scrollable Area) -->
          <div class="mt-3 flex-1 overflow-y-auto pr-1 space-y-2.5">
            {#if filteredFields.length === 0}
              <div class="flex flex-col items-center justify-center h-48 text-zinc-400 gap-1">
                <Search size={24} class="opacity-40" />
                <span class="text-xs">Kriterlere uygun form alanı bulunamadı</span>
              </div>
            {:else}
              {#each filteredFields as f (f.name)}
                <!-- svelte-ignore a11y-no-static-element-interactions -->
                <div
                  id={`field-card-${f.name}`}
                  class="group flex flex-col gap-1.5 rounded-xl border p-3 transition-all {highlightedField === f.name ? 'border-indigo-500 bg-indigo-50/20 ring-1 ring-indigo-500/40 dark:bg-indigo-950/20' : 'border-zinc-200/90 bg-zinc-50/50 hover:bg-white hover:border-zinc-300 dark:border-zinc-800 dark:bg-zinc-900/60 dark:hover:bg-zinc-900'}"
                  on:mouseenter={() => (highlightedField = f.name)}
                  on:mouseleave={() => (highlightedField = null)}
                >
                  <!-- Header: Name & Type Badge -->
                  <div class="flex items-center justify-between gap-2">
                    <span class="text-xs font-semibold text-zinc-800 dark:text-zinc-200 truncate select-all">
                      {f.name}
                    </span>
                    <span class="shrink-0 rounded-md px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider {f.field_type === 'checkbox' ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-950/60 dark:text-emerald-300' : f.field_type === 'radio' ? 'bg-purple-50 text-purple-700 dark:bg-purple-950/60 dark:text-purple-300' : f.field_type === 'pushbutton' ? 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400' : 'bg-blue-50 text-blue-700 dark:bg-blue-950/60 dark:text-blue-300'}">
                      {f.field_type === 'checkbox' ? 'Onay Kutusu' : f.field_type === 'radio' ? 'Radyo Grubu' : f.field_type === 'pushbutton' ? 'Buton' : 'Metin Alanı'}
                    </span>
                  </div>

                  <!-- Control depending on field_type -->
                  {#if f.field_type === "checkbox"}
                    <!-- Doğru ve Modern Checkbox / Toggle Denetimi -->
                    <button
                      type="button"
                      class="flex items-center gap-2.5 rounded-lg border p-2 text-left transition-all cursor-pointer {isCheckboxChecked(f) ? 'border-emerald-300 bg-emerald-50/50 text-emerald-900 dark:border-emerald-800/80 dark:bg-emerald-950/30 dark:text-emerald-200' : 'border-zinc-200 bg-white text-zinc-600 hover:border-zinc-300 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300'}"
                      on:click={() => toggleCheckbox(f)}
                    >
                      <div class="flex h-5 w-5 shrink-0 items-center justify-center rounded border transition-colors {isCheckboxChecked(f) ? 'border-emerald-600 bg-emerald-600 text-white' : 'border-zinc-400 bg-white dark:border-zinc-600 dark:bg-zinc-700'}">
                        {#if isCheckboxChecked(f)}
                          <Check size={14} class="stroke-[3]" />
                        {/if}
                      </div>
                      <span class="text-xs font-medium">
                        {isCheckboxChecked(f) ? 'İşaretli (Seçili)' : 'İşaretsiz (Boş)'}
                      </span>
                    </button>
                  {:else if f.field_type === "radio"}
                    <!-- Doğru ve Şık Radyo Buton Seçenekleri -->
                    <div class="flex flex-wrap gap-1.5 pt-0.5">
                      {#each f.options as opt}
                        <button
                          type="button"
                          class="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-semibold transition-all cursor-pointer {fieldValues[f.name] === opt ? 'border-purple-600 bg-purple-600 text-white shadow-xs' : 'border-zinc-200 bg-white text-zinc-700 hover:border-zinc-300 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200'}"
                          on:click={() => selectRadioOption(f, opt)}
                        >
                          <div class="h-2 w-2 rounded-full {fieldValues[f.name] === opt ? 'bg-white' : 'border border-zinc-400 dark:border-zinc-500'}"></div>
                          <span>{opt}</span>
                        </button>
                      {/each}
                    </div>
                  {:else if f.field_type === "multiline"}
                    <textarea
                      rows="2"
                      class="{inputCls} resize-y"
                      placeholder={`${f.name} değerini girin`}
                      bind:value={fieldValues[f.name]}
                      disabled={f.read_only}
                    ></textarea>
                  {:else if f.field_type === "choice" && f.options && f.options.length > 0}
                    <select
                      class={inputCls}
                      bind:value={fieldValues[f.name]}
                      disabled={f.read_only}
                    >
                      <option value="">-- Seçiniz --</option>
                      {#each f.options as opt}
                        <option value={opt}>{opt}</option>
                      {/each}
                    </select>
                  {:else if f.field_type === "pushbutton"}
                    <div class="rounded-lg border border-dashed border-zinc-300 bg-zinc-100/60 px-3 py-2 text-[11px] text-zinc-500 dark:border-zinc-700 dark:bg-zinc-800/60 dark:text-zinc-400">
                      Bu alan bir PDF aksiyon butonudur (yazdırma / gönderme), metin doldurma gerektirmez.
                    </div>
                  {:else}
                    <input
                      type="text"
                      class={inputCls}
                      placeholder={`${f.name} girin`}
                      bind:value={fieldValues[f.name]}
                      disabled={f.read_only}
                    />
                  {/if}
                </div>
              {/each}
            {/if}
          </div>

          <!-- Bottom Action Bar -->
          <div class="mt-4 pt-3 border-t border-zinc-100 dark:border-zinc-800">
            <ActionButton
              label="Formu Doldur ve Kaydet"
              {busy}
              disabled={fields.length === 0}
              onClick={runFill}
            />
          </div>
        {:else}
          <!-- TAB 2: İMZA DAMGASI -->
          <div class="flex items-center justify-between border-b border-zinc-100 pb-3 dark:border-zinc-800">
            <div>
              <h3 class="font-bold text-sm text-zinc-900 dark:text-zinc-100">
                İmza Damgası Yerleşimi
              </h3>
              <p class="text-[11px] text-zinc-500 dark:text-zinc-400">
                İmzanızı çizin veya görsel yükleyin, sol taraftaki PDF önizlemesinden yerini belirleyin
              </p>
            </div>
          </div>

          <div class="mt-4 flex-1 overflow-y-auto pr-1 space-y-4">
            <!-- Signature Source Selection -->
            <div class="flex flex-col gap-2">
              <span class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">
                1. İmza Görseli Seçin
              </span>

              <div class="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  class="flex items-center justify-center gap-2 rounded-xl border-2 border-dashed border-indigo-300 bg-indigo-50/40 p-3 text-xs font-bold text-indigo-700 hover:bg-indigo-50 hover:border-indigo-500 dark:border-indigo-800 dark:bg-indigo-950/30 dark:text-indigo-300 transition-all cursor-pointer"
                  on:click={() => (showSignatureModal = true)}
                >
                  <PenTool size={16} />
                  <span>İmza Çiz veya Yaz</span>
                </button>

                <button
                  type="button"
                  class="flex items-center justify-center gap-2 rounded-xl border border-zinc-200 bg-white p-3 text-xs font-bold text-zinc-700 hover:bg-zinc-50 hover:border-zinc-300 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-200 transition-all cursor-pointer"
                  on:click={pickSignatureFile}
                >
                  <Upload size={16} />
                  <span>Görsel Yükle (PNG)</span>
                </button>
              </div>

              {#if sigImagePath}
                <div class="flex items-center justify-between rounded-xl border border-purple-200 bg-purple-50/60 p-3 dark:border-purple-800/60 dark:bg-purple-950/40">
                  <div class="flex items-center gap-3">
                    {#if sigPreviewUrl}
                      <img src={sigPreviewUrl} alt="İmza" class="h-10 max-w-[120px] object-contain rounded bg-white p-1 shadow-2xs" />
                    {:else}
                      <PenTool size={18} class="text-purple-600 dark:text-purple-400" />
                    {/if}
                    <div class="flex flex-col">
                      <span class="text-xs font-bold text-purple-900 dark:text-purple-200 truncate max-w-[180px]">
                        {baseName(sigImagePath)}
                      </span>
                      <span class="text-[10px] text-purple-600 dark:text-purple-400">
                        İmza hazırlandı
                      </span>
                    </div>
                  </div>

                  <button
                    type="button"
                    class="rounded-lg p-1 text-purple-500 hover:bg-purple-100 hover:text-purple-700 dark:hover:bg-purple-900/60 cursor-pointer"
                    on:click={() => { sigImagePath = null; sigPreviewUrl = null; }}
                    title="İmzayı Kaldır"
                  >
                    <X size={15} />
                  </button>
                </div>
              {/if}
            </div>

            <!-- Position & Size Controls -->
            <div class="flex flex-col gap-2.5 rounded-xl border border-zinc-200 bg-zinc-50/70 p-3.5 dark:border-zinc-800 dark:bg-zinc-800/40">
              <span class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">
                2. Konum ve Boyut Ayarları
              </span>
              <p class="text-[11px] text-zinc-500 dark:text-zinc-400">
                Sol taraftaki PDF önizlemesinde mor kutuyu doğrudan sürükleyebilir veya sayfada istediğiniz noktaya tıklayabilirsiniz.
              </p>

              <div class="grid grid-cols-2 gap-2.5">
                <label class="flex flex-col gap-1 text-[11px] font-medium text-zinc-600 dark:text-zinc-400">
                  Hedef Sayfa
                  <input
                    type="number"
                    min="1"
                    max={doc ? doc.pageCount : 1}
                    class={inputCls}
                    bind:value={sigPage}
                    on:change={() => (activePage = sigPage)}
                  />
                </label>

                <label class="flex flex-col gap-1 text-[11px] font-medium text-zinc-600 dark:text-zinc-400">
                  Genişlik (pt)
                  <input type="number" min="20" max="600" class={inputCls} bind:value={sigW} />
                </label>

                <label class="flex flex-col gap-1 text-[11px] font-medium text-zinc-600 dark:text-zinc-400">
                  X Koordinatı (pt)
                  <input type="number" class={inputCls} bind:value={sigX} />
                </label>

                <label class="flex flex-col gap-1 text-[11px] font-medium text-zinc-600 dark:text-zinc-400">
                  Y Koordinatı (pt)
                  <input type="number" class={inputCls} bind:value={sigY} />
                </label>

                <label class="flex flex-col gap-1 text-[11px] font-medium text-zinc-600 dark:text-zinc-400 col-span-2">
                  Yükseklik (pt)
                  <input type="number" min="10" max="400" class={inputCls} bind:value={sigH} />
                </label>
              </div>
            </div>
          </div>

          <!-- Bottom Action Bar -->
          <div class="mt-4 pt-3 border-t border-zinc-100 dark:border-zinc-800">
            <ActionButton
              label="İmzayı Damgala ve Kaydet"
              {busy}
              disabled={!sigImagePath}
              onClick={runStamp}
            />
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

{#if showSignatureModal}
  <SignaturePadModal
    onClose={() => (showSignatureModal = false)}
    onSaveSignature={handleSaveDrawnSignature}
  />
{/if}
