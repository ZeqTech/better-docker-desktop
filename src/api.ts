import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { parseLines } from "./docker";
import type { RunOptions } from "./runform";

export interface Status {
  docker_version: string | null;
  context: string | null;
  daemon_running: boolean;
  error: string | null;
}

export interface EngineConfig {
  path: string;
  exists: boolean;
  content: string;
}

export interface StreamOptions {
  tail?: number;
  timestamps?: boolean;
  /** RFC 3339 time to continue from (logs only); replaces the tail. */
  since?: string;
}

export type PruneKind = "images" | "containers" | "volumes" | "buildcache";

export const api = {
  status: () => invoke<Status>("docker_status"),
  appVersion: () => invoke<string>("app_version"),
  containerLs: (container: string, path: string) => invoke<string>("container_ls", { container, path }),
  copyOut: (container: string, path: string, dest: string) => invoke<string>("container_copy_out", { container, path, dest }),
  copyIn: (container: string, src: string, dest: string) => invoke<string>("container_copy_in", { container, src, dest }),
  startEngine: () => invoke<string>("start_engine"),
  setDockerPath: (path: string) => invoke<void>("set_docker_path", { path }),
  readEngineConfig: () => invoke<EngineConfig>("read_engine_config"),
  writeEngineConfig: (content: string) => invoke<string>("write_engine_config", { content }),
  containers: async () => parseLines(await invoke<string>("list_containers")),
  stats: async () => parseLines(await invoke<string>("container_stats")),
  images: async () => parseLines(await invoke<string>("list_images")),
  diskUsage: () => invoke<string>("disk_usage"),
  buildHistory: async () => parseLines(await invoke<string>("build_history")),
  inspect: async (kind: "container" | "image" | "volume", target: string) => {
    const raw = await invoke<string>("inspect", { kind, target });
    try {
      return JSON.stringify(JSON.parse(raw), null, 2);
    } catch {
      return raw;
    }
  },
  containerAction: (id: string, action: string) => invoke<string>("container_action", { id, action }),
  imageRemove: (image: string, force: boolean) => invoke<string>("image_remove", { image, force }),
  imageTag: (source: string, target: string) => invoke<string>("image_tag", { source, target }),
  imageRun: (opts: RunOptions) => invoke<string>("image_run", { opts }),
  prune: (kind: PruneKind) => invoke<string>("prune", { kind }),
  volumeCreate: (name: string) => invoke<string>("volume_create", { name }),
  volumeRemove: (name: string, force: boolean) => invoke<string>("volume_remove", { name, force }),
  buildHistoryRemove: (reference: string) => invoke<string>("build_history_remove", { reference }),
};

export type StreamKind = "logs" | "pull" | "build" | "events" | "stats";

export interface StreamHandlers {
  onLine(line: string): void;
  onEnd(code: number | null): void;
}

/** Start a streaming command; resolves to a stop function. */
export async function startStream(
  kind: StreamKind,
  target: string,
  context: string,
  h: StreamHandlers,
  opts: StreamOptions = {},
): Promise<() => void> {
  const id = crypto.randomUUID();
  const offs: (() => void)[] = [];
  const cleanup = () => {
    offs.splice(0).forEach((f) => f());
  };
  offs.push(
    await listen<{ id: string; line: string }>("stream-line", (e) => {
      if (e.payload.id === id) h.onLine(e.payload.line);
    }),
    await listen<{ id: string; code: number | null }>("stream-end", (e) => {
      if (e.payload.id === id) {
        cleanup();
        h.onEnd(e.payload.code);
      }
    }),
  );
  try {
    await invoke("start_stream", { id, kind, target, context, tail: opts.tail, timestamps: opts.timestamps, since: opts.since });
  } catch (e) {
    cleanup();
    throw e;
  }
  return () => {
    cleanup();
    void invoke("stop_stream", { id });
  };
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
