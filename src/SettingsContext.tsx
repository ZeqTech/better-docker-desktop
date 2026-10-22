import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { ACCENTS, DEFAULTS, saveSettings, type Settings } from "./settings";

interface SettingsApi {
  settings: Settings;
  update(patch: Partial<Settings>): void;
  reset(): void;
}

const Ctx = createContext<SettingsApi>({ settings: DEFAULTS, update: () => {}, reset: () => {} });

export const useSettings = () => useContext(Ctx);

/** Applies the accent colour and density to the document and persists every change. */
export function SettingsProvider({ initial, children }: { initial: Settings; children: ReactNode }) {
  const [settings, setSettings] = useState(initial);

  useEffect(() => {
    const a = ACCENTS[settings.accent];
    const root = document.documentElement.style;
    root.setProperty("--accent", a.accent);
    root.setProperty("--accent-hi", a.hi);
    root.setProperty("--accent-soft", a.soft);
    document.documentElement.classList.toggle("compact", settings.compact);
    saveSettings(settings);
  }, [settings]);

  const update = useCallback((patch: Partial<Settings>) => setSettings((s) => ({ ...s, ...patch })), []);
  const reset = useCallback(() => setSettings((s) => ({ ...DEFAULTS, dockerPath: s.dockerPath })), []);
  const value = useMemo(() => ({ settings, update, reset }), [settings, update, reset]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
