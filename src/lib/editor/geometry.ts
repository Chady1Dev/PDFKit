import type { Annotation } from "../docStore";

export interface BoundingBox {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
  width: number;
  height: number;
}

/**
 * Calculates the bounding box for an array of annotations in unscaled page coordinates.
 */
export function calculateAnnotationsBoundingBox(
  items: Annotation[],
  unscaledWidth: number,
  unscaledHeight: number,
  padding: number = 8
): BoundingBox | null {
  if (!items || items.length === 0) return null;

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const item of items) {
    if (item.type === "pen" || item.type === "stroke" || item.type === "highlighter") {
      const pts = item.points || [];
      for (const pt of pts) {
        if (pt.x < minX) minX = pt.x;
        if (pt.y < minY) minY = pt.y;
        if (pt.x > maxX) maxX = pt.x;
        if (pt.y > maxY) maxY = pt.y;
      }
      const pad = (item.strokeWidth || 10) * 1.5;
      minX -= pad; minY -= pad; maxX += pad; maxY += pad;
    } else if (item.type === "arrow" || item.type === "line") {
      const sx = item.startX ?? item.x ?? 0;
      const sy = item.startY ?? item.y ?? 0;
      const ex = item.endX ?? sx;
      const ey = item.endY ?? sy;
      minX = Math.min(minX, sx - 15, ex - 15);
      minY = Math.min(minY, sy - 15, ey - 15);
      maxX = Math.max(maxX, sx + 15, ex + 15);
      maxY = Math.max(maxY, sy + 15, ey + 15);
    } else {
      const x = item.x ?? 0;
      const y = item.y ?? 0;
      const w = item.width || (item.type === "text" ? 250 : 50);
      const h = item.height || (item.type === "text" ? 50 : 50);
      minX = Math.min(minX, x - padding);
      minY = Math.min(minY, y - padding);
      maxX = Math.max(maxX, x + w + padding);
      maxY = Math.max(maxY, y + h + padding);
    }
  }

  if (!isFinite(minX) || !isFinite(minY) || !isFinite(maxX) || !isFinite(maxY)) {
    return null;
  }

  minX = Math.max(0, Math.floor(minX));
  minY = Math.max(0, Math.floor(minY));
  maxX = Math.min(unscaledWidth, Math.ceil(maxX));
  maxY = Math.min(unscaledHeight, Math.ceil(maxY));

  if (maxX <= minX || maxY <= minY) return null;

  return {
    minX,
    minY,
    maxX,
    maxY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

/**
 * Checks whether point (px, py) is inside rectangle [rx, ry, rw, rh]
 */
export function isPointInRect(
  px: number,
  py: number,
  rx: number,
  ry: number,
  rw: number,
  rh: number
): boolean {
  return px >= rx && px <= rx + rw && py >= ry && py <= ry + rh;
}
