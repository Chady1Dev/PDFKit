import { writable } from "svelte/store";

export interface RecentFile {
  path: string;
  name: string;
  lastOpened: number;
  pageCount?: number;
}

const STORAGE_KEY = "pdfkit_recent_files";
const MAX_RECENT = 10;

function loadRecentFiles(): RecentFile[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return parsed.filter((item) => item && typeof item.path === "string");
    }
  } catch (e) {
    console.warn("Error loading recent files:", e);
  }
  return [];
}

function saveRecentFiles(files: RecentFile[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(files));
  } catch (e) {
    console.warn("Error saving recent files:", e);
  }
}

export const recentFiles = writable<RecentFile[]>(loadRecentFiles());

export function addRecentFile(path: string, name: string, pageCount?: number) {
  const normPath = path.toLowerCase().replace(/\\/g, "/");
  recentFiles.update((list) => {
    const filtered = list.filter((item) => item.path.toLowerCase().replace(/\\/g, "/") !== normPath);
    const updated = [
      {
        path,
        name,
        lastOpened: Date.now(),
        pageCount,
      },
      ...filtered,
    ].slice(0, MAX_RECENT);
    saveRecentFiles(updated);
    return updated;
  });
}

export function removeRecentFile(path: string) {
  const normPath = path.toLowerCase().replace(/\\/g, "/");
  recentFiles.update((list) => {
    const updated = list.filter((item) => item.path.toLowerCase().replace(/\\/g, "/") !== normPath);
    saveRecentFiles(updated);
    return updated;
  });
}

export function clearRecentFiles() {
  saveRecentFiles([]);
  recentFiles.set([]);
}
