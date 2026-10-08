import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { readFile } from "@tauri-apps/plugin-fs";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

export interface TextItemData {
  str: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PdfLink {
  rect: [number, number, number, number];
  url?: string;
  destPage?: number;
}

export interface PdfFormField {
  id: string;
  name: string;
  type: "text" | "checkbox" | "choice";
  rect: [number, number, number, number];
  value: string;
  readOnly?: boolean;
}

/** Handle around an open pdf.js document. Always `destroy()` when done. */
export interface PdfDocHandle {
  path: string;
  pageCount: number;
  renderPage(page: number, scale: number): Promise<HTMLCanvasElement>;
  getPageDimensions(page: number): Promise<{ width: number; height: number }>;
  renderTextLayer(page: number, scale: number, container: HTMLElement): Promise<void>;
  getPageLinks(page: number, scale: number): Promise<PdfLink[]>;
  getPageFormFields(page: number, scale: number): Promise<PdfFormField[]>;
  getPageText(page: number): Promise<string>;
  getPageTextItems(page: number): Promise<TextItemData[]>;
  destroy(): Promise<void>;
}

/**
 * Opens a local PDF through pdf.js. File bytes are read via the Tauri fs
 * plugin (dialog-picked paths are auto-scoped) and handed to the worker —
 * nothing ever leaves the machine.
 */
export async function openPdf(path: string, customData?: Uint8Array): Promise<PdfDocHandle> {
  // Always clone customData (.slice(0)) because pdfjs.getDocument transfers the ArrayBuffer
  // to its web worker, which would otherwise detach the caller's ArrayBuffer!
  const data = customData ? customData.slice(0) : (await readFile(path));
  const doc = await pdfjs.getDocument({ data }).promise;
  let destroyed = false;

  return {
    path,
    pageCount: doc.numPages,

    async renderPage(page: number, scale: number): Promise<HTMLCanvasElement> {
      if (destroyed) throw new Error("document closed");
      const pdfPage = await doc.getPage(page);
      const viewport = pdfPage.getViewport({ scale });
      const canvas = document.createElement("canvas");
      canvas.width = Math.ceil(viewport.width);
      canvas.height = Math.ceil(viewport.height);
      const ctx = canvas.getContext("2d")!;
      await pdfPage.render({ canvasContext: ctx, viewport }).promise;
      pdfPage.cleanup();
      return canvas;
    },

    async getPageDimensions(page: number): Promise<{ width: number; height: number }> {
      if (destroyed) throw new Error("document closed");
      const pdfPage = await doc.getPage(page);
      const viewport = pdfPage.getViewport({ scale: 1.0 });
      const dims = { width: viewport.width, height: viewport.height };
      pdfPage.cleanup();
      return dims;
    },

    async renderTextLayer(page: number, scale: number, container: HTMLElement): Promise<void> {
      if (destroyed) throw new Error("document closed");
      container.innerHTML = "";
      const pdfPage = await doc.getPage(page);
      const viewport = pdfPage.getViewport({ scale });
      const textContent = await pdfPage.getTextContent();
      const textLayer = new (pdfjs as any).TextLayer({
        textContentSource: textContent,
        container,
        viewport,
      });
      await textLayer.render();
      pdfPage.cleanup();
    },

    async getPageLinks(page: number, scale: number): Promise<PdfLink[]> {
      if (destroyed) throw new Error("document closed");
      const pdfPage = await doc.getPage(page);
      const viewport = pdfPage.getViewport({ scale });
      const annotations = await pdfPage.getAnnotations();
      const links: PdfLink[] = [];
      for (const ann of annotations) {
        if (ann.subtype === "Link") {
          let destPage: number | undefined;
          if (ann.dest) {
            try {
              if (typeof ann.dest === "string") {
                const destArr = await doc.getDestination(ann.dest);
                if (destArr && destArr[0]) {
                  const pageIdx = await doc.getPageIndex(destArr[0]);
                  destPage = pageIdx + 1;
                }
              } else if (Array.isArray(ann.dest) && ann.dest[0]) {
                const pageIdx = await doc.getPageIndex(ann.dest[0]);
                destPage = pageIdx + 1;
              }
            } catch {}
          }
          if (ann.rect && ann.rect.length === 4) {
            const viewRect = viewport.convertToViewportRectangle(ann.rect);
            const x1 = Math.min(viewRect[0], viewRect[2]);
            const y1 = Math.min(viewRect[1], viewRect[3]);
            const x2 = Math.max(viewRect[0], viewRect[2]);
            const y2 = Math.max(viewRect[1], viewRect[3]);
            links.push({
              rect: [x1, y1, x2, y2],
              url: ann.url,
              destPage,
            });
          }
        }
      }
      pdfPage.cleanup();
      return links;
    },

    async getPageFormFields(page: number, scale: number): Promise<PdfFormField[]> {
      if (destroyed) throw new Error("document closed");
      const pdfPage = await doc.getPage(page);
      const viewport = pdfPage.getViewport({ scale });
      const annotations = await pdfPage.getAnnotations();
      const fields: PdfFormField[] = [];

      for (const ann of annotations) {
        if (ann.subtype === "Widget" && ann.fieldName && ann.rect && ann.rect.length === 4) {
          const viewRect = viewport.convertToViewportRectangle(ann.rect);
          const x1 = Math.min(viewRect[0], viewRect[2]);
          const y1 = Math.min(viewRect[1], viewRect[3]);
          const x2 = Math.max(viewRect[0], viewRect[2]);
          const y2 = Math.max(viewRect[1], viewRect[3]);

          let fieldType: "text" | "checkbox" | "choice" = "text";
          if (ann.fieldType === "Btn") {
            fieldType = "checkbox";
          } else if (ann.fieldType === "Ch") {
            fieldType = "choice";
          }

          fields.push({
            id: ann.id || ann.fieldName,
            name: ann.fieldName,
            type: fieldType,
            rect: [x1, y1, x2, y2],
            value: typeof ann.fieldValue === "string" ? ann.fieldValue : (ann.fieldValue ? String(ann.fieldValue) : ""),
            readOnly: Boolean(ann.readOnly),
          });
        }
      }
      pdfPage.cleanup();
      return fields;
    },


    async getPageText(page: number): Promise<string> {
      if (destroyed) throw new Error("document closed");
      const pdfPage = await doc.getPage(page);
      const textContent = await pdfPage.getTextContent();
      pdfPage.cleanup();

      if (!textContent.items || textContent.items.length === 0) {
        return "";
      }

      let text = "";
      let lastY: number | null = null;
      let lastXEnd: number | null = null;

      for (const rawItem of textContent.items) {
        const item = rawItem as any;
        const str = item.str ?? "";
        if (!str && !item.hasEOL) continue;

        const [, , , , tx, ty] = item.transform || [0, 0, 0, 0, 0, 0];
        const width = item.width || 0;

        if (lastY !== null && Math.abs(ty - lastY) > 5) {
          // Significant vertical jump -> new line
          text += "\n";
          lastXEnd = null;
        } else if (item.hasEOL) {
          text += str + "\n";
          lastY = null;
          lastXEnd = null;
          continue;
        } else if (
          lastXEnd !== null &&
          tx > lastXEnd + 3 &&
          !text.endsWith(" ") &&
          !text.endsWith("\n") &&
          !str.startsWith(" ")
        ) {
          // Horizontal gap between words
          text += " ";
        }

        text += str;
        lastY = ty;
        lastXEnd = tx + width;
      }

      return text.trim();
    },

    async getPageTextItems(page: number): Promise<TextItemData[]> {
      if (destroyed) throw new Error("document closed");
      const pdfPage = await doc.getPage(page);
      const viewport = pdfPage.getViewport({ scale: 1.0 });
      const textContent = await pdfPage.getTextContent();
      const items: TextItemData[] = [];
      for (const item of textContent.items as any[]) {
        if (!item.str || !item.str.trim()) continue;
        const [, , , scaleY, tx, ty] = item.transform;
        const [vx, vy] = viewport.convertToViewportPoint(tx, ty);
        const itemHeight = Math.abs(scaleY) || 12;
        items.push({
          str: item.str,
          x: vx,
          y: vy - itemHeight,
          width: item.width || (item.str.length * 8),
          height: itemHeight,
        });
      }
      pdfPage.cleanup();
      return items;
    },

    async destroy() {
      destroyed = true;
      await doc.destroy();
    },
  };
}
