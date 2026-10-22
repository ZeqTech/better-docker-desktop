# Better Docker Desktop — TODO

Scope: Docker is **local only**; Kubernetes goes through `kubectl` and any context in the kubeconfig (Docker Engine on this machine through the `docker` CLI; the only network use is pulling an image on request). Sections: Containers, Images, Volumes, Builds, plus Settings, Clean up and Activity.

## Open questions (answer these and I will pick the items up)

- [ ] **Networks section** (list, inspect, create, remove)? It would be a fifth sidebar section, and you originally asked for four.
- [ ] **Overview/dashboard page** with running counts, disk usage breakdown and recent activity as a landing page, or are the Containers page cards enough?
- [ ] **Guard against deleting "production" things**: how should the app know what is production (a container label, a name pattern, a project)?

## Not started (needs a decision or is large)

- [ ] Playwright UI tests and WebdriverIO desktop tests
- [ ] Mock-based tests of the Docker CLI wrapper (the opt-in live tests cover the real engine)
- [ ] Test disconnect/reconnect behaviour automatically (works manually by design)
- [ ] Test the portable `.exe` and the installer on a clean machine
- [ ] Test against several Docker Engine versions
- [ ] Windows ARM64 build
- [ ] Resource dependency graph (containers, networks, volumes)
- [ ] Docker system information page (`docker info`)

## Dropped (remote or out of scope for a local-only app)

- Docker context switching and remote engines, secure remote connections, remote credentials, remote host dashboard, local-vs-remote labelling
- Image update checks, vulnerability scanning and SBOM (`docker scout`): they query registries
- GitHub Actions CI (needs the remote)
- Tailwind/shadcn, SQLite, bollard, shared Docker integration library: the CLI wrapper does the job
- Light theme: the app is intentionally dark red

## Done

- Kubernetes (via kubectl, `--context=` on every call so the kubeconfig is never changed): context and namespace pickers, Nodes (cordon, pods, describe, YAML), Pods (logs, shell, containers, describe, YAML, delete), Workloads (restart, scale), Services, Events; production-looking contexts get a louder confirmation

- Setup: Tauri 2 + React + TypeScript + Vite, Rust backend, portable build and installer, burgundy icon, accent colours, compact tables
- Engine: detect, version and status, auto-start Docker Desktop, reconnect, timeouts for a stuck engine, custom Docker CLI path, `daemon.json` editor
- Containers: list, search, sort, filter, group by Compose project (start, stop, restart, take down), start/stop/restart/pause/kill/delete, bulk actions, port links, run from an image with ports, env, volumes, restart policy and command, saved presets
- Container detail: live colour-coded logs that follow restarts (wrap and timestamp toggles), Details with health checks, Environment, Mounts, Files (browse, upload, download), Terminal, live Stats charts, Inspect
- Images: list, in-use state, pull with progress, run, tag, export and import `.tar`, layers, inspect, delete, clean up
- Volumes: list with sizes, create, inspect, delete, used-by list, read-only file browser and preview, clean up
- Builds: history, cache, new build with streamed log
- Clean up assistant, Activity log from Docker events, instant list updates from events, crash notifications, window position memory, keyboard shortcuts
- Safety: confirmations, argv-only Docker calls with validated inputs, security notes in the README
- Tests: frontend unit tests, Rust unit tests, opt-in live tests against the local engine
