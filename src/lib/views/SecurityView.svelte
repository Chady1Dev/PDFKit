<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { Lock, LockOpen, ChevronDown, FileText, ShieldCheck } from "@lucide/svelte";
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

  async function run(targetMode: "encrypt" | "decrypt" = mode) {
    if (!input) {
      pushToast("Lütfen önce bir PDF dosyası seçin.", "warning");
      return;
    }
    const defaultName =
      targetMode === "encrypt"
        ? baseName(input).replace(/\.pdf$/i, "") + "_sifreli.pdf"
        : baseName(input).replace(/\.pdf$/i, "") + "_sifresiz.pdf";
    const output = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: defaultName,
    });
    if (!output) return;
    busy = true;
    try {
      const result =
        targetMode === "encrypt"
          ? await encryptPdf(
              input,
              output,
              userPassword,
              ownerPassword,
              {
                allow_print: allowPrint,
                allow_copy: allowCopy,
                allow_modify: allowModify,
                allow_annotate: allowAnnotate,
              },
              algorithm,
            )
          : await decryptPdf(input, output, userPassword);

      logActivity([baseName(input)], targetMode === "encrypt" ? "Şifrelendi" : "Şifre Kaldırıldı", output);
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

<ToolHeader
  title="Şifrele & Şifre Kaldır"
  description="AES-256 şifreleme ve gelişmiş izin yönetimi — dosyalar cihazdan asla çıkmaz"
/>

<FileDropzone extensions={["pdf"]} label="PDF dosyasını sürükleyin veya seçin" onFiles={(p) => (input = p[0])} />

{#if input}
  <div class="mt-3 mb-1 flex items-center justify-between rounded-xl border border-zinc-200 bg-white px-3.5 py-2 text-xs dark:border-zinc-800 dark:bg-zinc-900 max-w-6xl">
    <div class="flex items-center gap-2 font-medium text-zinc-700 dark:text-zinc-300">
      <FileText size={15} class="text-indigo-600 dark:text-indigo-400" />
      <span>Seçili Belge: <span class="font-mono text-zinc-500 dark:text-zinc-400">{baseName(input)}</span></span>
    </div>
    <div class="flex items-center gap-1.5">
      <span class="text-zinc-400 dark:text-zinc-500">Aktif İşlem:</span>
      <div class="flex rounded-md bg-zinc-200 p-0.5 dark:bg-zinc-800">
        <button
          type="button"
          class="rounded px-2.5 py-0.5 text-[11px] font-semibold transition-all {mode === 'encrypt' ? 'bg-white text-zinc-900 shadow-xs dark:bg-zinc-700 dark:text-white' : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-800'} cursor-pointer"
          on:click={() => (mode = "encrypt")}
        >
          Şifrele
        </button>
        <button
          type="button"
          class="rounded px-2.5 py-0.5 text-[11px] font-semibold transition-all {mode === 'decrypt' ? 'bg-white text-zinc-900 shadow-xs dark:bg-zinc-700 dark:text-white' : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-800'} cursor-pointer"
          on:click={() => (mode = "decrypt")}
        >
          Şifre Kaldır
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Akordiyon Sistemi: Sadece Biri Açık, Diğeri Kompakt Kapalı (Varsayılan: Şifrele Açık) -->
<div class="mt-4 grid grid-cols-1 lg:grid-cols-2 gap-4 items-start max-w-6xl">
  <!-- Sol Kart: Şifrele -->
  {#if mode === "encrypt"}
    <div
      class="flex flex-col justify-between rounded-2xl border-2 p-5 bg-white shadow-md ring-1 ring-indigo-500/30 border-indigo-500/80 dark:bg-zinc-900 transition-all"
    >
      <div class="flex flex-col gap-4">
        <!-- Kart Başlığı -->
        <div class="flex items-center justify-between border-b border-zinc-100 pb-3 dark:border-zinc-800">
          <div class="flex items-center gap-3">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-indigo-600 text-white shadow-xs">
              <Lock size={18} />
            </div>
            <div class="flex flex-col">
              <h3 class="font-bold text-sm text-zinc-900 dark:text-zinc-100">
                Şifrele
              </h3>
              <span class="text-xs text-zinc-500 dark:text-zinc-400">
                Belgeyi parola ile koruyun ve kullanım izinlerini yönetin
              </span>
            </div>
          </div>

          <span class="flex items-center gap-1.5 rounded-full bg-indigo-50 px-2.5 py-1 text-[11px] font-bold text-indigo-700 dark:bg-indigo-950/60 dark:text-indigo-300">
            <span class="h-1.5 w-1.5 rounded-full bg-indigo-600 animate-pulse"></span>
            Aktif
          </span>
        </div>

        <label class="flex flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
          Kullanıcı şifresi (belgeyi açmak için)
          <input type="password" class={inputCls} placeholder="Açılış şifresi girin..." bind:value={userPassword} />
        </label>

        <label class="flex flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
          Yönetici / Sahip şifresi (izinleri değiştirmek için — boş bırakılırsa açılış şifresi kullanılır)
          <input type="password" class={inputCls} placeholder="Sahip şifresi (isteğe bağlı)..." bind:value={ownerPassword} />
        </label>

        <!-- Şifreleme Standardı -->
        <div class="rounded-xl border border-zinc-200 bg-zinc-50/50 p-3.5 dark:border-zinc-800 dark:bg-zinc-800/40">
          <span class="text-xs font-bold text-zinc-800 dark:text-zinc-200 mb-2 block">Şifreleme Standardı</span>
          <div class="flex flex-col sm:flex-row gap-3">
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
        <div class="rounded-xl border border-zinc-200 bg-zinc-50/50 p-3.5 dark:border-zinc-800 dark:bg-zinc-800/40">
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
      </div>

      <!-- Kartın Altındaki Aksiyon Butonu -->
      <div class="pt-4 mt-3 border-t border-zinc-100 dark:border-zinc-800">
        <ActionButton
          label="Şifrele ve Kaydet"
          {busy}
          disabled={!input || !userPassword}
          onClick={() => run("encrypt")}
        />
      </div>
    </div>
  {:else}
    <!-- Şifrele Kapalı / Kompakt Akordiyon Kartı -->
    <div
      role="button"
      tabindex="0"
      class="group flex flex-col justify-between rounded-2xl border-2 p-5 bg-zinc-50/70 hover:bg-white hover:border-indigo-400/70 dark:bg-zinc-900/50 dark:hover:bg-zinc-900 border-zinc-200 dark:border-zinc-800 transition-all cursor-pointer shadow-xs hover:shadow-md"
      on:click={() => (mode = "encrypt")}
      on:keydown={(e) => e.key === "Enter" && (mode = "encrypt")}
    >
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-zinc-200/80 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400 group-hover:bg-indigo-600 group-hover:text-white transition-colors">
            <Lock size={18} />
          </div>
          <div class="flex flex-col">
            <h3 class="font-bold text-sm text-zinc-700 dark:text-zinc-300 group-hover:text-zinc-900 dark:group-hover:text-white transition-colors">
              Şifrele
            </h3>
            <span class="text-xs text-zinc-500 dark:text-zinc-400">
              Belgeyi parola ile koruyun ve kullanım izinlerini yönetin
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

  <!-- Sağ Kart: Şifre Kaldır -->
  {#if mode === "decrypt"}
    <div
      class="flex flex-col justify-between rounded-2xl border-2 p-5 bg-white shadow-md ring-1 ring-indigo-500/30 border-indigo-500/80 dark:bg-zinc-900 transition-all"
    >
      <div class="flex flex-col gap-4">
        <!-- Kart Başlığı -->
        <div class="flex items-center justify-between border-b border-zinc-100 pb-3 dark:border-zinc-800">
          <div class="flex items-center gap-3">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-indigo-600 text-white shadow-xs">
              <LockOpen size={18} />
            </div>
            <div class="flex flex-col">
              <h3 class="font-bold text-sm text-zinc-900 dark:text-zinc-100">
                Şifre Kaldır
              </h3>
              <span class="text-xs text-zinc-500 dark:text-zinc-400">
                Parola korumalı PDF belgesinin şifresini kaldırın
              </span>
            </div>
          </div>

          <span class="flex items-center gap-1.5 rounded-full bg-indigo-50 px-2.5 py-1 text-[11px] font-bold text-indigo-700 dark:bg-indigo-950/60 dark:text-indigo-300">
            <span class="h-1.5 w-1.5 rounded-full bg-indigo-600 animate-pulse"></span>
            Aktif
          </span>
        </div>

        <label class="flex flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
          Mevcut şifre
          <input type="password" class={inputCls} placeholder="Açılış şifresi girin..." bind:value={userPassword} />
        </label>

        <div class="flex items-center gap-2 rounded-xl border border-zinc-200 bg-zinc-50/50 p-3 text-[11px] text-zinc-500 dark:border-zinc-800 dark:bg-zinc-800/40 dark:text-zinc-400">
          <ShieldCheck size={16} class="shrink-0 text-emerald-600 dark:text-emerald-400" />
          <span>Belgenin mevcut açılış şifresini girerek parola korumasını ve kısıtlamaları tamamen kaldırabilirsiniz.</span>
        </div>
      </div>

      <!-- Kartın Altındaki Aksiyon Butonu -->
      <div class="pt-4 mt-3 border-t border-zinc-100 dark:border-zinc-800">
        <ActionButton
          label="Şifreyi Kaldır"
          {busy}
          disabled={!input || !userPassword}
          onClick={() => run("decrypt")}
        />
      </div>
    </div>
  {:else}
    <!-- Şifre Kaldır Kapalı / Kompakt Akordiyon Kartı -->
    <div
      role="button"
      tabindex="0"
      class="group flex flex-col justify-between rounded-2xl border-2 p-5 bg-zinc-50/70 hover:bg-white hover:border-indigo-400/70 dark:bg-zinc-900/50 dark:hover:bg-zinc-900 border-zinc-200 dark:border-zinc-800 transition-all cursor-pointer shadow-xs hover:shadow-md"
      on:click={() => (mode = "decrypt")}
      on:keydown={(e) => e.key === "Enter" && (mode = "decrypt")}
    >
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-zinc-200/80 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400 group-hover:bg-indigo-600 group-hover:text-white transition-colors">
            <LockOpen size={18} />
          </div>
          <div class="flex flex-col">
            <h3 class="font-bold text-sm text-zinc-700 dark:text-zinc-300 group-hover:text-zinc-900 dark:group-hover:text-white transition-colors">
              Şifre Kaldır
            </h3>
            <span class="text-xs text-zinc-500 dark:text-zinc-400">
              Parola korumalı PDF belgesinin şifresini kaldırın
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
</div>

