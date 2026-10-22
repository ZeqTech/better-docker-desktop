// App settings: typed defaults, validation of stored values, persistence. No React here.

export type PageId = "containers" | "images" | "volumes" | "builds";
export type AccentId = "burgundy" | "crimson" | "rose";

export interface Settings {
  autoStartEngine: boolean;
  startPage: "last" | PageId;
  confirmDestructive: boolean;
  refreshSeconds: number; // 0 = manual refresh only
  onlyRunning: boolean;
  logTail: number;
  logTimestamps: boolean;
  logWrap: boolean;
  dockerPath: string; // empty = docker from PATH
  accent: AccentId;
  compact: boolean;
}

export const DEFAULTS: Settings = {
  autoStartEngine: true,
  startPage: "last",
  confirmDestructive: true,
  refreshSeconds: 3,
  onlyRunning: false,
  logTail: 500,
  logTimestamps: true,
  logWrap: true,
  dockerPath: "",
  accent: "burgundy",
  compact: false,
};

export const PAGES: PageId[] = ["containers", "images", "volumes", "builds"];
export const REFRESH_CHOICES = [0, 2, 3, 5, 10, 30];
export const LOG_TAIL_CHOICES = [100, 500, 1000, 5000];

export const ACCENTS: Record<AccentId, { label: string; accent: string; hi: string; soft: string }> = {
  burgundy: { label: "Burgundy", accent: "#b3203f", hi: "#d6304f", soft: "rgba(179, 32, 63, 0.22)" },
  crimson: { label: "Crimson", accent: "#c4122f", hi: "#ef2a4a", soft: "rgba(196, 18, 47, 0.24)" },
  rose: { label: "Rosewood", accent: "#a13a56", hi: "#c9587a", soft: "rgba(161, 58, 86, 0.25)" },
};

const KEY = "settings.v1";

/** Merge stored JSON over the defaults, keeping only fields that have the right type and an allowed value. */
export function parseSettings(raw: string | null): Settings {
  const out: Settings = { ...DEFAULTS };
  if (!raw) return out;
  let v: Record<string, unknown>;
  try {
    const p = JSON.parse(raw);
    if (!p || typeof p !== "object" || Array.isArray(p)) return out;
    v = p as Record<string, unknown>;
  } catch {
    return out;
  }
  const bool = (k: "autoStartEngine" | "confirmDestructive" | "onlyRunning" | "logTimestamps" | "logWrap" | "compact") => {
    if (typeof v[k] === "boolean") out[k] = v[k] as boolean;
  };
  bool("autoStartEngine");
  bool("confirmDestructive");
  bool("onlyRunning");
  bool("logTimestamps");
  bool("logWrap");
  bool("compact");
  if (v.startPage === "last" || PAGES.includes(v.startPage as PageId)) out.startPage = v.startPage as Settings["startPage"];
  if (REFRESH_CHOICES.includes(v.refreshSeconds as number)) out.refreshSeconds = v.refreshSeconds as number;
  if (LOG_TAIL_CHOICES.includes(v.logTail as number)) out.logTail = v.logTail as number;
  if (typeof v.dockerPath === "string" && v.dockerPath.length <= 500) out.dockerPath = v.dockerPath;
  if (typeof v.accent === "string" && v.accent in ACCENTS) out.accent = v.accent as AccentId;
  return out;
}

export function loadSettings(): Settings {
  try {
    return parseSettings(localStorage.getItem(KEY));
  } catch {
    return { ...DEFAULTS };
  }
}

export function saveSettings(s: Settings): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {
    /* storage unavailable: settings last for this run only */
  }
}

/** Polling interval in ms for lists; 0 disables polling. */
export function pollMs(s: Settings, base = 1): number {
  return s.refreshSeconds * 1000 * base;
}

/** Validate engine-config text as the UI types it. Returns an error message, or null when fine. */
export function checkEngineJson(text: string): string | null {
  if (!text.trim()) return null;
  try {
    const v = JSON.parse(text);
    return v && typeof v === "object" && !Array.isArray(v) ? null : "Must be a JSON object, like { }";
  } catch (e) {
    return e instanceof Error ? e.message : "Invalid JSON";
  }
}
