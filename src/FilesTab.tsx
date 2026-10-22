import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "./api";
import { formatSize } from "./docker";
import { breadcrumbs, joinPath, parseLs, type FileEntry } from "./files";
import { Empty, useApp } from "./ui";

/** Browse a running container's filesystem and copy files in and out with `docker cp`. */
export function FilesTab({ container, running }: { container: string; running: boolean }) {
  const app = useApp();
  const [path, setPath] = useState("/");
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setEntries(parseLs(await api.containerLs(container, path)));
      setError("");
    } catch (e) {
      setEntries([]);
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }, [container, path]);

  useEffect(() => {
    if (running) void load();
  }, [running, load]);

  if (!running) return <Empty>Start the container to browse its files.</Empty>;

  const download = async (e: FileEntry) => {
    const dest = await open({ directory: true, multiple: false, title: `Save ${e.name} to…` });
    if (typeof dest !== "string") return;
    await app.act(`Copy ${e.name} to your computer`, () => api.copyOut(container, joinPath(path, e.name), dest));
  };

  const upload = async () => {
    const picked = await open({ multiple: false, title: `Upload into ${path}` });
    if (typeof picked !== "string") return;
    await app.act("Upload file", () => api.copyIn(container, picked, path), () => void load());
  };

  const uploadFolder = async () => {
    const picked = await open({ directory: true, multiple: false, title: `Upload folder into ${path}` });
    if (typeof picked !== "string") return;
    await app.act("Upload folder", () => api.copyIn(container, picked, path), () => void load());
  };

  return (
    <div className="files">
      <div className="toolbar">
        <nav className="crumbs" aria-label="Path">
          {breadcrumbs(path).map(([label, full], i, all) => (
            <span key={full}>
              <button className="link" disabled={i === all.length - 1} onClick={() => setPath(full)}>{label}</button>
              {i > 0 && i < all.length - 1 && <span className="muted">/</span>}
            </span>
          ))}
        </nav>
        <span className="spacer" />
        <button onClick={() => void upload()}>Upload file…</button>
        <button onClick={() => void uploadFolder()}>Upload folder…</button>
        <button onClick={() => void load()}>Refresh</button>
      </div>
      {error && <div className="banner">{error}</div>}
      <div className="table-wrap">
        <table>
          <thead>
            <tr><th>Name</th><th>Permissions</th><th>Owner</th><th className="num">Size</th><th>Modified</th><th>Actions</th></tr>
          </thead>
          <tbody>
            {entries.map((e) => (
              <tr key={e.name}>
                <td>
                  <span className="file-icon" aria-hidden>{e.kind === "dir" ? "📁" : e.kind === "link" ? "↪" : "📄"}</span>
                  {e.kind === "dir" ? (
                    <button className="link" onClick={() => setPath(joinPath(path, e.name))}>{e.name}</button>
                  ) : (
                    <span>{e.name}</span>
                  )}
                  {e.target && <small>→ {e.target}</small>}
                </td>
                <td className="mono">{e.mode}</td>
                <td className="muted">{e.owner}:{e.group}</td>
                <td className="num">{e.kind === "dir" ? "" : formatSize(e.size)}</td>
                <td className="muted">{e.modified}</td>
                <td className="actions">
                  {e.kind !== "other" && <button className="icon" title="Copy to your computer" onClick={() => void download(e)}>⭳</button>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {!error && entries.length === 0 && <Empty>{loading ? "Loading…" : "This folder is empty."}</Empty>}
      </div>
    </div>
  );
}
