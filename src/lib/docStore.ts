import { writable, get } from "svelte/store";
import { openPdf, type PdfDocHandle } from "./pdf";
import { baseName, loadEditorDocument } from "./api";
import { addRecentFile } from "./recentFiles";

export interface Annotation {
  id: string;
  type: "stroke" | "highlighter" | "text" | "rect" | "circle" | "arrow" | "line" | "redact" | "image" | "underline" | "strike";
  points?: { x: number; y: number }[];
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  startX?: number;
  startY?: number;
  endX?: number;
  endY?: number;
  text?: string;
  fontSize?: number;
  color?: string;
  fillColor?: string;
  strokeWidth?: number;
  opacity?: number;
  blendMode?: string;
  imageData?: string;
  imageEl?: HTMLImageElement;
  rects?: { x: number; y: number; width: number; height: number }[];
  locked?: boolean;
}

export interface OpenDocument {
  id: string;
  path: string;
  name: string;
  handle: PdfDocHandle;
  pageCount: number;
  currentPage: number;
  scale: number;
  annotations: Record<number, Annotation[]>;
  hasUnsavedChanges: boolean;
  cleanPdfBytes?: Uint8Array;
}

export const openDocs = writable<OpenDocument[]>([]);
export const activeDocId = writable<string | null>(null);

const inFlightOpens = new Map<string, Promise<string>>();

/** Opens a PDF file. If already open and not forceReload, activates it. Otherwise creates a new tab or refreshes. */
export async function openPdfDocument(path: string, forceReload = false): Promise<string> {
  const normPath = path.toLowerCase().replace(/\\/g, "/");
  const current = get(openDocs);
  const existing = current.find((d) => d.path.toLowerCase().replace(/\\/g, "/") === normPath);
  if (existing && !forceReload) {
    activeDocId.set(existing.id);
    return existing.id;
  }

  if (inFlightOpens.has(normPath)) {
    const existingPromise = inFlightOpens.get(normPath)!;
    return existingPromise;
  }

  const openPromise = (async () => {
    try {
      let loadedAnnotations: Record<number, Annotation[]> = {};
      let customBytes: Uint8Array | undefined = undefined;

      try {
        const res = await loadEditorDocument(path);
        if (res.clean_pdf_bytes && res.clean_pdf_bytes.length > 0) {
          customBytes = res.clean_pdf_bytes instanceof Uint8Array
            ? res.clean_pdf_bytes
            : new Uint8Array(res.clean_pdf_bytes);
        }
        if (res.annotations_json) {
          loadedAnnotations = JSON.parse(res.annotations_json);
        }
      } catch (e) {
        console.warn("loadEditorDocument info:", e);
      }

      // Also check localStorage fallback cache if empty
      if (Object.keys(loadedAnnotations).length === 0) {
        try {
          const cached = localStorage.getItem("pdfkit_ann_" + path);
          if (cached) {
            loadedAnnotations = JSON.parse(cached);
          }
        } catch {}
      }

      // Rehydrate any image elements from base64 imageData
      for (const pageAnns of Object.values(loadedAnnotations)) {
        for (const ann of pageAnns) {
          if (ann.type === "image" && ann.imageData && typeof ann.imageData === "string") {
            const img = new Image();
            img.src = ann.imageData;
            ann.imageEl = img;
          }
        }
      }

      let handle: PdfDocHandle;
      if (customBytes && customBytes.length > 0) {
        try {
          handle = await openPdf(path, customBytes);
        } catch (e) {
          console.warn("clean_pdf_bytes could not be loaded, using original file directly:", e);
          handle = await openPdf(path);
        }
      } else {
        handle = await openPdf(path);
      }

      const id = `doc_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
      const newDoc: OpenDocument = {
        id,
        path,
        name: baseName(path),
        handle,
        pageCount: handle.pageCount,
        currentPage: 1,
        scale: 1.5,
        annotations: loadedAnnotations,
        hasUnsavedChanges: false,
      };

      openDocs.update((docs) => {
        // Double check within atomic update to be 100% sure no duplicate
        const alreadyIn = docs.find((d) => d.path.toLowerCase().replace(/\\/g, "/") === normPath);
        if (alreadyIn) return docs;
        return [...docs, newDoc];
      });
      activeDocId.set(id);
      addRecentFile(path, newDoc.name, newDoc.pageCount);
      return id;
    } finally {
      inFlightOpens.delete(normPath);
    }
  })();

  inFlightOpens.set(normPath, openPromise);
  return openPromise;
}

/** Updates document path and name (e.g. after Save As). */
export function updateDocPath(docId: string, newPath: string) {
  openDocs.update((docs) =>
    docs.map((d) => (d.id === docId ? { ...d, path: newPath, name: baseName(newPath) } : d))
  );
}

/** Closes an open document and destroys its pdf.js handle. */
export async function closePdfDocument(id: string): Promise<void> {
  const docs = get(openDocs);
  const doc = docs.find((d) => d.id === id);
  if (doc) {
    try {
      await doc.handle.destroy();
    } catch (e) {
      console.warn("Error destroying doc handle:", e);
    }
  }

  const remaining = docs.filter((d) => d.id !== id);
  openDocs.set(remaining);

  if (get(activeDocId) === id) {
    activeDocId.set(remaining.length > 0 ? remaining[remaining.length - 1].id : null);
  }
}

/** Updates annotations for a specific page of a document. */
export function setPageAnnotations(docId: string, page: number, anns: Annotation[]) {
  openDocs.update((docs) =>
    docs.map((d) => {
      if (d.id !== docId) return d;
      return {
        ...d,
        annotations: {
          ...d.annotations,
          [page]: anns,
        },
        hasUnsavedChanges: true,
      };
    })
  );
}

/** Marks document as modified (e.g. after watermark removal or other non-annotation edit). */
export function markDocumentModified(docId: string, cleanPdfBytes?: Uint8Array) {
  openDocs.update((docs) =>
    docs.map((d) => {
      if (d.id !== docId) return d;
      return {
        ...d,
        hasUnsavedChanges: true,
        ...(cleanPdfBytes ? { cleanPdfBytes } : {}),
      };
    })
  );
}

/** Clears stored in-memory clean bytes for a document. */
export function clearDocumentCleanBytes(docId: string) {
  openDocs.update((docs) =>
    docs.map((d) => (d.id === docId ? { ...d, cleanPdfBytes: undefined } : d))
  );
}

/** Clears the unsaved changes flag (e.g. after successful save). */
export function markDocumentSaved(docId: string) {
  openDocs.update((docs) =>
    docs.map((d) => (d.id === docId ? { ...d, hasUnsavedChanges: false, cleanPdfBytes: undefined } : d))
  );
}

/** Sets active page for a document. */
export function updateDocPage(docId: string, page: number) {
  openDocs.update((docs) =>
    docs.map((d) => (d.id === docId ? { ...d, currentPage: page } : d))
  );
}

/** Sets zoom scale for a document. */
export function updateDocScale(docId: string, scale: number) {
  openDocs.update((docs) =>
    docs.map((d) => (d.id === docId ? { ...d, scale } : d))
  );
}

/** Checks whether any open document has unsaved annotations. */
export function hasAnyUnsavedChanges(): boolean {
  return get(openDocs).some((d) => d.hasUnsavedChanges);
}
