<script lang="ts">
  import { onMount } from "svelte";
  import { X, Check, Eraser, Upload, PenLine, Type, RefreshCw } from "@lucide/svelte";

  export let onClose: () => void;
  export let onSaveSignature: (dataUrl: string) => void;

  let activeTab: "draw" | "upload" | "type" = "draw";

  // Drawing canvas
  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D | null = null;
  let isDrawing = false;
  let hasDrawn = false;
  let penColor = "#0f172a";
  let strokeWidth = 3;

  // Type signature
  let typedName = "";
  let typedFont = "cursive";

  // Upload signature
  let uploadedImage: string | null = null;

  onMount(() => {
    initCanvas();
  });

  function initCanvas() {
    if (!canvas) return;
    ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.strokeStyle = penColor;
    ctx.lineWidth = strokeWidth;
  }

  function startDrawing(e: MouseEvent | TouchEvent) {
    if (!canvas || !ctx) return;
    isDrawing = true;
    hasDrawn = true;
    const rect = canvas.getBoundingClientRect();
    const x = "touches" in e ? e.touches[0].clientX - rect.left : e.clientX - rect.left;
    const y = "touches" in e ? e.touches[0].clientY - rect.top : e.clientY - rect.top;
    ctx.beginPath();
    ctx.moveTo(x, y);
  }

  function draw(e: MouseEvent | TouchEvent) {
    if (!isDrawing || !ctx || !canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = "touches" in e ? e.touches[0].clientX - rect.left : e.clientX - rect.left;
    const y = "touches" in e ? e.touches[0].clientY - rect.top : e.clientY - rect.top;
    ctx.strokeStyle = penColor;
    ctx.lineWidth = strokeWidth;
    ctx.lineTo(x, y);
    ctx.stroke();
  }

  function stopDrawing() {
    isDrawing = false;
  }

  function clearCanvas() {
    if (!ctx || !canvas) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    hasDrawn = false;
  }

  function handleFileUpload(e: Event) {
    const input = e.target as HTMLInputElement;
    if (!input.files || input.files.length === 0) return;
    const file = input.files[0];
    const reader = new FileReader();
    reader.onload = (ev) => {
      uploadedImage = ev.target?.result as string;
    };
    reader.readAsDataURL(file);
  }

  function createTypedSignature(): string {
    const tempCanvas = document.createElement("canvas");
    tempCanvas.width = 450;
    tempCanvas.height = 160;
    const tCtx = tempCanvas.getContext("2d")!;
    tCtx.fillStyle = penColor;
    tCtx.font = `italic 42px ${typedFont}, 'Segoe Script', 'Brush Script MT', cursive`;
    tCtx.textBaseline = "middle";
    tCtx.textAlign = "center";
    tCtx.fillText(typedName || "İmza", tempCanvas.width / 2, tempCanvas.height / 2);
    return tempCanvas.toDataURL("image/png");
  }

  function submit() {
    if (activeTab === "draw") {
      if (!hasDrawn || !canvas) return;
      onSaveSignature(canvas.toDataURL("image/png"));
    } else if (activeTab === "upload") {
      if (!uploadedImage) return;
      onSaveSignature(uploadedImage);
    } else if (activeTab === "type") {
      if (!typedName.trim()) return;
      onSaveSignature(createTypedSignature());
    }
    onClose();
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4" on:click|self={onClose}>
  <div class="w-full max-w-lg rounded-2xl border border-zinc-200 bg-white p-6 shadow-2xl dark:border-zinc-800 dark:bg-zinc-900">
    <!-- Header -->
    <div class="flex items-center justify-between border-b border-zinc-100 pb-3 dark:border-zinc-800">
      <h3 class="text-lg font-bold text-zinc-900 dark:text-zinc-100">İmza Ekle</h3>
      <button class="rounded-lg p-1 text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 hover:text-zinc-700 dark:hover:text-zinc-200" on:click={onClose}>
        <X size={18} />
      </button>
    </div>

    <!-- Tab Selector -->
    <div class="my-4 flex rounded-xl bg-zinc-100 p-1 dark:bg-zinc-800">
      <button
        class="flex flex-1 items-center justify-center gap-1.5 rounded-lg py-2 text-xs font-semibold transition-all {activeTab === 'draw' ? 'bg-white text-indigo-600 shadow-sm dark:bg-zinc-700 dark:text-indigo-400' : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400'}"
        on:click={() => { activeTab = "draw"; setTimeout(initCanvas, 50); }}
      >
        <PenLine size={14} /> Çiz
      </button>
      <button
        class="flex flex-1 items-center justify-center gap-1.5 rounded-lg py-2 text-xs font-semibold transition-all {activeTab === 'type' ? 'bg-white text-indigo-600 shadow-sm dark:bg-zinc-700 dark:text-indigo-400' : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400'}"
        on:click={() => (activeTab = "type")}
      >
        <Type size={14} /> Yaz
      </button>
      <button
        class="flex flex-1 items-center justify-center gap-1.5 rounded-lg py-2 text-xs font-semibold transition-all {activeTab === 'upload' ? 'bg-white text-indigo-600 shadow-sm dark:bg-zinc-700 dark:text-indigo-400' : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400'}"
        on:click={() => (activeTab = "upload")}
      >
        <Upload size={14} /> Görsel Yükle
      </button>
    </div>

    <!-- Content -->
    {#if activeTab === "draw"}
      <div class="flex flex-col gap-3">
        <div class="relative overflow-hidden rounded-xl border-2 border-dashed border-zinc-300 bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-950">
          <canvas
            bind:this={canvas}
            width={480}
            height={200}
            class="touch-none cursor-crosshair w-full"
            on:mousedown={startDrawing}
            on:mousemove={draw}
            on:mouseup={stopDrawing}
            on:mouseleave={stopDrawing}
            on:touchstart|passive={startDrawing}
            on:touchmove|passive={draw}
            on:touchend={stopDrawing}
          ></canvas>
          {#if !hasDrawn}
            <div class="pointer-events-none absolute inset-0 flex items-center justify-center text-xs text-zinc-400">
              Buraya farenizle veya dokunmatik ekranınızla imzanızı çizin
            </div>
          {/if}
        </div>

        <div class="flex items-center justify-between text-xs text-zinc-500">
          <div class="flex items-center gap-2">
            <span>Renk:</span>
            {#each ["#0f172a", "#1d4ed8", "#b91c1c"] as c}
              <button
                title="Renk: {c}"
                aria-label="Renk: {c}"
                class="h-5 w-5 rounded-full border-2 {penColor === c ? 'border-indigo-500 ring-2 ring-indigo-200' : 'border-transparent'}"
                style="background-color: {c}"
                on:click={() => (penColor = c)}
              ></button>
            {/each}
          </div>
          <button
            class="flex items-center gap-1 text-zinc-500 hover:text-red-500"
            on:click={clearCanvas}
          >
            <Eraser size={13} /> Temizle
          </button>
        </div>
      </div>
    {:else if activeTab === "type"}
      <div class="flex flex-col gap-3 py-2">
        <label class="flex flex-col gap-1 text-xs text-zinc-500 dark:text-zinc-400">
          İsim / Soyisim
          <input
            type="text"
            bind:value={typedName}
            placeholder="Ad Soyad"
            class="rounded-xl border border-zinc-300 bg-white px-3 py-2.5 text-sm dark:border-zinc-700 dark:bg-zinc-800 dark:text-white"
          />
        </label>
        <div class="flex h-28 items-center justify-center rounded-xl border border-zinc-200 bg-zinc-50 p-4 dark:border-zinc-800 dark:bg-zinc-950">
          <span style="font-family: {typedFont}, 'Segoe Script', cursive; font-size: 32px; color: {penColor}; font-style: italic;">
            {typedName || "İmza Önizleme"}
          </span>
        </div>
      </div>
    {:else if activeTab === "upload"}
      <div class="flex flex-col gap-3 py-2">
        {#if !uploadedImage}
          <label class="flex h-36 cursor-pointer flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-zinc-300 bg-zinc-50 p-4 text-center hover:border-indigo-400 dark:border-zinc-700 dark:bg-zinc-950">
            <Upload size={24} class="text-zinc-400" />
            <span class="text-xs text-zinc-500 dark:text-zinc-400">İmza görseli (PNG veya JPG) seçin</span>
            <input type="file" accept="image/png,image/jpeg" class="hidden" on:change={handleFileUpload} />
          </label>
        {:else}
          <div class="relative flex h-36 items-center justify-center rounded-xl border border-zinc-200 bg-zinc-50 p-4 dark:border-zinc-800 dark:bg-zinc-950">
            <img src={uploadedImage} alt="İmza" class="max-h-full max-w-full object-contain" />
            <button
              class="absolute right-2 top-2 rounded-lg bg-zinc-800/80 p-1 text-white hover:bg-red-600"
              on:click={() => (uploadedImage = null)}
            >
              <RefreshCw size={14} />
            </button>
          </div>
        {/if}
      </div>
    {/if}

    <!-- Actions -->
    <div class="mt-6 flex justify-end gap-2 border-t border-zinc-100 pt-3 dark:border-zinc-800">
      <button
        class="rounded-xl border border-zinc-200 px-4 py-2 text-xs font-semibold text-zinc-600 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
        on:click={onClose}
      >
        İptal
      </button>
      <button
        class="flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white hover:bg-indigo-500 disabled:opacity-50"
        disabled={(activeTab === 'draw' && !hasDrawn) || (activeTab === 'upload' && !uploadedImage) || (activeTab === 'type' && !typedName.trim())}
        on:click={submit}
      >
        <Check size={14} /> İmzayı Sayfaya Ekle
      </button>
    </div>
  </div>
</div>
