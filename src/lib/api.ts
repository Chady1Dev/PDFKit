import { invoke } from "@tauri-apps/api/core";

/** Mirrors `PdfInfo` in src-tauri/src/types.rs */
export interface PdfInfo {
  path: string;
  page_count: number;
  version: string;
  encrypted: boolean;
  file_size_bytes: number;
}

/** Mirrors `OpResult` in src-tauri/src/types.rs */
export interface OpResult {
  outputs: string[];
  total_pages: number;
  total_size_bytes: number;
}

/** Mirrors `CompressResult` in src-tauri/src/types.rs */
export interface CompressResult {
  output: string;
  before_bytes: number;
  after_bytes: number;
  already_optimized: boolean;
}

/** Mirrors `FormField` in src-tauri/src/types.rs */
export interface FormField {
  name: string;
  field_type: string;
  value: string;
  options?: string[];
  read_only?: boolean;
  export_value?: string | null;
}

/** Mirrors `PageRange` in src-tauri/src/commands/split.rs */
export interface PageRange {
  start: number;
  end: number;
}

/** Mirrors `OrganizePage` in src-tauri/src/commands/organize.rs */
export interface OrganizePage {
  source_page: number;
  rotate_delta: number;
}

// --- Phase 1 ---
export const getPdfInfo = (path: string) =>
  invoke<PdfInfo>("get_pdf_info", { path });

export const mergePdfs = (inputs: string[], output: string) =>
  invoke<OpResult>("merge_pdfs", { inputs, output });

export const splitPdf = (input: string, ranges: PageRange[], outputDir: string) =>
  invoke<OpResult>("split_pdf", { input, ranges, outputDir });

export const rotatePages = (input: string, pages: number[], angle: number, output: string) =>
  invoke<OpResult>("rotate_pages", { input, pages, angle, output });

export const organizePdf = (input: string, pages: OrganizePage[], output: string) =>
  invoke<OpResult>("organize_pdf", { input, pages, output });

export const imagesToPdf = (inputs: string[], output: string) =>
  invoke<OpResult>("images_to_pdf", { inputs, output });

export const writeBinaryFile = (path: string, data: Uint8Array) =>
  invoke<void>("write_binary_file", { path, data });

export type CompressLevel = "lossless" | "balanced" | "maximum";

export const compressPdf = (input: string, output: string, level?: CompressLevel) =>
  invoke<CompressResult>("compress_pdf", { input, output, level: level ?? "lossless" });

// --- Phase 2 & 4: Security ---
export interface EncryptionPermissions {
  allow_print: boolean;
  allow_copy: boolean;
  allow_modify: boolean;
  allow_annotate: boolean;
}

export const encryptPdf = (
  input: string,
  output: string,
  userPassword: string,
  ownerPassword: string,
  permissions?: EncryptionPermissions | null,
  algorithm?: string | null,
) =>
  invoke<OpResult>("encrypt_pdf", {
    input,
    output,
    userPassword,
    ownerPassword,
    permissions: permissions ?? null,
    algorithm: algorithm ?? "aes256",
  });


export const decryptPdf = (input: string, output: string, password: string) =>
  invoke<OpResult>("decrypt_pdf", { input, output, password });

export const addWatermark = (input: string, output: string, text: string, fontSize: number, opacity: number) =>
  invoke<OpResult>("add_watermark", { input, output, text, fontSize, opacity });

export interface WatermarkZones {
  top: boolean;
  center: boolean;
  bottom: boolean;
}

export const removeWatermark = (
  input: string,
  output: string,
  targetText?: string | null,
  zones?: WatermarkZones | null
) =>
  invoke<OpResult>("remove_watermark", {
    input,
    output,
    targetText: targetText || null,
    zones: zones ?? null,
  });

export const removeWatermarkBytes = (
  input?: string | null,
  pdfBytes?: number[] | Uint8Array | null,
  targetText?: string | null,
  zones?: WatermarkZones | null
) =>
  invoke<number[]>("remove_watermark_bytes", {
    input: input ?? null,
    pdfBytes: pdfBytes ? Array.from(pdfBytes) : null,
    targetText: targetText ?? null,
    zones: zones ?? null,
  }).then((res) => new Uint8Array(res));

/**
 * Unified Watermark Service: Single Source of Truth for removing watermarks.
 * Removes watermarks in-memory via the Rust engine and returns pristine, detached-safe Uint8Array bytes.
 */
export async function cleanWatermarkBytes(
  inputPath: string,
  existingBytes?: Uint8Array | null,
  targetText?: string | null,
  zones?: WatermarkZones | null
): Promise<Uint8Array> {
  const result = await removeWatermarkBytes(
    existingBytes ? null : inputPath,
    existingBytes ?? null,
    targetText ?? null,
    zones ?? null
  );
  return new Uint8Array(result);
}

export const writePdfBytes = (path: string, data: number[] | Uint8Array) =>
  invoke<OpResult>("write_pdf_bytes", {
    path,
    data: data instanceof Uint8Array ? Array.from(data) : data,
  });

export const addPageNumbers = (input: string, output: string, fontSize: number, margin: number) =>
  invoke<OpResult>("add_page_numbers", { input, output, fontSize, margin });

export const listFormFields = (input: string) =>
  invoke<FormField[]>("list_form_fields", { input });

export const fillForm = (input: string, output: string, values: Record<string, string>) =>
  invoke<OpResult>("fill_form", { input, output, values });

export const stampImage = (
  input: string,
  output: string,
  page: number,
  imagePath: string,
  x: number,
  y: number,
  width: number,
  height: number,
) => invoke<OpResult>("stamp_image", { input, output, page, imagePath, x, y, width, height });

export interface PageOverlayPayload {
  page: number;
  png_data: number[] | Uint8Array;
  replace_page_content?: boolean;
  bbox?: [number, number, number, number] | null;
}


export const stampPageOverlays = (
  input: string,
  output: string,
  overlays: PageOverlayPayload[],
  editorData?: string,
  inputBytes?: number[] | Uint8Array | null,
) =>
  invoke<OpResult>("stamp_page_overlays", {
    input,
    output,
    overlays,
    editorData: editorData ?? null,
    inputBytes: inputBytes ? (inputBytes instanceof Uint8Array ? Array.from(inputBytes) : inputBytes) : null,
  });

export interface EditorLoadResult {
  annotations_json?: string | null;
  clean_pdf_bytes?: number[] | null;
}

export const loadEditorDocument = (path: string) =>
  invoke<EditorLoadResult>("load_editor_document", { path });

// --- Printer API ---
export interface PrinterInfo {
  name: string;
  is_default: boolean;
}

export interface PrinterListResult {
  printers: PrinterInfo[];
  default_printer: string;
}

export interface MarginsMm {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export interface DirectPrintPayload {
  printer_name: string;
  doc_title: string;
  page_pngs: number[][];
  copies: number;
  collate: boolean;
  orientation: string;
  margins: MarginsMm;
  grayscale: boolean;
}

export const getSystemPrinters = () =>
  invoke<PrinterListResult>("get_system_printers");

export const openPrinterProperties = (printerName: string) =>
  invoke<boolean>("open_printer_properties", { printerName });

export const directPrintPages = (payload: DirectPrintPayload) =>
  invoke<void>("direct_print_pages", { payload });

// --- Helpers ---
export const fmtSize = (bytes: number) =>
  bytes > 1_048_576 ? `${(bytes / 1_048_576).toFixed(1)} MB` : `${Math.max(1, bytes / 1024).toFixed(0)} KB`;

export const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

// --- Office Export API ---
export interface PageTextExportItem {
  page: number;
  text: string;
}

export interface ConversionProgress {
  current: number;
  total: number;
  percent: number;
  stage: string;
}

export const cancelConversion = () =>
  invoke<void>("cancel_conversion");

export const exportOfficeDocument = (
  outputPath: string,
  format: "docx" | "xlsx" | "pptx" | "txt",
  docTitle: string,
  pages: PageTextExportItem[],
  language: string = "tr",
  inputPath?: string,
) =>
  invoke<void>("export_office_document", {
    inputPath: inputPath ?? null,
    outputPath,
    format,
    docTitle,
    language,
    pages,
  });

export const revealInExplorer = (path: string) =>
  invoke<void>("reveal_in_explorer", { path });
