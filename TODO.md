# Better Docker Desktop — TODO

Scope so far: Containers, Images, Volumes, Builds, Settings. The app drives the `docker` CLI (no bollard). The checklist below is the original wish list; `[x]` means done. **Next up** is the working order.

## Next up

### Fix / verify first
- [ ] Run against a healthy engine and try every action (start, stop, restart, pause, delete, pull, run, tag, build, prune, settings)
- [ ] Confirm Docker Desktop reads `~/.docker/daemon.json` on this setup (Settings > Docker Engine editor)
- [ ] Burgundy app icon (installer and taskbar still use the docker-compose-studio icon)
- [ ] Right-align numeric column headers (Size, CPU, Steps)
- [ ] Test log re-attach after Restart/Start/Stop in the container Logs tab (new; only unit-checked)

### High value
- [ ] Interactive terminal in a running container (exec), reusing the PTY approach from docker-compose-studio
- [ ] Cleanup assistant: one screen with reclaimable containers, images, volumes and build cache, total size, single action
- [ ] Docker events stream so lists update instantly instead of polling
- [ ] Live CPU / memory charts in the container detail page (streaming `docker stats`)
- [ ] Richer Run dialog: several ports, environment variables, volumes, restart policy, command

### Next tier
- [ ] Container file browser and copy in/out (`docker cp`)
- [ ] Image layers view (`docker history`) and export/import (`docker save` / `docker load`)
- [ ] Networks section: list, inspect, create, remove
- [ ] Container detail: parsed Env, Mounts, Ports and Health tabs instead of raw inspect only
- [ ] Remember window size/position; keyboard shortcuts (`/` search, `Del` delete selection, `Ctrl+R` refresh)
- [ ] Desktop notification when a container dies or exits with an error
- [ ] Image update check (is a newer tag available?)
- [ ] Volume browsing (read-only) and which containers use each volume
- [ ] Clearer local vs remote engine labelling in the status bar when the context is not local

### Later
- [ ] Saved container configurations
- [ ] Compose project actions (up/down) from the grouped container rows
- [ ] Vulnerability scanning and SBOM viewer integration (`docker scout`)
- [ ] Rust command tests against a disposable engine; Playwright UI tests
- [ ] GitHub Actions CI; Windows ARM64 build

---
# Better Docker Desktop — TODO

## Project Setup
- [x] Initialize Tauri 2 + React + TypeScript + Vite
- [ ] Configure Tailwind CSS and shadcn/ui
- [ ] Set up Rust backend and SQLite
- [ ] Reuse shared Docker integration library
- [x] Configure portable Windows build
- [ ] Add dark/light themes
- [ ] Create dashboard and navigation layout

## Docker Engine Integration
- [x] Detect local Docker Engine
- [ ] Connect using bollard
- [x] Support Docker contexts and remote engines
- [x] Display Docker Engine version and status
- [x] Handle daemon disconnections and reconnection
- [ ] Support secure remote connections
- [ ] Show Docker system information

## Container Management
- [x] List running and stopped containers
- [x] Search, sort, and filter containers
- [x] Start, stop, restart, pause, and remove containers
- [ ] Display CPU, memory, and network usage
- [x] Stream container logs
- [ ] Open interactive container terminal
- [ ] Inspect environment variables and mounts
- [ ] Display container health checks
- [x] Support bulk container operations
- [x] Create containers from images

## Image Management
- [x] List local images
- [x] Pull images from registries
- [x] Remove unused images
- [ ] Inspect image metadata and layers
- [x] Display image disk usage
- [ ] Tag and export images
- [x] Build images from Dockerfiles
- [x] Show build progress and logs

## Volumes and Networks
- [x] List Docker volumes
- [x] Inspect volume metadata
- [ ] Browse volume contents safely
- [x] Create and delete volumes
- [ ] List Docker networks
- [ ] Inspect network connections
- [ ] Create and remove networks
- [ ] Visualize container network relationships

## Dashboard and Monitoring
- [ ] Build overview dashboard
- [ ] Show running/stopped container counts
- [ ] Display live resource usage charts
- [ ] Show disk usage breakdown
- [ ] Display Docker events timeline
- [ ] Add searchable activity history
- [ ] Add desktop notifications

## Advanced Features
- [ ] Container filesystem browser
- [ ] Image layer visualization
- [ ] Image vulnerability scanning integration
- [ ] SBOM viewer
- [ ] Resource dependency graph
- [ ] Docker cleanup assistant
- [ ] Saved container configurations
- [ ] Remote Docker host dashboard
- [ ] Integrate Compose project management

## Security and Reliability
- [ ] Store remote credentials securely
- [ ] Restrict Docker API access to required operations
- [x] Confirm destructive operations
- [ ] Clearly distinguish local and remote engines
- [ ] Prevent accidental production resource deletion
- [ ] Handle large log streams without UI freezing
- [ ] Support cancellation and operation timeouts
- [ ] Document Docker daemon access risks

## Automated Testing
- [ ] Configure Vitest and Playwright
- [ ] Configure Rust unit tests
- [ ] Mock Docker Engine API responses
- [ ] Test against disposable Docker containers
- [ ] Test container lifecycle operations
- [ ] Test image, volume, and network management
- [ ] Test disconnect/reconnect behavior
- [ ] Configure WebdriverIO desktop tests
- [ ] Add GitHub Actions CI

## Release
- [ ] Build Windows x64 and ARM64 where supported
- [ ] Test portable executable behavior
- [x] Build installer
- [ ] Test against supported Docker Engine versions
- [ ] Document existing Docker Engine requirement
- [ ] Add application icons and metadata
- [x] Write README and user documentation
