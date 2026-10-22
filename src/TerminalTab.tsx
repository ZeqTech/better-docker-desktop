import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

/** An interactive shell inside a running container, backed by a pseudo-terminal on the Rust side. */
export function TerminalTab({ container }: { container: string }) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = `shell:${container}`;
    const term = new Terminal({
      fontSize: 13,
      fontFamily: 'Consolas, "Cascadia Mono", monospace',
      cursorBlink: true,
      theme: {
        background: "#2b2325", foreground: "#e3dcdd", cursor: "#ff6b6b", selectionBackground: "rgba(214, 48, 79, 0.45)",
        black: "#2b2325", red: "#ff7b7b", green: "#7ee2a4", yellow: "#f0c15a", blue: "#7fb4ff", magenta: "#e59bff", cyan: "#6fe0e6", white: "#e3dcdd",
        brightBlack: "#8a7478", brightRed: "#ff9a9a", brightGreen: "#9ff0c0", brightYellow: "#f6dc9c", brightBlue: "#a3c9ff", brightMagenta: "#efb8ff", brightCyan: "#9aeef2", brightWhite: "#ffffff",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host.current!);
    try {
      fit.fit();
    } catch {
      /* hidden while measuring; the observer fits it once visible */
    }

    let alive = true;
    const unlisten = [
      listen<{ id: string; data: string }>("shell-out", (e) => e.payload.id === id && term.write(e.payload.data)),
      listen<string>("shell-end", (e) => {
        if (e.payload === id) term.write("\r\n\x1b[90m[session ended]\x1b[0m\r\n");
      }),
    ];
    Promise.all(unlisten).then(() => {
      if (!alive) return;
      invoke("start_shell", { id, container, cols: term.cols, rows: term.rows }).catch((e) => term.write(`\r\n${String(e)}\r\n`));
    });

    const input = term.onData((data) => void invoke("shell_write", { id, data }).catch(() => {}));
    const observer = new ResizeObserver(() => {
      if (!host.current || host.current.offsetWidth === 0) return; // tab is hidden
      try {
        fit.fit();
        void invoke("shell_resize", { id, cols: term.cols, rows: term.rows }).catch(() => {});
      } catch {
        /* ignore */
      }
    });
    observer.observe(host.current!);

    return () => {
      alive = false;
      observer.disconnect();
      input.dispose();
      unlisten.forEach((u) => void u.then((un) => un()));
      void invoke("stop_shell", { id });
      term.dispose();
    };
  }, [container]);

  return <div ref={host} className="term-host" />;
}
