import type {
  BoundingBox,
  ChangeCategory,
  ComparisonSummary,
  DiffChangeItem,
  PageAlignment,
} from "../api";

export type FilterCategory = "all" | ChangeCategory;

export type VisualDisplayMode = "side_by_side" | "highlight" | "overlay" | "heatmap" | "quick_flip";

export interface ScrollSyncTarget {
  targetPane: "left" | "right";
  targetPage: number;
  relY: number;
  relX: number;
}

/**
 * Prevents feedback scroll loops when Pane A programmatically scrolls Pane B.
 */
export class ScrollSyncGuard {
  private activeDriver: "left" | "right" | null = null;
  private lockUntil = 0;

  public canProcessScroll(pane: "left" | "right", syncEnabled: boolean, now = Date.now()): boolean {
    if (!syncEnabled) return false;
    if (this.activeDriver !== null && this.activeDriver !== pane && now < this.lockUntil) {
      return false;
    }
    this.activeDriver = pane;
    this.lockUntil = now + 140;
    return true;
  }

  public markProgrammaticScroll(driverPane: "left" | "right", now = Date.now()): void {
    this.activeDriver = driverPane;
    this.lockUntil = now + 140;
  }

  public reset(): void {
    this.activeDriver = null;
    this.lockUntil = 0;
  }
}

/**
 * Maps a scroll position `(sourcePage, relY, relX)` in one pane to the aligned page and
 * relative offset in the opposite pane using the PageAlignment model.
 * Never blindly binds by identical page number when pages were inserted, deleted, or moved.
 */
export function computeSyncedScrollTarget(
  sourcePane: "left" | "right",
  sourcePage: number,
  relY: number,
  relX: number,
  syncEnabled: boolean,
  alignments: PageAlignment[],
): ScrollSyncTarget | null {
  if (!syncEnabled || alignments.length === 0) {
    return null;
  }

  const clampedY = Math.max(0, Math.min(1, Number.isFinite(relY) ? relY : 0));
  const clampedX = Math.max(0, Math.min(1, Number.isFinite(relX) ? relX : 0));
  const targetPane = sourcePane === "left" ? "right" : "left";

  // 1. Direct match in alignments
  const exactIdx = alignments.findIndex((a) =>
    sourcePane === "left" ? a.left_page === sourcePage : a.right_page === sourcePage,
  );

  if (exactIdx !== -1) {
    const align = alignments[exactIdx];
    const directTarget = sourcePane === "left" ? align.right_page : align.left_page;
    if (directTarget !== null) {
      return {
        targetPane,
        targetPage: directTarget,
        relY: clampedY,
        relX: clampedX,
      };
    }

    // Source page is an Added or Deleted page without a direct counterpart.
    // Find nearest preceding or succeeding matched alignment anchor without wild jumping.
    for (let offset = 1; offset < alignments.length; offset++) {
      const prev = exactIdx - offset >= 0 ? alignments[exactIdx - offset] : null;
      if (prev) {
        const prevTarget = sourcePane === "left" ? prev.right_page : prev.left_page;
        if (prevTarget !== null) {
          return {
            targetPane,
            targetPage: prevTarget,
            relY: 1.0,
            relX: clampedX,
          };
        }
      }
      const next = exactIdx + offset < alignments.length ? alignments[exactIdx + offset] : null;
      if (next) {
        const nextTarget = sourcePane === "left" ? next.right_page : next.left_page;
        if (nextTarget !== null) {
          return {
            targetPane,
            targetPage: nextTarget,
            relY: 0.0,
            relX: clampedX,
          };
        }
      }
    }
  }

  return null;
}

/**
 * Filters changes by category and optional search query.
 */
export function filterChanges(
  changes: DiffChangeItem[],
  category: FilterCategory,
  searchQuery = "",
): DiffChangeItem[] {
  const q = searchQuery.trim().toLocaleLowerCase("tr-TR");
  return changes.filter((c) => {
    if (category !== "all" && c.category !== category) {
      return false;
    }
    if (q.length > 0) {
      const hay = `${c.title} ${c.detail} ${c.before_text ?? ""} ${c.after_text ?? ""}`.toLocaleLowerCase("tr-TR");
      if (!hay.includes(q)) return false;
    }
    return true;
  });
}

/**
 * Recomputes summary statistics over the currently filtered changes and alignments.
 */
export function computeFilteredSummary(
  filteredChanges: DiffChangeItem[],
  alignments: PageAlignment[],
): ComparisonSummary {
  const summary: ComparisonSummary = {
    total_changes: filteredChanges.length,
    text_added_count: 0,
    text_deleted_count: 0,
    text_modified_count: 0,
    text_moved_count: 0,
    formatting_changes_count: 0,
    visual_changes_count: 0,
    image_graphic_changes_count: 0,
    annotation_changes_count: 0,
    form_changes_count: 0,
    header_footer_changes_count: 0,
    background_changes_count: 0,
    pages_added_count: 0,
    pages_deleted_count: 0,
    pages_moved_count: 0,
    pages_modified_count: 0,
    pages_identical_count: 0,
    changed_left_pages: [],
    changed_right_pages: [],
  };

  const leftSet = new Set<number>();
  const rightSet = new Set<number>();

  for (const c of filteredChanges) {
    if (c.left_page !== null) leftSet.add(c.left_page);
    if (c.right_page !== null) rightSet.add(c.right_page);

    switch (c.kind) {
      case "text_added":
        summary.text_added_count++;
        break;
      case "text_deleted":
        summary.text_deleted_count++;
        break;
      case "text_modified":
      case "line_paragraph_changed":
        summary.text_modified_count++;
        break;
      case "text_moved":
        summary.text_moved_count++;
        break;
      case "formatting_changed":
      case "layout_shifted":
        summary.formatting_changes_count++;
        break;
      case "visual_region_diff":
        summary.visual_changes_count++;
        break;
      case "image_added":
      case "image_deleted":
      case "image_modified":
      case "image_moved":
      case "vector_changed":
      case "table_changed":
        summary.image_graphic_changes_count++;
        break;
      case "annotation_added":
      case "annotation_deleted":
      case "annotation_modified":
        summary.annotation_changes_count++;
        break;
      case "form_field_added":
      case "form_field_deleted":
      case "form_field_modified":
        summary.form_changes_count++;
        break;
      case "header_footer_changed":
        summary.header_footer_changes_count++;
        break;
      case "background_changed":
        summary.background_changes_count++;
        break;
      case "page_added":
        summary.pages_added_count++;
        break;
      case "page_deleted":
        summary.pages_deleted_count++;
        break;
      case "page_moved":
        summary.pages_moved_count++;
        break;
      case "page_geometry_changed":
        break;
    }
  }

  for (const a of alignments) {
    if (a.status === "identical") summary.pages_identical_count++;
    else if (a.status === "modified") summary.pages_modified_count++;
  }

  summary.changed_left_pages = Array.from(leftSet).sort((a, b) => a - b);
  summary.changed_right_pages = Array.from(rightSet).sort((a, b) => a - b);
  return summary;
}

export function navigateChangeIndex(
  currentIndex: number,
  total: number,
  direction: "first" | "prev" | "next",
): number {
  if (total <= 0) return -1;
  if (direction === "first") return 0;
  if (currentIndex < 0) return 0;
  if (direction === "prev") {
    return (currentIndex - 1 + total) % total;
  }
  return (currentIndex + 1) % total;
}

export interface CanvasVisualDiffOutput {
  diffRatio: number;
  changedPixels: number;
  geometryCompatible: boolean;
  geometryWarning: string | null;
  regionsPt: BoundingBox[];
  heatmapCanvas: HTMLCanvasElement | null;
}

/**
 * Performs pixel-level visual comparison on two rendered pdf.js canvases with
 * anti-aliasing edge jitter filtering and connected-component bounding-box clustering.
 */
export function computeCanvasVisualDiff(
  leftCanvas: HTMLCanvasElement,
  rightCanvas: HTMLCanvasElement,
  leftDimsPt: [number, number],
  rightDimsPt: [number, number],
  tolerance = 24,
  ignoreAntialiasing = true,
  excludeRectsPt: BoundingBox[] = [],
): CanvasVisualDiffOutput {
  const wl = leftCanvas.width;
  const hl = leftCanvas.height;
  const wr = rightCanvas.width;
  const hr = rightCanvas.height;

  const lwPt = Math.max(1, leftDimsPt[0]);
  const lhPt = Math.max(1, leftDimsPt[1]);
  const rwPt = Math.max(1, rightDimsPt[0]);
  const rhPt = Math.max(1, rightDimsPt[1]);

  const wRel = Math.abs(lwPt - rwPt) / Math.max(lwPt, rwPt);
  const hRel = Math.abs(lhPt - rhPt) / Math.max(lhPt, rhPt);
  const geometryCompatible = wRel <= 0.025 && hRel <= 0.025;
  const geometryWarning = !geometryCompatible
    ? `Sayfa boyutları farklı (${Math.round(lwPt)}x${Math.round(lhPt)} pt vs ${Math.round(rwPt)}x${Math.round(rhPt)} pt); ortak kesişim alanı karşılaştırıldı.`
    : null;

  const cmpW = Math.min(wl, wr);
  const cmpH = Math.min(hl, hr);
  if (cmpW <= 0 || cmpH <= 0) {
    return {
      diffRatio: 0,
      changedPixels: 0,
      geometryCompatible,
      geometryWarning,
      regionsPt: [],
      heatmapCanvas: null,
    };
  }

  const ctxL = leftCanvas.getContext("2d");
  const ctxR = rightCanvas.getContext("2d");
  if (!ctxL || !ctxR) {
    return {
      diffRatio: 0,
      changedPixels: 0,
      geometryCompatible,
      geometryWarning,
      regionsPt: [],
      heatmapCanvas: null,
    };
  }

  const dataL = ctxL.getImageData(0, 0, cmpW, cmpH).data;
  const dataR = ctxR.getImageData(0, 0, cmpW, cmpH).data;

  const hmCanvas = document.createElement("canvas");
  hmCanvas.width = cmpW;
  hmCanvas.height = cmpH;
  const hmCtx = hmCanvas.getContext("2d")!;
  const hmImgData = hmCtx.createImageData(cmpW, cmpH);
  const hmData = hmImgData.data;

  const cellSize = 8;
  const gridCols = Math.ceil(cmpW / cellSize);
  const gridRows = Math.ceil(cmpH / cellSize);
  const gridCounts = new Uint16Array(gridCols * gridRows);

  let changedPixels = 0;
  const totalPixels = cmpW * cmpH;
  const effectiveTol = Math.max(24, tolerance);

  for (let y = 0; y < cmpH; y++) {
    const rowOff = y * cmpW;
    for (let x = 0; x < cmpW; x++) {
      const idx = (rowOff + x) * 4;
      const dr = Math.abs(dataL[idx] - dataR[idx]);
      const dg = Math.abs(dataL[idx + 1] - dataR[idx + 1]);
      const db = Math.abs(dataL[idx + 2] - dataR[idx + 2]);
      const maxDelta = Math.max(dr, dg, db);

      if (maxDelta <= effectiveTol) continue;

      if (
        ignoreAntialiasing &&
        isPixelAntialiased(dataL, dataR, x, y, cmpW, cmpH, effectiveTol)
      ) {
        continue;
      }

      changedPixels++;
      const gx = Math.floor(x / cellSize);
      const gy = Math.floor(y / cellSize);
      gridCounts[gy * gridCols + gx]++;

      const alpha = Math.min(235, Math.max(70, Math.round((maxDelta / 255) * 235)));
      hmData[idx] = 244;
      hmData[idx + 1] = maxDelta > 140 ? 40 : 145;
      hmData[idx + 2] = maxDelta > 140 ? 110 : 25;
      hmData[idx + 3] = alpha;
    }
  }

  hmCtx.putImageData(hmImgData, 0, 0);

  const scaleXPt = lwPt / wl;
  const scaleYPt = lhPt / hl;
  const rawBoxes: BoundingBox[] = [];

  for (let gy = 0; gy < gridRows; gy++) {
    for (let gx = 0; gx < gridCols; gx++) {
      const cellCount = gridCounts[gy * gridCols + gx];
      if (cellCount < 6) continue;

      // Verify that the 8x8 cell has a genuine mean RGB / ink change, not just resampled edge softness
      if (
        ignoreAntialiasing &&
        !hasGenuineCellVisualChange(dataL, dataR, gx * cellSize, gy * cellSize, cellSize, cmpW, cmpH)
      ) {
        continue;
      }

      rawBoxes.push({
        x: gx * cellSize * scaleXPt,
        y: gy * cellSize * scaleYPt,
        width: cellSize * scaleXPt,
        height: cellSize * scaleYPt,
      });
    }
  }

  const merged = mergeBoxes(rawBoxes, 6).filter(
    (b) => b.width >= 8 && b.height >= 6 && b.width * b.height >= 48,
  );
  const filtered = merged.filter(
    (b) =>
      !excludeRectsPt.some(
        (ex) => boxOverlapRatio(b, ex) >= 0.45 || boxesNear(b, ex, 2.5),
      ),
  );

  return {
    diffRatio: totalPixels > 0 ? changedPixels / totalPixels : 0,
    changedPixels,
    geometryCompatible,
    geometryWarning,
    regionsPt: filtered,
    heatmapCanvas: filtered.length > 0 ? hmCanvas : null,
  };
}

function isPixelAntialiased(
  dataL: Uint8ClampedArray,
  dataR: Uint8ClampedArray,
  x: number,
  y: number,
  w: number,
  h: number,
  tol: number,
): boolean {
  if (x <= 1 || y <= 1 || x + 2 >= w || y + 2 >= h) return false;
  const cIdx = (y * w + x) * 4;
  const lr = dataL[cIdx];
  const lg = dataL[cIdx + 1];
  const lb = dataL[cIdx + 2];
  const rr = dataR[cIdx];
  const rg = dataR[cIdx + 1];
  const rb = dataR[cIdx + 2];

  let lMatchesR = false;
  let rMatchesL = false;
  let maxGrad = 0;

  let sumRL = 0, sumGL = 0, sumBL = 0;
  let sumRR = 0, sumGR = 0, sumBR = 0;
  let minLumL = 255, maxLumL = 0;
  let minLumR = 255, maxLumR = 0;
  let maxChromaL = 0, maxChromaR = 0;
  let count = 0;

  for (let dy = -2; dy <= 2; dy++) {
    const ny = y + dy;
    for (let dx = -2; dx <= 2; dx++) {
      const nIdx = (ny * w + (x + dx)) * 4;
      const nlr = dataL[nIdx];
      const nlg = dataL[nIdx + 1];
      const nlb = dataL[nIdx + 2];
      const nrr = dataR[nIdx];
      const nrg = dataR[nIdx + 1];
      const nrb = dataR[nIdx + 2];

      sumRL += nlr;
      sumGL += nlg;
      sumBL += nlb;
      sumRR += nrr;
      sumGR += nrg;
      sumBR += nrb;
      count++;

      const chromaL = Math.max(Math.abs(nlr - nlg), Math.abs(nlg - nlb), Math.abs(nlr - nlb));
      const chromaR = Math.max(Math.abs(nrr - nrg), Math.abs(nrg - nrb), Math.abs(nrr - nrb));
      if (chromaL > maxChromaL) maxChromaL = chromaL;
      if (chromaR > maxChromaR) maxChromaR = chromaR;

      const lumL = (nlr * 77 + nlg * 150 + nlb * 29) >> 8;
      const lumR = (nrr * 77 + nrg * 150 + nrb * 29) >> 8;
      if (lumL < minLumL) minLumL = lumL;
      if (lumL > maxLumL) maxLumL = lumL;
      if (lumR < minLumR) minLumR = lumR;
      if (lumR > maxLumR) maxLumR = lumR;

      if (dx >= -1 && dx <= 1 && dy >= -1 && dy <= 1 && (dx !== 0 || dy !== 0)) {
        const grad = Math.max(
          Math.abs(lr - nlr),
          Math.abs(lg - nlg),
          Math.abs(lb - nlb),
          Math.abs(rr - nrr),
          Math.abs(rg - nrg),
          Math.abs(rb - nrb),
        );
        if (grad > maxGrad) maxGrad = grad;
      }

      const dLtoNR = Math.max(
        Math.abs(lr - nrr),
        Math.abs(lg - nrg),
        Math.abs(lb - nrb),
      );
      if (dLtoNR <= tol + 12) lMatchesR = true;

      const dRtoNL = Math.max(
        Math.abs(rr - nlr),
        Math.abs(rg - nlg),
        Math.abs(rb - nlb),
      );
      if (dRtoNL <= tol + 12) rMatchesL = true;
    }
  }

  // 1. Standard 1-2px edge anti-aliasing shift
  if (maxGrad > 45 && lMatchesR && rMatchesL) {
    return true;
  }

  // 2. Resampling / font-hinting / raster-flattening invariance:
  // When a page is flattened at 2.5x and downscaled to 1.25x, both 5x5 patches still contain
  // the stroke/border (neither side is flat blank paper) and the local mean RGB does not shift
  // like a solid fill or redaction box.
  const meanDelta =
    Math.max(
      Math.abs(sumRL - sumRR),
      Math.abs(sumGL - sumGR),
      Math.abs(sumBL - sumBR),
    ) / count;
  const rangeLumL = maxLumL - minLumL;
  const rangeLumR = maxLumR - minLumR;
  const bothHaveFeature = rangeLumL >= 22 && rangeLumR >= 18;
  const sameBackground = Math.abs(maxLumL - maxLumR) <= 42;

  if (bothHaveFeature && sameBackground && meanDelta <= 36) {
    return true;
  }

  return false;
}

function hasGenuineCellVisualChange(
  dataL: Uint8ClampedArray,
  dataR: Uint8ClampedArray,
  startX: number,
  startY: number,
  cellSize: number,
  w: number,
  h: number,
): boolean {
  const endX = Math.min(w, startX + cellSize);
  const endY = Math.min(h, startY + cellSize);
  let sumRL = 0, sumGL = 0, sumBL = 0;
  let sumRR = 0, sumGR = 0, sumBR = 0;
  let minLumL = 255, minLumR = 255;
  let maxLumL = 0, maxLumR = 0;
  let count = 0;

  for (let y = startY; y < endY; y++) {
    const rowOff = y * w;
    for (let x = startX; x < endX; x++) {
      const idx = (rowOff + x) * 4;
      const lr = dataL[idx], lg = dataL[idx + 1], lb = dataL[idx + 2];
      const rr = dataR[idx], rg = dataR[idx + 1], rb = dataR[idx + 2];
      sumRL += lr; sumGL += lg; sumBL += lb;
      sumRR += rr; sumGR += rg; sumBR += rb;
      const lumL = (lr * 77 + lg * 150 + lb * 29) >> 8;
      const lumR = (rr * 77 + rg * 150 + rb * 29) >> 8;
      if (lumL < minLumL) minLumL = lumL;
      if (lumL > maxLumL) maxLumL = lumL;
      if (lumR < minLumR) minLumR = lumR;
      if (lumR > maxLumR) maxLumR = lumR;
      count++;
    }
  }

  if (count === 0) return false;
  const cellMeanDelta =
    Math.max(
      Math.abs(sumRL - sumRR),
      Math.abs(sumGL - sumGR),
      Math.abs(sumBL - sumBR),
    ) / count;

  const rangeLumL = maxLumL - minLumL;
  const rangeLumR = maxLumR - minLumR;

  // Case B: One side is flat/blank (e.g. Daksil whiteout or blank paper) while the other side has strokes/content
  const oneSideFlatAndOtherHasContent =
    (rangeLumL <= 18 && rangeLumR >= 75) ||
    (rangeLumR <= 18 && rangeLumL >= 75);

  // Case C: Background itself was covered (e.g. Sansür black/dark box over light page)
  const backgroundCovered = Math.abs(maxLumL - maxLumR) >= 48;

  return (
    cellMeanDelta >= 28 ||
    (cellMeanDelta >= 14 && (oneSideFlatAndOtherHasContent || backgroundCovered))
  );
}

function boxesNear(a: BoundingBox, b: BoundingBox, margin: number): boolean {
  return !(
    a.x + a.width + margin < b.x ||
    b.x + b.width + margin < a.x ||
    a.y + a.height + margin < b.y ||
    b.y + b.height + margin < a.y
  );
}

function boxOverlapRatio(a: BoundingBox, b: BoundingBox): number {
  const ix0 = Math.max(a.x, b.x);
  const iy0 = Math.max(a.y, b.y);
  const ix1 = Math.min(a.x + a.width, b.x + b.width);
  const iy1 = Math.min(a.y + a.height, b.y + b.height);
  if (ix1 <= ix0 || iy1 <= iy0) return 0;
  const inter = (ix1 - ix0) * (iy1 - iy0);
  const minArea = Math.max(1, Math.min(a.width * a.height, b.width * b.height));
  return inter / minArea;
}

function mergeBoxes(boxes: BoundingBox[], margin: number): BoundingBox[] {
  let list = [...boxes];
  while (list.length > 1) {
    let mergedAny = false;
    const next: BoundingBox[] = [];
    const used = new Array(list.length).fill(false);

    for (let i = 0; i < list.length; i++) {
      if (used[i]) continue;
      let acc = { ...list[i] };
      used[i] = true;
      for (let j = i + 1; j < list.length; j++) {
        if (!used[j] && boxesNear(acc, list[j], margin)) {
          const minX = Math.min(acc.x, list[j].x);
          const minY = Math.min(acc.y, list[j].y);
          const maxX = Math.max(acc.x + acc.width, list[j].x + list[j].width);
          const maxY = Math.max(acc.y + acc.height, list[j].y + list[j].height);
          acc = { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
          used[j] = true;
          mergedAny = true;
        }
      }
      next.push(acc);
    }
    list = next;
    if (!mergedAny) break;
  }
  return list;
}
