import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { formatBytes, parseItems, podState, toNodeMetrics, toNodes, toPodMetrics, toPods, type KubeNode, type KubePod } from "./kube";
import {
  CELL, HEAD, NODE_GAP, detailLevel, fitLabel, fitView, layoutMap, loadColor, nodeMemory, podKey, zoomAt,
  type NodeBox, type NodeMemory, type View,
} from "./clustermap";
import { TextPanel, type KubeSel } from "./KubeViews";
import { useSettings } from "./SettingsContext";
import { MAP_REFRESH_CHOICES } from "./settings";
import { Empty, useData } from "./ui";

type Selection = { type: "pod"; key: string } | { type: "node"; key: string } | null;

const MAX_K = 24;
const HIGHLIGHT_KEY = "kube.map.highlight";

/** Wrap a pod name into at most three short lines for the cell label. */
function wrapName(name: string, width = 11): string[] {
  const lines: string[] = [];
  let rest = name;
  while (rest && lines.length < 3) {
    lines.push(rest.slice(0, width));
    rest = rest.slice(width);
  }
  if (rest) lines[2] = `${lines[2].slice(0, width - 1)}…`;
  return lines;
}

export function KubeMap({ kube }: { kube: KubeSel }) {
  const { settings, update } = useSettings();
  // The map always shows the whole cluster; the namespace filter below only highlights.
  const data = useData(async () => {
    // Usage needs metrics-server; without it the map falls back to memory requests.
    const [n, p, nm, pm] = await Promise.all([
      api.kubeGet(kube.context, "nodes", ""),
      api.kubeGet(kube.context, "pods", ""),
      api.kubeMetrics(kube.context, "nodes").then(toNodeMetrics, () => null),
      api.kubeMetrics(kube.context, "pods").then(toPodMetrics, () => null),
    ]);
    setUpdated(new Date());
    return { nodes: toNodes(parseItems(n)), pods: toPods(parseItems(p)), nodeUsage: nm, podUsage: pm };
  }, settings.mapRefreshSeconds * 1000);
  const [updated, setUpdated] = useState<Date | null>(null);
  const [highlight, setHighlight] = useState(() => {
    try {
      return localStorage.getItem(HIGHLIGHT_KEY) ?? "";
    } catch {
      return "";
    }
  });
  const chooseHighlight = (ns: string) => {
    setHighlight(ns);
    try {
      localStorage.setItem(HIGHLIGHT_KEY, ns);
    } catch {
      /* not remembered this run */
    }
  };
  const layout = useMemo(() => layoutMap(data.data?.nodes ?? [], data.data?.pods ?? []), [data.data]);

  const host = useRef<HTMLDivElement>(null);
  const svg = useRef<SVGSVGElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [view, setView] = useState<View>({ k: 1, x: 0, y: 0 });
  const [fitted, setFitted] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [sel, setSel] = useState<Selection>(null);
  const moved = useRef(false);
  const minK = useRef(0.05);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  const fit = useCallback(() => {
    const v = fitView(layout, size.w, size.h);
    minK.current = Math.max(0.01, v.k * 0.6);
    setView(v);
  }, [layout, size.w, size.h]);

  // Fit once when the first data arrives; later refreshes keep the user's pan and zoom.
  useEffect(() => {
    if (!fitted && layout.boxes.length && size.w > 0) {
      fit();
      setFitted(true);
    }
  }, [fitted, layout, size.w, fit]);

  // Wheel zoom needs a non-passive listener to stop the page scrolling.
  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      const factor = Math.exp(-e.deltaY * 0.0015);
      setView((v) => zoomAt(v, factor, e.clientX - r.left, e.clientY - r.top, minK.current, MAX_K));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [layout.boxes.length]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const sx = e.clientX, sy = e.clientY;
    let lx = sx, ly = sy;
    moved.current = false;
    const move = (m: PointerEvent) => {
      if (!moved.current && Math.hypot(m.clientX - sx, m.clientY - sy) < 4) return;
      moved.current = true;
      setDragging(true);
      const dx = m.clientX - lx, dy = m.clientY - ly;
      lx = m.clientX;
      ly = m.clientY;
      setView((v) => ({ ...v, x: v.x + dx, y: v.y + dy }));
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      setDragging(false);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const onClick = (e: React.MouseEvent) => {
    if (moved.current) return;
    const t = (e.target as Element).closest<SVGElement>("[data-sel]");
    const [type, ...rest] = (t?.dataset.sel ?? "").split(":");
    setSel(t ? ({ type, key: rest.join(":") } as Selection) : null);
  };

  const zoomBy = (f: number) => setView((v) => zoomAt(v, f, size.w / 2, size.h / 2, minK.current, MAX_K));
  const level = detailLevel(view.k);
  const nodes = data.data?.nodes ?? [];
  const pods = data.data?.pods ?? [];
  const nodeUsage = data.data?.nodeUsage ?? null;
  const podUsage = data.data?.podUsage ?? null;
  const memOf = (b: NodeBox) => (b.node ? nodeMemory(b.node, b.pods.map((c) => c.pod), nodeUsage, podUsage) : null);
  const namespaces = useMemo(() => [...new Set(pods.map((p) => p.namespace))].sort(), [pods]);
  const lit = highlight ? pods.filter((p) => p.namespace === highlight).length : 0;
  const selPod = sel?.type === "pod" ? pods.find((p) => podKey(p) === sel.key) : undefined;
  const selNode = sel?.type === "node" ? nodes.find((n) => n.name === sel.key) : undefined;
  // Only draw what is on screen: big clusters hold thousands of cells.
  const visible = (x: number, y: number, w: number, h: number) =>
    x * view.k + view.x < size.w && (x + w) * view.k + view.x > 0 && y * view.k + view.y < size.h && (y + h) * view.k + view.y > 0;

  return (
    <section className="view">
      <header className="view-head">
        <h1>Cluster map</h1>
        <span className="tag">{kube.context}</span>
        <small className="muted">{nodes.length} nodes · {pods.length} pods{updated ? ` · updated ${updated.toLocaleTimeString()}` : ""}</small>
        <span className="spacer" />
        <label className="check">Highlight{" "}
          <select value={highlight} onChange={(e) => chooseHighlight(e.target.value)}>
            <option value="">No highlight</option>
            {highlight && !namespaces.includes(highlight) && <option value={highlight}>{highlight} (none here)</option>}
            {namespaces.map((n) => <option key={n} value={n}>{n}</option>)}
          </select>
        </label>
        {highlight && <small className="muted">{lit} pod{lit === 1 ? "" : "s"}</small>}
        <label className="check">Auto-refresh{" "}
          <select value={settings.mapRefreshSeconds} onChange={(e) => update({ mapRefreshSeconds: Number(e.target.value) })}>
            {MAP_REFRESH_CHOICES.map((n) => <option key={n} value={n}>{n === 0 ? "Off" : n >= 60 ? `${n / 60} min` : `${n}s`}</option>)}
          </select>
        </label>
        <button onClick={() => void data.reload()}>Refresh</button>
      </header>
      {data.error && <div className="banner">{data.error}</div>}
      <div className="kmap" ref={host}>
        {layout.boxes.length === 0 ? (
          <Empty>{data.loading ? "Loading…" : "No nodes."}</Empty>
        ) : (
          <svg ref={svg} className={dragging ? "dragging" : ""} onPointerDown={onPointerDown} onClick={onClick}>
            <g transform={`translate(${view.x} ${view.y}) scale(${view.k})`}>
              {layout.boxes.map((b) => {
                if (!visible(b.x, b.y, b.w, b.h)) return null;
                const down = b.node && !b.node.ready;
                const selected = sel?.type === "node" && sel.key === b.key;
                const far = level === "nodes";
                return (
                  <g key={b.key || "unscheduled"}>
                    <rect className={`km-node${down ? " down" : ""}${selected ? " sel" : ""}`} x={b.x} y={b.y} width={b.w} height={b.h} rx={14} data-sel={b.node ? `node:${b.key}` : undefined} style={b.node ? { cursor: "pointer" } : undefined} />
                    {!far && b.pods.map((c) => {
                      const x = b.x + c.x, y = b.y + c.y;
                      if (!visible(x, y, CELL, CELL)) return null;
                      return <PodCellView key={podKey(c.pod)} pod={c.pod} x={x} y={y} level={level} selected={sel?.type === "pod" && sel.key === podKey(c.pod)} mem={podMemText(c.pod, podUsage)} dim={!!highlight && c.pod.namespace !== highlight} lit={!!highlight && c.pod.namespace === highlight} />;
                    })}
                  </g>
                );
              })}
            </g>
            {/* Labels and memory bars live in screen space so they stay readable at any zoom. */}
            <g pointerEvents="none">
              {layout.boxes.map((b) => (visible(b.x, b.y, b.w, b.h) ? <NodeLabel key={b.key || "unscheduled"} box={b} view={view} level={level} mem={memOf(b)} /> : null))}
            </g>
          </svg>
        )}
        <div className="kmap-tools">
          <button onClick={() => zoomBy(1.5)} aria-label="Zoom in">＋</button>
          <button onClick={() => zoomBy(1 / 1.5)} aria-label="Zoom out">－</button>
          <button onClick={fit}>Fit</button>
          <span className="pill">{level === "nodes" ? "nodes" : level === "pods" ? "pods" : level === "names" ? "pod names" : "containers"}</span>
        </div>
        <div className="kmap-hint">Scroll to zoom, drag to pan, click a node or pod.</div>
        {(selPod || selNode) && (
          <aside className="kmap-panel">
            <div className="toolbar"><span className="spacer" /><button className="icon" onClick={() => setSel(null)} aria-label="Close">✕</button></div>
            {selPod && <PodPanel kube={kube} pod={selPod} usage={podUsage?.get(podKey(selPod)) ?? null} />}
            {selNode && <NodePanel kube={kube} node={selNode} pods={pods.filter((p) => p.node === selNode.name)} mem={nodeMemory(selNode, pods.filter((p) => p.node === selNode.name), nodeUsage, podUsage)} />}
          </aside>
        )}
      </div>
    </section>
  );
}

/** "123 MiB" when usage is known, else "req 128 MiB" from the pod's requests, else empty. */
function podMemText(pod: KubePod, usage: Map<string, number> | null): string {
  const used = usage?.get(podKey(pod));
  if (used !== undefined) return formatBytes(used);
  return pod.memRequest ? `req ${formatBytes(pod.memRequest)}` : "";
}

function NodeLabel({ box: b, view, level, mem }: { box: NodeBox; view: View; level: ReturnType<typeof detailLevel>; mem: NodeMemory | null }) {
  const { k } = view;
  const bx = b.x * k + view.x, by = b.y * k + view.y, bw = b.w * k, bh = b.h * k;
  const hasMem = !!mem && mem.capacity > 0;
  const shown = hasMem ? (mem.used ?? mem.requested) : 0;
  const fraction = hasMem ? Math.min(1, shown / mem.capacity) : 0;
  const memText = hasMem ? `${mem.used === null ? "req " : ""}${formatBytes(shown)} / ${formatBytes(mem.capacity)}` : "";
  const count = `${b.pods.length} pod${b.pods.length === 1 ? "" : "s"}`;
  const flags = `${b.node && !b.node.ready ? " · NotReady" : ""}${b.node?.unschedulable ? " · cordoned" : ""}`;
  const detail = `${count}${flags}${memText ? ` · ${memText}` : ""}`;
  const far = level === "nodes";
  const inside = !far && HEAD * k >= 40;
  const barH = Math.min(8, Math.max(3, bh * 0.04));

  let labels: React.ReactNode;
  if (far) {
    const name = fitLabel(b.label, bw - 8, 15, 9);
    const sub = fitLabel(detail, bw - 8, 11, 8);
    labels = (
      <>
        <text className="km-label" x={bx + bw / 2} y={by + bh / 2 - 2} fontSize={name.px} fontWeight={600} textAnchor="middle">{name.text}</text>
        <text className="km-label sub" x={bx + bw / 2} y={by + bh / 2 + sub.px + 2} fontSize={sub.px} textAnchor="middle">{sub.text}</text>
      </>
    );
  } else if (inside) {
    const name = fitLabel(b.label, bw - 24, 15, 9);
    const sub = fitLabel(detail, bw - 24, 12, 8);
    labels = (
      <>
        <text className="km-label" x={bx + 12} y={by + 18} fontSize={name.px} fontWeight={600}>{name.text}</text>
        <text className="km-label sub" x={bx + 12} y={by + 18 + sub.px + 4} fontSize={sub.px}>{sub.text}</text>
      </>
    );
  } else {
    // Too little room above the pods: put the name on the line above the box, over the gap.
    const both = fitLabel(`${b.label} · ${detail}`, bw + NODE_GAP * k - 4, 13, 9);
    labels = <text className="km-label halo" x={bx + 2} y={by - 5} fontSize={both.px} fontWeight={600}>{both.text}</text>;
  }
  return (
    <g>
      {labels}
      {hasMem && bh >= 24 && (
        <g>
          <rect x={bx + 8} y={by + bh - barH - 6} width={Math.max(0, bw - 16)} height={barH} rx={barH / 2} className="km-bar-bg" />
          <rect x={bx + 8} y={by + bh - barH - 6} width={Math.max(0, (bw - 16) * fraction)} height={barH} rx={barH / 2} fill={loadColor(fraction)} />
          {mem.used !== null && mem.requested > 0 && (
            <rect x={bx + 8 + (bw - 16) * Math.min(1, mem.requested / mem.capacity) - 1} y={by + bh - barH - 8} width={2} height={barH + 4} fill="#fff" />
          )}
        </g>
      )}
    </g>
  );
}

function PodCellView({ pod, x, y, level, selected, dim, lit, mem }: { mem: string; pod: KubePod; x: number; y: number; level: ReturnType<typeof detailLevel>; selected: boolean; dim: boolean; lit: boolean }) {
  const state = podState(pod.status);
  const lines = level === "pods" ? [] : wrapName(pod.name);
  return (
    <g opacity={dim ? 0.15 : 1}>
      <title>{`${pod.namespace}/${pod.name} · ${pod.status}`}</title>
      <rect className={`km-pod ${state}${selected ? " sel" : ""}${lit ? " lit" : ""}`} x={x} y={y} width={CELL} height={CELL} rx={6} data-sel={`pod:${podKey(pod)}`} />
      {lines.map((l, i) => <text key={i} className="km-podtext" x={x + 4} y={y + 11 + i * 8} fontSize={7}>{l}</text>)}
      {level !== "pods" && mem && <text className="km-podtext" x={x + 4} y={y + CELL - 14} fontSize={6.5}>{mem}</text>}
      {level !== "pods" && (
        <text className="km-podtext" x={x + 4} y={y + CELL - 5} fontSize={6.5}>
          {pod.ready}{pod.restarts ? ` ↻${pod.restarts}` : ""}
        </text>
      )}
      {level === "containers" && pod.containers.map((c, i) => (
        <rect key={c.name} x={x + 4 + i * 9} y={y + CELL - 25} width={7} height={7} rx={1.5} fill={c.ready ? "#9ff0c0" : "#ffb3b3"}><title>{`${c.name}: ${c.state}`}</title></rect>
      ))}
    </g>
  );
}

function PodPanel({ kube, pod, usage }: { kube: KubeSel; pod: KubePod; usage: number | null }) {
  const [describe, setDescribe] = useState(false);
  return (
    <>
      <h2>{pod.name}</h2>
      <dl className="kv">
        {([["Namespace", pod.namespace], ["Status", pod.status], ["Ready", pod.ready], ["Restarts", String(pod.restarts)], ["Node", pod.node || "-"], ["IP", pod.ip || "-"], ["Age", pod.age], ["Memory in use", usage === null ? "no metrics" : formatBytes(usage)], ["Memory request", pod.memRequest ? formatBytes(pod.memRequest) : "none"], ["Memory limit", pod.memLimit ? formatBytes(pod.memLimit) : "none"]] as const).map(([k, v]) => (
          <div key={k}><dt>{k}</dt><dd>{v}</dd></div>
        ))}
      </dl>
      <table className="mini">
        <thead><tr><th>Container</th><th>State</th><th className="num">Restarts</th></tr></thead>
        <tbody>
          {pod.containers.map((c) => <tr key={c.name}><td>{c.name}</td><td>{c.state}</td><td className="num">{c.restarts}</td></tr>)}
        </tbody>
      </table>
      <button onClick={() => setDescribe((d) => !d)}>{describe ? "Hide describe" : "Describe"}</button>
      {describe && <TextPanel key={podKey(pod)} load={() => api.kubeShow(kube.context, "describe", "pods", pod.name, pod.namespace)} />}
    </>
  );
}

function NodePanel({ kube, node, pods, mem }: { kube: KubeSel; node: KubeNode; pods: KubePod[]; mem: NodeMemory }) {
  const [describe, setDescribe] = useState(false);
  const bad = pods.filter((p) => podState(p.status) === "exited").length;
  return (
    <>
      <h2>{node.name}</h2>
      <dl className="kv">
        {([["Status", node.status], ["Roles", node.roles], ["Version", node.version], ["Internal IP", node.ip], ["OS", node.os], ["CPU", node.cpu], ["Memory capacity", node.memory], ["Memory allocatable", node.allocBytes ? formatBytes(node.allocBytes) : "-"], ["Requested by pods", `${formatBytes(mem.requested)}${node.allocBytes ? ` (${Math.round((mem.requested / node.allocBytes) * 100)}% of allocatable)` : ""}`], ["In use", mem.used === null ? "no metrics" : `${formatBytes(mem.used)}${mem.capacity ? ` (${Math.round((mem.used / mem.capacity) * 100)}%)` : ""}`], ["Pods", `${pods.length} of ${node.pods}`], ["Unhealthy pods", String(bad)], ["Age", node.age]] as const).map(([k, v]) => (
          <div key={k}><dt>{k}</dt><dd>{v}</dd></div>
        ))}
      </dl>
      <button onClick={() => setDescribe((d) => !d)}>{describe ? "Hide describe" : "Describe"}</button>
      {describe && <TextPanel key={node.name} load={() => api.kubeShow(kube.context, "describe", "nodes", node.name, "")} />}
    </>
  );
}
