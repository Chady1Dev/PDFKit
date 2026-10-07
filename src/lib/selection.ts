/**
 * Click / Ctrl-click / Shift-click selection helper shared by grid views.
 * Returns the new selection; mutates `anchor` (last clicked index).
 */
export function applySelection(
  e: MouseEvent,
  index: number,
  selection: Set<number>,
  anchor: { value: number | null },
): Set<number> {
  const next = new Set(selection);

  if (e.shiftKey && anchor.value !== null) {
    const [from, to] = [Math.min(anchor.value, index), Math.max(anchor.value, index)];
    if (!e.ctrlKey && !e.metaKey) next.clear();
    for (let i = from; i <= to; i++) next.add(i);
  } else if (e.ctrlKey || e.metaKey) {
    if (next.has(index)) next.delete(index);
    else next.add(index);
    anchor.value = index;
  } else {
    next.clear();
    next.add(index);
    anchor.value = index;
  }
  return next;
}

/** Collapses a sorted page list into contiguous 1-based ranges. */
export function pagesToRanges(pages: number[]): { start: number; end: number }[] {
  const sorted = [...pages].sort((a, b) => a - b);
  const ranges: { start: number; end: number }[] = [];
  for (const p of sorted) {
    const last = ranges[ranges.length - 1];
    if (last && p === last.end + 1) last.end = p;
    else ranges.push({ start: p, end: p });
  }
  return ranges;
}
