import { useEffect, useMemo, useState } from "react";
import { api } from "./api";
import {
  actionsFor, groupByProject, hostPorts, matches, sortRows, toContainers,
  type Container, type ContainerAction,
} from "./docker";
import { DetailsTab, EnvTab, MountsTab } from "./ContainerTabs";
import { InspectView } from "./InspectView";
import { FilesTab } from "./FilesTab";
import { StatsTab } from "./StatsTab";
import { TerminalTab } from "./TerminalTab";
import { parseInspect } from "./inspect";
import { StreamPanel } from "./StreamPanel";
import { Empty, SearchBox, StatusDot, Summary, Th, useApp, useData, useSort } from "./ui";
import { openUrl } from "./open";
import { useSettings } from "./SettingsContext";
import { pollMs } from "./settings";
import type { Ticks } from "./events";

const LABELS: Record<ContainerAction, string> = {
  start: "Start", stop: "Stop", restart: "Restart", pause: "Pause", unpause: "Resume", kill: "Kill", remove: "Delete",
};

export function ContainersView({ refreshKey, ticks }: { refreshKey: number; ticks: Ticks }) {
  const app = useApp();
  const { settings } = useSettings();
  const list = useData(api.containers, pollMs(settings), ticks.containers);
  const stats = useData(api.stats, pollMs(settings, 2));
  const [query, setQuery] = useState("");
  const [onlyRunning, setOnlyRunning] = useState(settings.onlyRunning);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [detail, setDetail] = useState<Container | null>(null);
  const { sort, dir, onSort } = useSort<Container>("name");

  const containers = useMemo(
    () => toContainers(list.data ?? [], stats.data ?? []),
    // refreshKey forces a recompute after global refreshes
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [list.data, stats.data, refreshKey],
  );
  const shown = useMemo(
    () =>
      sortRows(
        containers.filter((c) => (!onlyRunning || c.state === "running") && matches(query, c.name, c.image, c.id, c.project)),
        sort,
        dir,
      ),
    [containers, query, onlyRunning, sort, dir],
  );
  const groups = useMemo(() => groupByProject(shown), [shown]);

  const running = containers.filter((c) => c.state === "running");
  const cpu = running.reduce((n, c) => n + (c.cpu ?? 0), 0);
  const reload = () => {
    void list.reload();
    void stats.reload();
  };

  const run = async (c: Container, action: ContainerAction) => {
    if (action === "remove" || action === "kill") {
      const ok = await app.confirm({
        title: `${LABELS[action]} container?`,
        body: <>Container <b>{c.name}</b> will be {action === "kill" ? "killed" : "deleted"}.{action === "remove" && " Its writable layer is lost; named volumes are kept."}</>,
        label: LABELS[action],
        danger: true,
      });
      if (!ok) return;
    }
    await app.act(`${LABELS[action]} ${c.name}`, () => api.containerAction(c.id, action), reload);
  };

  const bulk = async (action: "start" | "stop" | "remove") => {
    const targets = containers.filter((c) => selected.has(c.id));
    if (action === "remove") {
      const ok = await app.confirm({
        title: `Delete ${targets.length} container${targets.length === 1 ? "" : "s"}?`,
        body: <>This removes {targets.map((c) => c.name).join(", ")}.</>,
        label: "Delete",
        danger: true,
      });
      if (!ok) return;
    }
    await app.act(
      `${LABELS[action]} ${targets.length} container${targets.length === 1 ? "" : "s"}`,
      () => Promise.all(targets.map((c) => api.containerAction(c.id, action))),
      () => {
        setSelected(new Set());
        reload();
      },
    );
  };

  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (!n.delete(id)) n.add(id);
      return n;
    });
  const allSelected = shown.length > 0 && shown.every((c) => selected.has(c.id));

  if (detail) {
    const live = containers.find((c) => c.id === detail.id) ?? detail;
    return <ContainerDetail container={live} onBack={() => setDetail(null)} onAction={run} />;
  }

  const row = (c: Container, indent = false) => (
    <tr key={c.id} className={selected.has(c.id) ? "sel" : ""}>
      <td className="chk"><input type="checkbox" checked={selected.has(c.id)} onChange={() => toggle(c.id)} aria-label={`Select ${c.name}`} /></td>
      <td className={indent ? "indent" : ""}>
        <div className="namecell">
          <StatusDot state={c.state} />
          <div>
            <button className="link" onClick={() => setDetail(c)}>{c.name}</button>
            <small className="mono">{c.id.slice(0, 12)}</small>
          </div>
        </div>
      </td>
      <td>{c.image.length > 40 ? `${c.image.slice(0, 37)}…` : c.image}</td>
      <td>{c.status}</td>
      <td>
        {hostPorts(c.ports).length === 0 ? <span className="muted">-</span> : hostPorts(c.ports).map((p) => (
          <button key={p} className="link port" onClick={() => void openUrl(`http://localhost:${p}`)} title={`Open localhost:${p}`}>{p}</button>
        ))}
      </td>
      <td className="num">{c.cpu === null ? "-" : `${c.cpu.toFixed(1)}%`}</td>
      <td className="muted">{c.created}</td>
      <td className="actions">
        {c.state === "running" ? (
          <button className="icon" title="Stop" onClick={() => void run(c, "stop")}>■</button>
        ) : (
          <button className="icon" title="Start" onClick={() => void run(c, c.state === "paused" ? "unpause" : "start")}>▶</button>
        )}
        <button className="icon" title="Restart" disabled={!actionsFor(c.state).includes("restart")} onClick={() => void run(c, "restart")}>↻</button>
        <button className="icon danger" title="Delete" onClick={() => void run(c, "remove")}>🗑</button>
      </td>
    </tr>
  );

  return (
    <section className="view">
      <header className="view-head">
        <h1>Containers</h1>
        <span className="spacer" />
        <button onClick={reload}>Refresh</button>
      </header>
      <Summary
        items={[
          { label: "Container CPU usage", value: `${cpu.toFixed(2)}%`, sub: `${running.length} running` },
          { label: "Containers", value: String(containers.length), sub: `${containers.length - running.length} stopped` },
          { label: "Projects", value: String(new Set(containers.map((c) => c.project).filter(Boolean)).size), sub: "Compose" },
        ]}
      />
      <div className="toolbar">
        <SearchBox value={query} onChange={setQuery} />
        <label className="check"><input type="checkbox" checked={onlyRunning} onChange={(e) => setOnlyRunning(e.target.checked)} /> Only show running containers</label>
        <span className="spacer" />
        {selected.size > 0 && (
          <>
            <span className="muted">{selected.size} selected</span>
            <button onClick={() => void bulk("start")}>Start</button>
            <button onClick={() => void bulk("stop")}>Stop</button>
            <button className="danger" onClick={() => void bulk("remove")}>Delete</button>
          </>
        )}
        <button
          className="danger"
          onClick={async () => {
            const ok = await app.confirm({ title: "Delete all stopped containers?", label: "Delete", danger: true });
            if (ok) await app.act("Remove stopped containers", () => api.prune("containers"), reload);
          }}
        >
          Clean up
        </button>
      </div>
      {list.error && <div className="banner">{list.error}</div>}
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th className="chk"><input type="checkbox" checked={allSelected} onChange={() => setSelected(allSelected ? new Set() : new Set(shown.map((c) => c.id)))} aria-label="Select all" /></th>
              <Th<Container> label="Name" k="name" sort={sort} dir={dir} onSort={onSort} />
              <Th<Container> label="Image" k="image" sort={sort} dir={dir} onSort={onSort} />
              <Th<Container> label="Status" k="state" sort={sort} dir={dir} onSort={onSort} />
              <th>Port(s)</th>
              <Th<Container> label="CPU" k="cpu" sort={sort} dir={dir} onSort={onSort} num />
              <th>Created</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {groups.map(([project, items]) =>
              project === "" ? items.map((c) => row(c)) : (
                <GroupRows
                  key={project}
                  project={project}
                  items={items}
                  collapsed={collapsed.has(project)}
                  onToggle={() => setCollapsed((s) => { const n = new Set(s); if (!n.delete(project)) n.add(project); return n; })}
                  onAction={async (a) => {
                    if (a === "down") {
                      const ok = await app.confirm({
                        title: `Take down ${project}?`,
                        body: <>Removes all {items.length} container{items.length === 1 ? "" : "s"} of <b>{project}</b> and its networks. Named volumes are kept.</>,
                        label: "Take down",
                        danger: true,
                      });
                      if (!ok) return;
                    }
                    await app.act(`${a === "down" ? "Take down" : a[0].toUpperCase() + a.slice(1)} ${project}`, () => api.projectAction(project, a), reload);
                  }}
                  render={(c) => row(c, true)}
                />
              ),
            )}
          </tbody>
        </table>
        {shown.length === 0 && <Empty>{list.loading ? "Loading…" : containers.length ? "No containers match your filters." : "No containers yet. Run an image from the Images tab."}</Empty>}
      </div>
    </section>
  );
}

function GroupRows({ project, items, collapsed, onToggle, onAction, render }: {
  project: string;
  items: Container[];
  collapsed: boolean;
  onToggle(): void;
  onAction(a: "start" | "stop" | "restart" | "down"): void;
  render(c: Container): React.ReactNode;
}) {
  const up = items.filter((c) => c.state === "running").length;
  return (
    <>
      <tr className="group">
        <td />
        <td colSpan={6}>
          <button className="link" onClick={onToggle}>{collapsed ? "▸" : "▾"} {project}</button>
          <small>{up}/{items.length} running</small>
        </td>
        <td className="actions">
          <button className="icon" title="Start the project" onClick={() => onAction("start")}>▶</button>
          <button className="icon" title="Stop the project" onClick={() => onAction("stop")}>■</button>
          <button className="icon" title="Restart the project" onClick={() => onAction("restart")}>↻</button>
          <button className="icon danger" title="Take the project down (removes its containers)" onClick={() => onAction("down")}>🗑</button>
        </td>
      </tr>
      {!collapsed && items.map(render)}
    </>
  );
}

const TAB_LABEL = { logs: "Logs", details: "Details", env: "Environment", mounts: "Mounts", files: "Files", terminal: "Terminal", stats: "Stats", inspect: "Inspect" } as const;
const TABS = ["logs", "details", "env", "mounts", "files", "terminal", "stats", "inspect"] as const;
type Tab = (typeof TABS)[number];

function ContainerDetail({ container: c, onBack, onAction }: {
  container: Container;
  onBack(): void;
  onAction(c: Container, a: ContainerAction): Promise<void>;
}) {
  const [tab, setTab] = useState<Tab>("logs");
  // Once opened, the terminal stays mounted (hidden) so switching tabs does not end the session.
  const [terminalOpened, setTerminalOpened] = useState(false);
  const running = c.state === "running";
  const insp = useData(() => api.inspect("container", c.id));
  const details = useMemo(() => (insp.data ? parseInspect(insp.data) : null), [insp.data]);
  // Re-read after the container starts, stops or restarts.
  useEffect(() => {
    void insp.reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [c.state, c.status]);
  return (
    <section className="view">
      <header className="view-head">
        <button onClick={onBack}>← Containers</button>
        <StatusDot state={c.state} />
        <h1>{c.name}</h1>
        <small className="mono">{c.id.slice(0, 12)}</small>
        <span className="spacer" />
        {actionsFor(c.state).map((a) => (
          <button key={a} className={a === "remove" || a === "kill" ? "danger" : ""} onClick={async () => { await onAction(c, a); if (a === "remove") onBack(); }}>
            {LABELS[a]}
          </button>
        ))}
      </header>
      <div className="meta">
        <span>Image <b>{c.image}</b></span>
        <span>Status <b>{c.status}</b></span>
        {c.project && <span>Project <b>{c.project}</b></span>}
        {c.ports && <span>Ports <b>{c.ports}</b></span>}
      </div>
      <div className="tabs">
        {TABS.map((t) => (
          <button
            key={t}
            className={tab === t ? "active" : ""}
            onClick={() => {
              setTab(t);
              if (t === "terminal") setTerminalOpened(true);
            }}
          >
            {TAB_LABEL[t]}
          </button>
        ))}
      </div>
      <div className="tab-body">
        {tab === "logs" && <StreamPanel kind="logs" target={c.id} reattach active={c.state === "running"} />}
        {tab === "details" && (details ? <DetailsTab d={details} /> : <Empty>{insp.error ?? "Loading…"}</Empty>)}
        {tab === "env" && (details ? <EnvTab d={details} /> : <Empty>{insp.error ?? "Loading…"}</Empty>)}
        {tab === "mounts" && (details ? <MountsTab d={details} /> : <Empty>{insp.error ?? "Loading…"}</Empty>)}
        {terminalOpened && running && (
          <div className="term-wrap" style={{ display: tab === "terminal" ? "flex" : "none" }}>
            <TerminalTab container={c.id} />
          </div>
        )}
        {tab === "terminal" && !running && <Empty>Start the container to open a terminal.</Empty>}
        {tab === "files" && <FilesTab container={c.id} running={running} />}
        {tab === "inspect" && <InspectView kind="container" target={c.id} />}
        {tab === "stats" && <StatsTab container={c.id} running={running} />}
      </div>
    </section>
  );
}
