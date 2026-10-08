import type { Annotation } from "../docStore";

export interface HistorySnapshot {
  page: number;
  annotations: Annotation[];
}

export class EditorHistoryManager {
  private undoStack: HistorySnapshot[] = [];
  private redoStack: HistorySnapshot[] = [];
  private maxDepth: number;

  constructor(maxDepth: number = 50) {
    this.maxDepth = maxDepth;
  }

  public push(snapshot: HistorySnapshot) {
    this.undoStack.push({
      page: snapshot.page,
      annotations: JSON.parse(JSON.stringify(snapshot.annotations)),
    });
    if (this.undoStack.length > this.maxDepth) {
      this.undoStack.shift();
    }
    // New action invalidates redo branch
    this.redoStack = [];
  }

  public undo(currentSnapshot: HistorySnapshot): HistorySnapshot | null {
    if (this.undoStack.length === 0) return null;
    const previous = this.undoStack.pop()!;
    this.redoStack.push({
      page: currentSnapshot.page,
      annotations: JSON.parse(JSON.stringify(currentSnapshot.annotations)),
    });
    return previous;
  }

  public redo(currentSnapshot: HistorySnapshot): HistorySnapshot | null {
    if (this.redoStack.length === 0) return null;
    const next = this.redoStack.pop()!;
    this.undoStack.push({
      page: currentSnapshot.page,
      annotations: JSON.parse(JSON.stringify(currentSnapshot.annotations)),
    });
    return next;
  }

  public canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  public canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  public clear() {
    this.undoStack = [];
    this.redoStack = [];
  }
}
