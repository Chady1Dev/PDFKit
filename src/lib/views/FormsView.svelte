<script lang="ts">
  import { save, open } from "@tauri-apps/plugin-dialog";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import { listFormFields, fillForm, stampImage, baseName, type FormField } from "../api";
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
  let fields: FormField[] = [];
  let busy = false;

  // Signature stamp
  let sigImage: string | null = null;
  let sigPage = 1;
  let sigX = 400;
  let sigY = 50;
  let sigW = 150;
  let sigH = 60;

  const inputCls =
    "rounded-lg border border-zinc-300 bg-white px-3 py-2 text-sm text-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100";

  async function load(paths: string[]) {
    input = paths[0];
    fields = [];
    try {
      fields = await listFormFields(input);
      if (fields.length === 0) pushToast("Bu PDF'de AcroForm alanı bulunamadı", "info");
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    }
  }

  async function runFill() {
    if (!input) return;
    const output = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }], defaultPath: "doldurulmus.pdf" });
    if (!output) return;
    busy = true;
    try {
      const values = Object.fromEntries(fields.map((f) => [f.name, f.value]));
      const result = await fillForm(input, output, values);
      logActivity([baseName(input)], "Form Dolduruldu", output);
      pushToast(`Kaydedildi: ${baseName(result.outputs[0])}`, "success", {
        label: "Klasöre Git",
        path: output,
      });
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }

  async function pickSignature() {
    const selected = await open({ filters: [{ name: "Görsel", extensions: ["png", "jpg", "jpeg"] }] });
    if (typeof selected === "string") sigImage = selected;
  }

  async function runStamp() {
    if (!input || !sigImage) return;
    const output = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }], defaultPath: "imzali.pdf" });
    if (!output) return;
    busy = true;
    try {
      const result = await stampImage(input, output, sigPage, sigImage, sigX, sigY, sigW, sigH);
      logActivity([baseName(input)], "İmza Damgalandı", output);
      pushToast(`Kaydedildi: ${baseName(result.outputs[0])}`, "success", {
        label: "Klasöre Git",
        path: output,
      });
    } catch (e) {
      pushToast(`Hata: ${e}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<ToolHeader title="Form Doldur & İmza" description="AcroForm alanlarını doldur, imza görseli damgala" />

<FileDropzone extensions={["pdf"]} label="Formlu PDF'i sürükleyin veya seçin" onFiles={load} />

{#if input}
  <p class="mt-3 text-sm text-zinc-500 dark:text-zinc-400">Seçili: {baseName(input)}</p>
  <div class="mt-3 grid max-w-4xl grid-cols-1 gap-4 md:grid-cols-2">
    <section class="flex flex-col gap-3 rounded-xl border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900">
      <h3 class="font-medium text-zinc-900 dark:text-zinc-100">Form Alanları ({fields.length})</h3>
      {#if fields.length === 0}
        <p class="text-sm text-zinc-400">Alan yok.</p>
      {:else}
        <div class="flex max-h-72 flex-col gap-2 overflow-y-auto pr-1">
          {#each fields as f (f.name)}
            <label class="flex flex-col gap-1 text-xs text-zinc-500 dark:text-zinc-400">
              {f.name} <span class="text-zinc-400">({f.field_type})</span>
              <input class={inputCls} bind:value={f.value} />
            </label>
          {/each}
        </div>
        <ActionButton label="Doldur ve Kaydet" {busy} onClick={runFill} />
      {/if}
    </section>

    <section class="flex flex-col gap-3 rounded-xl border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900">
      <h3 class="font-medium text-zinc-900 dark:text-zinc-100">İmza Damgası</h3>
      <button class="rounded-lg border border-dashed border-zinc-300 px-3 py-2 text-sm text-zinc-500 hover:border-indigo-400 dark:border-zinc-700" on:click={pickSignature}>
        {sigImage ? baseName(sigImage) : "İmza görseli seç (PNG/JPG)"}
      </button>
      <div class="grid grid-cols-2 gap-2">
        <label class="flex flex-col gap-1 text-xs text-zinc-500">Sayfa
          <input type="number" min="1" class={inputCls} bind:value={sigPage} /></label>
        <label class="flex flex-col gap-1 text-xs text-zinc-500">X (pt)
          <input type="number" class={inputCls} bind:value={sigX} /></label>
        <label class="flex flex-col gap-1 text-xs text-zinc-500">Y (pt)
          <input type="number" class={inputCls} bind:value={sigY} /></label>
        <label class="flex flex-col gap-1 text-xs text-zinc-500">Genişlik
          <input type="number" class={inputCls} bind:value={sigW} /></label>
        <label class="flex flex-col gap-1 text-xs text-zinc-500">Yükseklik
          <input type="number" class={inputCls} bind:value={sigH} /></label>
      </div>
      <p class="text-xs text-zinc-400">Koordinatlar PDF punto biriminde, sol-alt köşe orijindir.</p>
      <ActionButton label="Damgala ve Kaydet" {busy} disabled={!sigImage} onClick={runStamp} />
    </section>
  </div>
{/if}
