# Better Docker Desktop

A lean Docker Desktop alternative in Tauri v2, with a dark red / burgundy theme. It drives the `docker` CLI and covers four sections, laid out like Docker Desktop (sidebar, summary cards, sortable tables, status bar):

- **Containers** – grouped by Compose project, CPU usage, start/stop/restart/delete, bulk actions, clickable published ports, and a detail page with live logs, inspect and stats.
- **Images** – in-use/unused, pull with live progress, run (name + port), add tag, inspect, delete, clean up unused.
- **Volumes** – size and in-use status, create, inspect, delete, clean up unused.
- **Builds** – build history (Buildx 0.20+), build cache with clean-up, and a New build dialog that streams the build log.

Destructive actions ask for confirmation (switchable in Settings).

**Settings** (gear in the sidebar): start the engine on launch, start page, confirmation prompts, refresh interval, log history and timestamps, a custom Docker CLI path, Docker context switching, an editor for the engine config (`~/.docker/daemon.json`, with a `.bak` backup), accent colour and compact tables. Names and references are validated in Rust and passed to Docker as argv, never through a shell.

Requires Docker Engine (or Docker Desktop) running locally. The window polls the engine and reconnects when it comes back.

```bash
pnpm install
pnpm tauri dev     # run
pnpm test          # tsc + vitest
cd src-tauri && cargo test
./make.sh win      # installer + portable exe into dist/
```
