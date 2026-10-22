// Docker event stream: parsing, routing to the views that need a refresh, and crash detection.

export interface DockerEvent {
  type: "container" | "image" | "volume" | "";
  /** Action without any ": detail" suffix, e.g. "die", "health_status". */
  action: string;
  id: string;
  name: string;
  exitCode: string;
  signal: string;
}

/** Parse one `docker events --format '{{json .}}'` line; null for anything else. */
export function parseEvent(line: string): DockerEvent | null {
  let j: Record<string, unknown>;
  try {
    j = JSON.parse(line);
  } catch {
    return null;
  }
  if (!j || typeof j !== "object") return null;
  const type = String(j.Type ?? "");
  const actor = (j.Actor ?? {}) as { ID?: string; Attributes?: Record<string, string> };
  const attrs = actor.Attributes ?? {};
  return {
    type: type === "container" || type === "image" || type === "volume" ? type : "",
    action: String(j.Action ?? j.status ?? "").split(":")[0].trim(),
    id: String(actor.ID ?? j.id ?? ""),
    name: attrs.name ?? "",
    exitCode: attrs.exitCode ?? "",
    signal: attrs.signal ?? "",
  };
}

export interface Ticks {
  containers: number;
  images: number;
  volumes: number;
}

/** Which listings an event makes stale. */
export function affected(ev: DockerEvent): (keyof Ticks)[] {
  if (ev.type === "container") return ["containers"];
  if (ev.type === "image") return ["images"];
  if (ev.type === "volume") return ["volumes"];
  return [];
}

export interface CrashAlert {
  name: string;
  reason: string;
}

/**
 * Spots containers that died on their own. `docker stop` and `kill` emit a "kill" event just before
 * "die", so a die with a recent kill is a deliberate stop and stays silent.
 */
export class CrashDetector {
  private killed = new Map<string, number>();
  constructor(private windowMs = 15_000) {}

  feed(ev: DockerEvent, now = Date.now()): CrashAlert | null {
    if (ev.type !== "container") return null;
    if (ev.action === "kill") {
      this.killed.set(ev.id, now);
      return null;
    }
    if (ev.action === "oom") return { name: ev.name || ev.id.slice(0, 12), reason: "ran out of memory" };
    if (ev.action === "die") {
      const at = this.killed.get(ev.id);
      this.killed.delete(ev.id);
      if (at !== undefined && now - at <= this.windowMs) return null;
      if (ev.exitCode === "" || ev.exitCode === "0") return null;
      return { name: ev.name || ev.id.slice(0, 12), reason: `exited with code ${ev.exitCode}` };
    }
    return null;
  }
}
