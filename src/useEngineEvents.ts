import { useEffect, useRef, useState } from "react";
import { startStream } from "./api";
import { CrashDetector, affected, parseEvent, type CrashAlert, type Ticks } from "./events";

async function sendCrashNotification(a: CrashAlert): Promise<void> {
  try {
    const n = await import("@tauri-apps/plugin-notification");
    let ok = await n.isPermissionGranted();
    if (!ok) ok = (await n.requestPermission()) === "granted";
    if (ok) n.sendNotification({ title: `${a.name} stopped`, body: `Container ${a.reason}.` });
  } catch {
    /* notifications unavailable: the list still updates */
  }
}

/**
 * Follows `docker events` while the engine is up. Returns counters that change when containers,
 * images or volumes change, so views can refetch immediately instead of waiting for their poll.
 * Also raises a desktop notification when a container dies on its own.
 */
export function useEngineEvents(up: boolean, notifyOnCrash: boolean): Ticks {
  const [ticks, setTicks] = useState<Ticks>({ containers: 0, images: 0, volumes: 0 });
  const notifyRef = useRef(notifyOnCrash);
  notifyRef.current = notifyOnCrash;

  useEffect(() => {
    if (!up) return;
    let cancelled = false;
    let stop: (() => void) | null = null;
    let retry: ReturnType<typeof setTimeout> | null = null;
    let flush: ReturnType<typeof setTimeout> | null = null;
    const pending = new Set<keyof Ticks>();
    const crashes = new CrashDetector();

    const schedule = (ms: number) => {
      if (!cancelled) retry = setTimeout(connect, ms);
    };
    const connect = () => {
      startStream("events", "", "", {
        onLine: (line) => {
          const ev = parseEvent(line);
          if (!ev) return;
          affected(ev).forEach((k) => pending.add(k));
          if (!flush) {
            // Coalesce bursts (a compose up emits dozens of events) into one refresh.
            flush = setTimeout(() => {
              flush = null;
              const keys = [...pending];
              pending.clear();
              setTicks((t) => {
                const n = { ...t };
                keys.forEach((k) => (n[k] += 1));
                return n;
              });
            }, 250);
          }
          const alert = crashes.feed(ev);
          if (alert && notifyRef.current) void sendCrashNotification(alert);
        },
        onEnd: () => schedule(3000),
      })
        .then((s) => {
          if (cancelled) s();
          else stop = s;
        })
        .catch(() => schedule(5000));
    };
    connect();
    return () => {
      cancelled = true;
      stop?.();
      if (retry) clearTimeout(retry);
      if (flush) clearTimeout(flush);
    };
  }, [up]);

  return ticks;
}
