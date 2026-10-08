import type { Annotation } from "../docStore";

export const defaultHighlighterColors = [
  "#fef08a", // soft yellow
  "#bbf7d0", // soft green
  "#fed7aa", // soft orange
  "#bfdbfe", // soft blue
  "#fbcfe8", // soft pink
];

/**
 * Pixel-perfect arrow renderer with solid arrowhead and butt-capped shaft.
 */
export function renderArrow(
  ctx: CanvasRenderingContext2D,
  startX: number,
  startY: number,
  endX: number,
  endY: number,
  color: string,
  strokeWidth: number,
  opacity?: number
) {
  const dist = Math.hypot(endX - startX, endY - startY);
  if (dist < 2) return;

  ctx.save();
  if (opacity !== undefined) ctx.globalAlpha = opacity;

  const angle = Math.atan2(endY - startY, endX - startX);
  const w = Math.max(1, strokeWidth);
  const headLen = Math.min(dist * 0.75, Math.max(15, w * 3.8));
  const headAngle = Math.PI / 6;
  const shaftEndX = endX - headLen * 0.75 * Math.cos(angle);
  const shaftEndY = endY - headLen * 0.75 * Math.sin(angle);

  // 1. Shaft with butt cap to prevent stroke poking past arrowhead tip
  ctx.strokeStyle = color;
  ctx.lineWidth = w;
  ctx.lineCap = "butt";
  ctx.beginPath();
  ctx.moveTo(startX, startY);
  ctx.lineTo(shaftEndX, shaftEndY);
  ctx.stroke();

  // 2. Solid crisp arrowhead with sharp tip at (endX, endY)
  ctx.fillStyle = color;
  ctx.beginPath();
  ctx.moveTo(endX, endY);
  ctx.lineTo(
    endX - headLen * Math.cos(angle - headAngle),
    endY - headLen * Math.sin(angle - headAngle)
  );
  ctx.lineTo(
    endX - headLen * Math.cos(angle + headAngle),
    endY - headLen * Math.sin(angle + headAngle)
  );
  ctx.closePath();
  ctx.fill();

  ctx.restore();
}

/**
 * Draws a single annotation object onto a 2D canvas context.
 */
export function drawSingleAnnotation(
  offCtx: CanvasRenderingContext2D,
  item: Annotation,
  saveScale: number,
  highlighterColors: string[] = defaultHighlighterColors
) {
  offCtx.save();
  if (item.blendMode === "destination-out" && item.points) {
    offCtx.globalCompositeOperation = "destination-out";
    offCtx.strokeStyle = "rgba(0,0,0,1)";
    offCtx.lineWidth = (item.strokeWidth || 20) / saveScale;
    offCtx.lineCap = "round";
    offCtx.lineJoin = "round";
    offCtx.beginPath();
    offCtx.moveTo(item.points[0].x, item.points[0].y);
    for (let i = 1; i < item.points.length; i++) {
      offCtx.lineTo(item.points[i].x, item.points[i].y);
    }
    offCtx.stroke();
    offCtx.globalCompositeOperation = "source-over";
  } else if (item.type === "stroke" && item.points) {
    if (item.opacity !== undefined) offCtx.globalAlpha = item.opacity;
    offCtx.strokeStyle = item.color || "#000";
    offCtx.lineWidth = item.strokeWidth || 3;
    offCtx.lineCap = "round";
    offCtx.lineJoin = "round";
    offCtx.beginPath();
    offCtx.moveTo(item.points[0].x, item.points[0].y);
    for (let i = 1; i < item.points.length; i++) {
      offCtx.lineTo(item.points[i].x, item.points[i].y);
    }
    offCtx.stroke();
  } else if (item.type === "highlighter" && item.points) {
    offCtx.strokeStyle = item.color || highlighterColors[0];
    offCtx.lineWidth = 18;
    offCtx.lineCap = "round";
    offCtx.lineJoin = "round";
    offCtx.beginPath();
    offCtx.moveTo(item.points[0].x, item.points[0].y);
    for (let i = 1; i < item.points.length; i++) {
      offCtx.lineTo(item.points[i].x, item.points[i].y);
    }
    offCtx.stroke();
  } else if (item.type === "text" && item.text && item.x !== undefined && item.y !== undefined) {
    if (item.opacity !== undefined) offCtx.globalAlpha = item.opacity;
    offCtx.font = `${item.fontSize || 18}px sans-serif`;
    offCtx.fillStyle = item.color || "#9333ea";
    offCtx.textBaseline = "top";
    offCtx.fillText(item.text, item.x, item.y);
  } else if (item.type === "rect" && item.x !== undefined && item.y !== undefined) {
    if (item.opacity !== undefined) offCtx.globalAlpha = item.opacity;
    if (item.fillColor && item.fillColor !== "transparent") {
      offCtx.fillStyle = item.fillColor;
      offCtx.fillRect(item.x, item.y, item.width || 0, item.height || 0);
    }
    offCtx.strokeStyle = item.color || "#000";
    offCtx.lineWidth = item.strokeWidth || 3;
    offCtx.strokeRect(item.x, item.y, item.width || 0, item.height || 0);
  } else if (item.type === "redact" && item.x !== undefined && item.y !== undefined) {
    offCtx.fillStyle = item.fillColor || "#000000";
    offCtx.fillRect(item.x, item.y, item.width || 0, item.height || 0);
  } else if (item.type === "circle" && item.x !== undefined && item.y !== undefined) {
    if (item.opacity !== undefined) offCtx.globalAlpha = item.opacity;
    const rx = (item.width || 0) / 2;
    const ry = (item.height || 0) / 2;
    offCtx.beginPath();
    offCtx.ellipse(item.x + rx, item.y + ry, rx, ry, 0, 0, Math.PI * 2);
    if (item.fillColor && item.fillColor !== "transparent") {
      offCtx.fillStyle = item.fillColor;
      offCtx.fill();
    }
    offCtx.strokeStyle = item.color || "#000";
    offCtx.lineWidth = item.strokeWidth || 3;
    offCtx.stroke();
  } else if (item.type === "line" && item.startX !== undefined && item.startY !== undefined && item.endX !== undefined && item.endY !== undefined) {
    if (item.opacity !== undefined) offCtx.globalAlpha = item.opacity;
    offCtx.strokeStyle = item.color || "#000";
    offCtx.lineWidth = item.strokeWidth || 3;
    offCtx.beginPath();
    offCtx.moveTo(item.startX, item.startY);
    offCtx.lineTo(item.endX, item.endY);
    offCtx.stroke();
  } else if (item.type === "arrow" && item.startX !== undefined && item.startY !== undefined && item.endX !== undefined && item.endY !== undefined) {
    renderArrow(
      offCtx,
      item.startX,
      item.startY,
      item.endX,
      item.endY,
      item.color || "#000",
      item.strokeWidth || 3,
      item.opacity
    );
  } else if (item.type === "image" && item.x !== undefined && item.y !== undefined) {
    if (item.imageEl instanceof HTMLImageElement && item.imageEl.complete && item.imageEl.naturalWidth > 0) {
      offCtx.drawImage(
        item.imageEl,
        item.x,
        item.y,
        item.width || 120,
        item.height || 60
      );
    }
  }
  offCtx.restore();
}
