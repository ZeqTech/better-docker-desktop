# Better Docker Desktop

A lean Docker Desktop alternative in Tauri v2 with a dark red / burgundy theme. It is **local only**: it talks to the Docker Engine on this machine through the `docker` command line and has no remote features. The one network use is pulling an image when you ask for it.

Layout follows Docker Desktop: sidebar, summary cards, sortable tables, status bar.

## Sections

- **Containers**
  - Grouped by Compose project, with start, stop, restart and take-down for a whole project.
  - Start, stop, restart, pause, kill and delete, plus bulk actions and a Delete-key shortcut.
  - Published ports open in your browser.
  - Detail page tabs: **Logs** (live, colour-coded, follows restarts, wrap and timestamp toggles), **Details** (config, health checks, ports, networks, labels), **Environment** (secrets masked), **Mounts**, **Files** (browse, upload, download), **Terminal** (shell in the container), **Stats** (live CPU, memory, network and disk charts) and **Inspect**.
- **Images**
  - In use or unused, with sizes.
  - Pull with live progress.
  - Run with ports, environment variables, volumes, restart policy and command, plus saved presets.
  - Add tag, export to and import from `.tar`, layers view, inspect, delete.
- **Volumes**
  - Sizes and in-use status, create, inspect, delete.
  - A "Used by" list of containers.
- **Builds**
  - Build history (Buildx 0.20+) and build cache.
  - New build from a folder with a Dockerfile, with a streamed log.

Also: a **Clean up** assistant (stopped containers, unused images, unused volumes, build cache), an **Activity** log of Docker events, crash notifications, and **Settings** (start the engine on launch, start page, confirmations, refresh interval, log history, custom Docker CLI path, an editor for `~/.docker/daemon.json`, accent colour, compact tables).

The window updates instantly from `docker events`, with polling as a fallback. It starts Docker Desktop if the engine is down and reconnects when it comes back. Window size and position are remembered.

## Keyboard shortcuts

| Keys | Action |
| --- | --- |
| Ctrl+1 … Ctrl+4 | Containers, Images, Volumes, Builds |
| Ctrl+, | Settings |
| `/` or Ctrl+F | Search the current list |
| Ctrl+R or F5 | Refresh everything |
| Delete | Delete the selected containers |
| Esc | Close a dialog |

## Security notes

- Anything that can reach the Docker Engine effectively has root on the host. This app can start containers with arbitrary volume mounts, run a shell inside any container, and delete volumes. Only run it as a user you trust with that.
- Names, image references, paths, ports, environment variables and volumes are validated in Rust and passed to Docker as separate arguments, never through a shell.
- Destructive actions ask first (switchable in Settings). Unused-volume clean-up is unchecked by default because it deletes data for good.
- Editing the engine config can stop Docker from starting if the JSON is wrong. The previous file is kept as `daemon.json.bak`.

## Development

Needs Node, pnpm, Rust (pinned in `rust-toolchain.toml`) and a running Docker Engine.

```bash
pnpm install
pnpm tauri dev                 # run the app
pnpm test                      # tsc + vitest
cd src-tauri && cargo test     # Rust unit tests
./make.sh win                  # installer + portable exe into dist/
```

Live tests run against your real engine and create disposable `bdd-live-*` containers and volumes, removing them afterwards. They need at least one image already present locally and never pull:

```bash
cd src-tauri && cargo test --test live -- --ignored --test-threads=1
```
