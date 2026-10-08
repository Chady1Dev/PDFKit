<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import FileDropzone from "../components/FileDropzone.svelte";
  import ToolHeader from "../components/ToolHeader.svelte";
  import ActionButton from "../components/ActionButton.svelte";
  import { encryptPdf, decryptPdf, baseName } from "../api";
  import { pushToast, pendingFiles } from "../stores";
  import { logActivity } from "../activityStore";

  $: if ($pendingFiles && $pendingFiles.targetView === "security") {
    const pdfs = $pendingFiles.paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (pdfs.length > 0) {
      input = pdfs[0];
    }
    pendingFiles.set(null);
  }

  let mode: "encrypt" | "decrypt" = "encrypt";
  let input: string | null = null;
  let userPassword = "";
  let ownerPassword = "";
  let algorithm = "aes256";
  let allowPrint = true;
  let allowCopy = true;
  let allowModify = false;
  let allowAnnotate = true;
  let busy = false;

  const inputCls =
    "rounded-lg border border-zinc-300 bg-white px-3 py-2 text-sm text-zinc-900 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100";

  async function run() {
    if (!input) return;
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: mode === "encrypt" ? "sifreli.pdf" : "sifresiz.pdf",
    });
    if (!output) return;
    busy = true;
    try {
      const result =
        mode === "encrypt"
          ? await encryptPdf(input, output, userPassword, ownerPassword, {
              allow_print: allowPrint,
              allow_copy: allowCopy,
              allow_modify: allowModify,
              allow_annotate: allowAnnotate,
            }, algorithm)
          : await decryptPdf(input, output, userPassword);

      logActivity([baseName(input)], mode === "encrypt" ? "Şifrelendi" : "Şifre Kaldırıldı", output);
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

<ToolHeader title="Şifrele / Şifre Kaldır" description="AES-256 şifreleme ve gelişmiş izin yönetimi — dosyalar cihazdan asla çıkmaz">
  <div class="flex rounded-lg bg-zinc-200 p-0.5 dark:bg-zinc-800">
    {#each [["encrypt", "Şifrele"], ["decrypt", "Şifre Kaldır"]] as [m, label]}
      <button
        class="rounded-md px-3 py-1 text-xs font-medium {mode === m ? 'bg-white text-zinc-900 shadow dark:bg-zinc-700 dark:text-white' : 'text-zinc-500 dark:text-zinc-400'}"
        on:click={() => (mode = m as typeof mode)}
      >
        {label}
      </button>
    {/each}
  </div>
</ToolHeader>

<FileDropzone extensions={["pdf"]} label="PDF dosyasını sürükleyin veya seçin" onFiles={(p) => (input = p[0])} />

{#if input}
  <div class="mt-4 flex max-w-lg flex-col gap-4">
    <p class="text-sm font-medium text-zinc-700 dark:text-zinc-300">Seçili: <span class="font-mono text-zinc-500 dark:text-zinc-400">{baseName(input)}</span></p>

    <label class="flex flex-col gap-1 text-sm text-zinc-600 dark:text-zinc-400">
      {mode === "encrypt" ? "Kullanıcı şifresi (belgeyi açmak için)" : "Mevcut şifre"}
      <input type="password" class={inputCls} placeholder="Açılış şifresi girin..." bind:value={userPassword} />
    </label>

    {#if mode === "encrypt"}
      <label class="flex flex-col gap-1 text-sm text-zinc-600 dark:text-zinc-400">
        Yönetici / Sahip şifresi (izinleri değiştirmek için — boş bırakılırsa açılış şifresi kullanılır)
        <input type="password" class={inputCls} placeholder="Sahip şifresi (isteğe bağlı)..." bind:value={ownerPassword} />
      </label>

      <!-- Şifreleme Standardı -->
      <div class="rounded-xl border border-zinc-200 bg-white p-3.5 dark:border-zinc-800 dark:bg-zinc-900/60">
        <span class="text-xs font-bold text-zinc-800 dark:text-zinc-200 mb-2 block">Şifreleme Standardı</span>
        <div class="flex gap-4">
          <label class="flex items-center gap-2 text-xs font-medium text-zinc-700 dark:text-zinc-300 cursor-pointer">
            <input type="radio" bind:group={algorithm} value="aes256" class="text-indigo-600" />
            AES-256 (Önerilen, Askeri Düzey Güvenlik)
          </label>
          <label class="flex items-center gap-2 text-xs font-medium text-zinc-700 dark:text-zinc-300 cursor-pointer">
            <input type="radio" bind:group={algorithm} value="aes128" class="text-indigo-600" />
            AES-128 (Geniş Uyumluluk)
          </label>
        </div>
      </div>

      <!-- Belge İzinleri -->
      <div class="rounded-xl border border-zinc-200 bg-white p-3.5 dark:border-zinc-800 dark:bg-zinc-900/60">
        <span class="text-xs font-bold text-zinc-800 dark:text-zinc-200 mb-2.5 block">Belge İzinleri</span>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
          <label class="flex items-center gap-2 text-xs text-zinc-700 dark:text-zinc-300 cursor-pointer">
            <input type="checkbox" bind:checked={allowPrint} class="rounded text-indigo-600" />
            <span>Yazdırmaya İzin Ver</span>
          </label>
          <label class="flex items-center gap-2 text-xs text-zinc-700 dark:text-zinc-300 cursor-pointer">
            <input type="checkbox" bind:checked={allowCopy} class="rounded text-indigo-600" />
            <span>Metin / Görsel Kopyalamaya İzin Ver</span>
          </label>
          <label class="flex items-center gap-2 text-xs text-zinc-700 dark:text-zinc-300 cursor-pointer">
            <input type="checkbox" bind:checked={allowModify} class="rounded text-indigo-600" />
            <span>Sayfa / İçerik Değiştirmeye İzin Ver</span>
          </label>
          <label class="flex items-center gap-2 text-xs text-zinc-700 dark:text-zinc-300 cursor-pointer">
            <input type="checkbox" bind:checked={allowAnnotate} class="rounded text-indigo-600" />
            <span>Yorum & Form Doldurmaya İzin Ver</span>
          </label>
        </div>
      </div>
    {/if}

    <ActionButton
      label={mode === "encrypt" ? "Şifrele ve Kaydet" : "Şifreyi Kaldır"}
      {busy}
      disabled={!userPassword}
      onClick={run}
    />
  </div>
{/if}

