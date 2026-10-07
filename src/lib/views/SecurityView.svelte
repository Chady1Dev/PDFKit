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
          ? await encryptPdf(input, output, userPassword, ownerPassword)
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

<ToolHeader title="Şifrele / Şifre Kaldır" description="AES-128 şifreleme — dosyalar cihazdan asla çıkmaz">
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
  <div class="mt-4 flex max-w-md flex-col gap-3">
    <p class="text-sm text-zinc-500 dark:text-zinc-400">Seçili: {baseName(input)}</p>
    <label class="flex flex-col gap-1 text-sm text-zinc-500 dark:text-zinc-400">
      {mode === "encrypt" ? "Kullanıcı şifresi (açmak için)" : "Mevcut şifre"}
      <input type="password" class={inputCls} bind:value={userPassword} />
    </label>
    {#if mode === "encrypt"}
      <label class="flex flex-col gap-1 text-sm text-zinc-500 dark:text-zinc-400">
        Sahip şifresi (izinler için — boş = kullanıcı şifresi)
        <input type="password" class={inputCls} bind:value={ownerPassword} />
      </label>
    {/if}
    <ActionButton
      label={mode === "encrypt" ? "Şifrele ve Kaydet" : "Şifreyi Kaldır"}
      {busy}
      disabled={!userPassword}
      onClick={run}
    />
  </div>
{/if}
