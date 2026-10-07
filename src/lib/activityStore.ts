import { writable } from "svelte/store";

export interface ActivityLog {
  id: string;
  files: string[];
  action: string;
  outputPath?: string;
  timestamp: string;
  timeMs: number;
}

const STORAGE_KEY = "pdfkit_activity_logs";

function loadLogs(): ActivityLog[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    return JSON.parse(raw);
  } catch (e) {
    console.warn("Failed to load activity logs:", e);
    return [];
  }
}

export const activityLogs = writable<ActivityLog[]>(loadLogs());

function saveLogs(logs: ActivityLog[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(logs));
  } catch (e) {
    console.warn("Failed to save activity logs:", e);
  }
}

function formatTurkishDateTime(d: Date): string {
  const pad = (n: number) => n.toString().padStart(2, "0");
  const day = pad(d.getDate());
  const month = pad(d.getMonth() + 1);
  const year = d.getFullYear();
  const hours = pad(d.getHours());
  const minutes = pad(d.getMinutes());
  const seconds = pad(d.getSeconds());
  return `${day}.${month}.${year} / ${hours}:${minutes}:${seconds}`;
}

export function logActivity(files: string[], action: string, outputPath?: string) {
  const now = new Date();
  const entry: ActivityLog = {
    id: Math.random().toString(36).slice(2) + Date.now().toString(36),
    files,
    action,
    outputPath,
    timestamp: formatTurkishDateTime(now),
    timeMs: now.getTime(),
  };

  activityLogs.update((list) => {
    const next = [entry, ...list];
    saveLogs(next);
    return next;
  });
}

export function clearActivityLogs() {
  activityLogs.set([]);
  saveLogs([]);
}
