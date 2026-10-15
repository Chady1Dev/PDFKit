<script lang="ts">
  import { onMount, createEventDispatcher } from "svelte";
  import {
    ShieldCheck,
    Download,
    ExternalLink,
    RefreshCw,
    Check,
    Sparkles,
  } from "@lucide/svelte";
  import {
    checkPythonEnvironment,
    openExternalUrl,
    installPythonRuntime,
    installPythonPackages,
    type PythonEnvStatus,
  } from "../api";
  import { pushToast } from "../stores";

  export let requiredFormat: string = "docx";
  export let compact: boolean = false;
  $: void requiredFormat;

  const dispatch = createEventDispatcher<{
    ready: PythonEnvStatus;
    bypass: void;
  }>();

  let loading = true;
  let status: PythonEnvStatus | null = null;

  // Kurulum durumları (CREATE_NO_WINDOW arka plan işlemleri)
  let installingPython = false;
  let installingPackages = false;
  let packagesInstalledSuccess = false;

  export async function checkStatus(forceRefresh = false) {
    loading = true;
    try {
      status = await checkPythonEnvironment();

      if (status.installed && status.packages_ready) {
        dispatch("ready", status);
        if (forceRefresh) {
          pushToast("Python ve tüm dönüştürme paketleri hazır!", "success");
        }
      } else if (forceRefresh) {
        if (!status.installed) {
          pushToast(
            "Henüz Python algılanamadı. Otomatik kurulumu başlatabilir veya python.org'dan indirebilirsiniz.",
            "error"
          );
        } else if (!status.packages_ready) {
          pushToast(
            `Python (${status.version}) algılandı! Şimdi 'Python İçin Paketleri Otomatik Kur' butonuyla devam edebilirsiniz.`,
            "info"
          );
        }
      }
    } catch (err) {
      console.warn("Python check error:", err);
    } finally {
      loading = false;
    }
  }

  // 1. Kart Aksiyonu: Arka planda (CREATE_NO_WINDOW) Python indir ve kur -> Bittiğinde 2. Karta geç
  async function handleAutoInstallPython() {
    installingPython = true;
    try {
      await installPythonRuntime();
      pushToast("Python kurulumu tamamlandı! Şimdi dönüştürme paketlerini kurabilirsiniz.", "success");
      await checkStatus(false);
    } catch (err) {
      pushToast(`${err}`, "error");
    } finally {
      installingPython = false;
    }
  }

  // 2. Kart Aksiyonu: Arka planda (CREATE_NO_WINDOW) Pip paketlerini kur -> Loading -> Yeşil Tik -> Kilit Kalkar
  async function handleAutoInstallPackages() {
    installingPackages = true;
    packagesInstalledSuccess = false;
    try {
      await installPythonPackages();
      installingPackages = false;
      packagesInstalledSuccess = true;
      pushToast("Tüm dönüştürme paketleri başarıyla kuruldu!", "success");

      // Yeşil tik onayını kullanıcıya gösterdikten hemen sonra kilidi tamamen kaldır
      setTimeout(async () => {
        packagesInstalledSuccess = false;
        await checkStatus(false);
      }, 950);
    } catch (err) {
      installingPackages = false;
      pushToast(`Paket kurulumu tamamlanamadı: ${err}`, "error");
    }
  }

  onMount(() => {
    checkStatus();

    // Kullanıcı python.org'dan kurulumu tamamlayıp PDFKit penceresine döndüğü anda otomatik algılar
    const onFocus = () => {
      if (
        status &&
        (!status.installed || !status.packages_ready) &&
        !installingPython &&
        !installingPackages
      ) {
        checkStatus();
      }
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });

  function handleOpenDownload() {
    openExternalUrl("https://www.python.org/downloads/");
  }
</script>

{#if loading}
  <div class="flex flex-col items-center justify-center p-8 text-center text-zinc-500 dark:text-zinc-400">
    <RefreshCw size={24} class="animate-spin text-blue-600 mb-3" />
    <span class="text-xs font-semibold">Dönüştürme ortamı denetleniyor...</span>
  </div>
{:else if status && status.installed && status.packages_ready}
  <!-- Python ve tüm paketler yüklüyse bu uyarı ekranı ASLA gelmez, doğrudan dönüştürme arayüzü açılır -->
  <slot />
{:else}
  <!-- PYTHON VEYA PAKET EKSİK: Akıllı, İki Aşamalı, Penceresiz (CREATE_NO_WINDOW) Kurulum Kartı -->
  <div
    class="relative w-full rounded-2xl border border-zinc-200 bg-gradient-to-b from-white to-zinc-50/50 p-6 shadow-sm dark:border-zinc-800 dark:from-zinc-900 dark:to-zinc-900/50 text-zinc-900 dark:text-zinc-100 animate-in fade-in zoom-in-95 duration-200 {compact ? 'max-w-xl mx-auto' : 'max-w-2xl mx-auto my-3'}"
  >
    <!-- Başlık & İkon Rozeti -->
    <div class="flex flex-col items-center text-center">
      <div
        class="relative mb-3 flex h-14 w-14 items-center justify-center rounded-2xl bg-gradient-to-br from-blue-500/10 via-indigo-500/10 to-amber-500/10 p-3 ring-1 ring-blue-500/20 dark:from-blue-500/20 dark:to-amber-500/20"
      >
        <span class="text-2xl select-none">🐍</span>
        <span
          class="absolute -right-1 -bottom-1 flex h-6 w-6 items-center justify-center rounded-full {packagesInstalledSuccess ? 'bg-emerald-600' : 'bg-blue-600'} text-[11px] text-white shadow-xs transition-colors"
        >
          {packagesInstalledSuccess ? "✓" : "⇄"}
        </span>
      </div>

      <div
        class="inline-flex items-center gap-1.5 rounded-full bg-blue-50 px-3 py-1 text-[11px] font-bold text-blue-700 dark:bg-blue-950/60 dark:text-blue-300 mb-2"
      >
        <Sparkles size={13} />
        <span>Gelişmiş Ofis Dönüştürücü Motoru</span>
      </div>

      <h3 class="text-base sm:text-lg font-bold tracking-tight text-zinc-900 dark:text-zinc-100">
        {#if packagesInstalledSuccess}
          Kurulum Tamamlandı!
        {:else if status && status.installed && !status.packages_ready}
          Dönüştürme Kütüphaneleri Eksik
        {:else}
          Bu Özellik İçin Python Gereklidir
        {/if}
      </h3>

      <p class="mt-1.5 max-w-lg text-xs leading-relaxed text-zinc-600 dark:text-zinc-400">
        PDF belgelerini, Word (.docx) ve PowerPoint (.pptx) office formatlarına kayıpsız ve koruyarak dönüştürmek için Python altyapısı kullanılmaktadır.
      </p>
    </div>

    <!-- Güvenlik Bilgilendirmesi (Yeşil Bilgilendirme Alanı) -->
    <div
      class="mt-4 rounded-xl border border-emerald-200/80 bg-emerald-50/60 p-3.5 text-left dark:border-emerald-900/40 dark:bg-emerald-950/20"
    >
      <div class="flex items-start gap-2.5">
        <div
          class="mt-0.5 rounded-lg bg-emerald-100 p-1 text-emerald-700 dark:bg-emerald-900/60 dark:text-emerald-300"
        >
          <ShieldCheck size={16} />
        </div>
        <div class="text-xs">
          <h4 class="font-bold text-emerald-900 dark:text-emerald-200">
            %100 Güvenli, Yerel ve Bağımsız
          </h4>
          <p class="mt-0.5 text-emerald-800/90 dark:text-emerald-300/80 leading-normal text-[11px]">
            Python, dünya çapında kabul görmüş resmi ve güvenli bir programlama aracıdır. Dönüştürme esnasında <b>hiçbir dosyanız internete gönderilmez</b>; tüm işlem tamamen kendi bilgisayarınızın işlemcisinde gerçekleşir.
          </p>
        </div>
      </div>
    </div>

    <!-- AŞAMALI EYLEM KARTI -->
    <div class="mt-5">
      {#if !status || !status.installed}
        <!-- KART 1: PYTHON HİÇ YÜKLÜ DEĞİLSE -->
        <div
          class="rounded-xl border border-zinc-200 bg-white p-4 shadow-2xs dark:border-zinc-800 dark:bg-zinc-800/40"
        >
          <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div>
              <div class="flex items-center gap-2 text-xs font-bold text-zinc-800 dark:text-zinc-100">
                <span
                  class="flex h-5 w-5 items-center justify-center rounded-full bg-blue-600 text-[10px] font-bold text-white"
                  >1</span
                >
                <span>Adım 1: Python Altyapısını Kurun</span>
              </div>
              <p class="mt-1 text-[11px] text-zinc-500 dark:text-zinc-400">
                Uygulama içinden tek tıkla arka planda kurabilir veya resmi web sitesinden indirebilirsiniz.
              </p>
            </div>

            <div class="flex flex-wrap items-center gap-2 shrink-0">
              <!-- Tek Tıkla Sessiz Kurulum (CREATE_NO_WINDOW) -->
              <button
                type="button"
                class="inline-flex items-center gap-2 rounded-xl bg-blue-600 hover:bg-blue-700 active:scale-95 px-3.5 py-2 text-xs font-bold text-white shadow-xs transition-all disabled:opacity-60 cursor-pointer"
                disabled={installingPython}
                on:click={handleAutoInstallPython}
              >
                {#if installingPython}
                  <RefreshCw size={14} class="animate-spin" />
                  <span>Python Kuruluyor...</span>
                {:else}
                  <Download size={14} />
                  <span>Python'ı Otomatik Kur</span>
                {/if}
              </button>

              <!-- Resmi Siteden İndirme Tercihi -->
              <button
                type="button"
                class="inline-flex items-center gap-1.5 rounded-xl border border-zinc-300 dark:border-zinc-700 bg-zinc-50 hover:bg-zinc-100 dark:bg-zinc-800 dark:hover:bg-zinc-700 px-3 py-2 text-xs font-semibold text-zinc-700 dark:text-zinc-200 transition-colors cursor-pointer"
                disabled={installingPython}
                on:click={handleOpenDownload}
              >
                <ExternalLink size={13} />
                <span>python.org'dan İndir</span>
              </button>
            </div>
          </div>
        </div>
      {:else}
        <!-- KART 2: PYTHON YÜKLÜ (VEYA YENİ KURULDU), SIRA PAKETLERDE -->
        <div
          class="rounded-xl border {packagesInstalledSuccess ? 'border-emerald-500 bg-emerald-50/40 dark:bg-emerald-950/30' : 'border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-800/40'} p-4 shadow-2xs transition-all"
        >
          <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div>
              <div class="flex items-center gap-2">
                <span
                  class="inline-flex items-center gap-1 rounded-md bg-emerald-100 px-2 py-0.5 text-[11px] font-bold text-emerald-800 dark:bg-emerald-950 dark:text-emerald-300"
                >
                  <Check size={12} />
                  Python {status.version ?? ""} Algılandı
                </span>
              </div>
              <p class="mt-1.5 text-xs font-semibold text-zinc-800 dark:text-zinc-200">
                {#if packagesInstalledSuccess}
                  Tüm dönüştürme kütüphaneleri hazır! Dönüştürme ekranı açılıyor...
                {:else if installingPackages}
                  Gerekli dönüştürme paketleri arka planda kuruluyor, lütfen bekleyin...
                {:else}
                  Ofis dönüşümü için gerekli kütüphaneleri tek tıkla tamamlayın.
                {/if}
              </p>
            </div>

            <div class="shrink-0">
              {#if packagesInstalledSuccess}
                <div
                  class="inline-flex items-center gap-2 rounded-xl bg-emerald-600 px-4 py-2.5 text-xs font-bold text-white shadow-xs"
                >
                  <Check size={16} />
                  <span>Tamamlandı!</span>
                </div>
              {:else}
                <button
                  type="button"
                  class="inline-flex items-center gap-2 rounded-xl bg-emerald-600 hover:bg-emerald-700 active:scale-95 px-4 py-2.5 text-xs font-bold text-white shadow-xs transition-all disabled:opacity-60 cursor-pointer"
                  disabled={installingPackages}
                  on:click={handleAutoInstallPackages}
                >
                  {#if installingPackages}
                    <RefreshCw size={14} class="animate-spin" />
                    <span>Paketler Kuruluyor...</span>
                  {:else}
                    <Download size={14} />
                    <span>Python İçin Paketleri Otomatik Kur</span>
                  {/if}
                </button>
              {/if}
            </div>
          </div>
        </div>
      {/if}
    </div>

    <!-- Alt Kontrol Barı -->
    <div
      class="mt-5 flex flex-wrap items-center justify-between gap-3 border-t border-zinc-100 pt-4 dark:border-zinc-800"
    >
      <!-- Sol Ana Buton -->
      <button
        type="button"
        class="inline-flex items-center gap-2 rounded-xl bg-blue-600 hover:bg-blue-700 active:scale-95 px-4 py-2 text-xs font-bold text-white shadow-xs transition-all disabled:opacity-50 cursor-pointer"
        disabled={installingPython || installingPackages || packagesInstalledSuccess}
        on:click={() => checkStatus(true)}
      >
        <RefreshCw size={14} class={loading ? "animate-spin" : ""} />
        <span>Kurulumu Tamamladım, Tekrar Denetle</span>
      </button>

      <!-- Temel Moda Geç Butonu (İçi şeffaf, sol ana butonla aynı mavi çerçeveli) -->
      {#if $$slots.default}
        <button
          type="button"
          class="inline-flex items-center gap-1.5 rounded-xl border-2 border-blue-600 bg-transparent px-4 py-2 text-xs font-bold text-blue-600 hover:bg-blue-50 dark:hover:bg-blue-950/40 active:scale-95 transition-all cursor-pointer"
          on:click={() => dispatch("bypass")}
        >
          Temel Moda Geç
        </button>
      {/if}
    </div>
  </div>
{/if}
