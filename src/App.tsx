import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "./api";
import { BuildsView } from "./BuildsView";
import { CleanupDialog } from "./CleanupDialog";
import { ContainersView } from "./ContainersView";
import { ImagesView } from "./ImagesView";
import { SettingsView } from "./SettingsView";
import { useSettings } from "./SettingsContext";
import { PAGES, type PageId, type Settings } from "./settings";
import { useEngineEvents } from "./useEngineEvents";
import { Ctx, Modal, useData, type AppCtx, type ConfirmOptions } from "./ui";
import { VolumesView } from "./VolumesView";

type Page = PageId | "settings";

function initialPage(startPage: Settings["startPage"]): Page {
  if (startPage !== "last") return startPage;
  const last = localStorage.getItem("page");
  return PAGES.includes(last as PageId) ? (last as PageId) : "containers";
}

const NAV: { id: Page; label: string; icon: string }[] = [
  { id: "containers", label: "Containers", icon: "▣" },
  { id: "images", label: "Images", icon: "◈" },
  { id: "volumes", label: "Volumes", icon: "⛁" },
  { id: "builds", label: "Builds", icon: "⚒" },
];

interface Toast {
  id: number;
  text: string;
  kind: "ok" | "error";
}

export default function App() {
  const { settings } = useSettings();
  const [page, setPage] = useState<Page>(() => initialPage(settings.startPage));
  const [refreshKey, setRefreshKey] = useState(0);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [asking, setAsking] = useState<{ o: ConfirmOptions; done(v: boolean): void } | null>(null);
  const [starting, setStarting] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const status = useData(api.status, starting ? 2000 : 5000);
  const nextId = useRef(0);
  const autoTried = useRef(false);
  const startedAt = useRef(0);
  const confirmRef = useRef(settings.confirmDestructive);
  confirmRef.current = settings.confirmDestructive;

  useEffect(() => {
    if (page !== "settings") localStorage.setItem("page", page);
  }, [page]);

  const notify = useCallback((text: string, kind: "ok" | "error" = "ok") => {
    const id = nextId.current++;
    setToasts((t) => [...t, { id, text, kind }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === "error" ? 8000 : 3500);
  }, []);

  const ctx = useMemo<AppCtx>(
    () => ({
      notify,
      confirm: (o) => (confirmRef.current ? new Promise<boolean>((done) => setAsking({ o, done })) : Promise.resolve(true)),
      act: async (label, fn, after) => {
        try {
          await fn();
          notify(`${label}: done`);
          after?.();
          return true;
        } catch (e) {
          notify(`${label} failed: ${errorText(e)}`, "error");
          after?.();
          return false;
        }
      },
    }),
    [notify],
  );

  const s = status.data;
  const up = !!s?.daemon_running;
  const events = useEngineEvents(up, settings.notifyOnCrash);
  // Docker events and "Refresh all" both make the lists refetch.
  const ticks = useMemo(
    () => ({ containers: events.containers + refreshKey, images: events.images + refreshKey, volumes: events.volumes + refreshKey }),
    [events, refreshKey],
  );

  const startEngine = useCallback(async () => {
    try {
      await api.startEngine();
      startedAt.current = Date.now();
      setStarting(true);
    } catch (e) {
      notify(errorText(e), "error");
    }
  }, [notify]);

  // Launch Docker Desktop once at startup when the engine is down (but not if the CLI is missing).
  useEffect(() => {
    if (!s || s.daemon_running || autoTried.current || s.error?.startsWith("Docker CLI not found")) return;
    autoTried.current = true;
    if (settings.autoStartEngine) void startEngine();
  }, [s, startEngine, settings.autoStartEngine]);

  // While starting: stop when the engine answers, give up after three minutes.
  useEffect(() => {
    if (!starting) return;
    if (up) {
      setStarting(false);
      setRefreshKey((k) => k + 1);
      notify("Docker Engine is running");
    } else if (Date.now() - startedAt.current > 180_000) {
      setStarting(false);
      notify("Docker Engine did not start within 3 minutes", "error");
    }
  }, [starting, up, s, notify]);
  const answer = (v: boolean) => {
    asking?.done(v);
    setAsking(null);
  };

  return (
    <Ctx.Provider value={ctx}>
      <div className="app">
        <aside className="sidebar">
          <div className="brand">
            <span className="logo" aria-hidden>▣</span>
            <span>Better Docker<br />Desktop</span>
          </div>
          <nav>
            {NAV.map((n) => (
              <button key={n.id} className={page === n.id ? "active" : ""} onClick={() => setPage(n.id)}>
                <span className="nav-icon" aria-hidden>{n.icon}</span>
                {n.label}
              </button>
            ))}
          </nav>
          <div className="sidebar-foot">
            <button onClick={() => setCleaning(true)} disabled={!up}>✦ Clean up…</button>
            <button className={page === "settings" ? "active" : ""} onClick={() => setPage("settings")}>⚙ Settings</button>
            <button onClick={() => { setRefreshKey((k) => k + 1); void status.reload(); }}>↻ Refresh all</button>
          </div>
        </aside>

        <main>
          {!up && !status.loading && (
            <div className="engine-down">
              <b>{starting ? "Starting Docker Engine…" : "Docker Engine is not running."}</b>
              <span>
                {starting
                  ? "Launching Docker Desktop. This can take a minute; the window connects automatically."
                  : (s?.error ?? status.error ?? "Start Docker and this window will reconnect automatically.")}
              </span>
              <button onClick={() => void status.reload()}>Retry</button>
              {!starting && <button className="primary" onClick={() => void startEngine()}>Start Docker</button>}
            </div>
          )}
          {page === "containers" && <ContainersView refreshKey={refreshKey} ticks={ticks} />}
          {page === "images" && <ImagesView refreshKey={refreshKey} ticks={ticks} />}
          {page === "volumes" && <VolumesView refreshKey={refreshKey} ticks={ticks} />}
          {page === "builds" && <BuildsView refreshKey={refreshKey} ticks={ticks} />}
          {page === "settings" && <SettingsView status={s ?? null} onReconnect={() => { autoTried.current = true; void status.reload(); setRefreshKey((k) => k + 1); }} />}
        </main>

        <footer className="statusbar">
          <span className={`engine ${up ? "up" : "down"}`}>
            <span className={`dot ${up ? "running" : "exited"}`} />
            {status.loading ? "Connecting…" : up ? "Engine running" : starting ? "Engine starting…" : "Engine stopped"}
          </span>
          {s?.docker_version && <span>Docker {s.docker_version}</span>}
          {s?.context && <span>Context: {s.context}</span>}
        </footer>

        <div className="toasts" aria-live="polite">
          {toasts.map((t) => (
            <div key={t.id} className={`toast ${t.kind}`}>{t.text}</div>
          ))}
        </div>

        {cleaning && <CleanupDialog onClose={() => setCleaning(false)} onDone={() => setRefreshKey((k) => k + 1)} />}

        {asking && (
          <Modal title={asking.o.title} onClose={() => answer(false)}>
            {asking.o.body && <p className="confirm-body">{asking.o.body}</p>}
            <div className="form-actions">
              <button onClick={() => answer(false)}>Cancel</button>
              <button autoFocus className={asking.o.danger ? "danger-solid" : "primary"} onClick={() => answer(true)}>
                {asking.o.label ?? "OK"}
              </button>
            </div>
          </Modal>
        )}
      </div>
    </Ctx.Provider>
  );
}
