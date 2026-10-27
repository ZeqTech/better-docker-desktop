# Better Docker Desktop

A lean Docker Desktop alternative in Tauri v2 with a selectable ruby (default), emerald or sapphire dark theme, each with its own accents. It is **local only**: it talks to the Docker Engine on this machine through the `docker` command line and has no remote features. The one network use is pulling an image when you ask for it.

Layout follows Docker Desktop: sidebar, summary cards, sortable tables, status bar.

## Download

Get the latest build from the [Releases](../../releases) page. You need Docker installed on your machine.

- **Windows:** `BetterDockerDesktop-vX.Y.Z-win-x64-setup.exe` installs the app; `...-portable.exe` runs without installing.
- **macOS:** `BetterDockerDesktop-vX.Y.Z-macos-universal.dmg` runs on Apple Silicon and Intel. Open it and drag the app to Applications.

The builds are not code-signed, so the first launch shows a warning:

- Windows SmartScreen: choose **More info**, then **Run anyway**.
- macOS Gatekeeper: right-click the app and choose **Open**, or run `xattr -dr com.apple.quarantine "/Applications/Better Docker Desktop.app"`.

Check a download against `SHA256SUMS.txt` from the same release (`Get-FileHash` on Windows, `shasum -a 256` on macOS).

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
  - A read-only **Files** tab to browse a volume and preview text files. It reads through a throwaway `alpine` or `busybox` container with no network and a read-only mount. If neither image is local the tab offers to pull `alpine:3` (about 8 MB).
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
- The window runs under a strict content security policy (no inline scripts, no remote connections), so even injected content could not reach the backend commands or the network.
- Saved run presets never keep secret values: environment variables that look like secrets (by name, or a URL with a password in it) are saved empty and entered again each time.
- A custom Docker CLI path must be the real `docker.exe`; scripts such as `docker.bat` are refused.
- The helper image for volume browsing is pinned by digest, so the tag cannot be swapped for different content. Update the digest in `src/VolumeFiles.tsx` to move to a newer Alpine.
- The Run dialog asks before starting a container that mounts the Docker socket, a whole drive or a writable system folder.
- Not covered: the installer is not code-signed (Windows SmartScreen will warn), and dependencies are not audited automatically. Run `pnpm audit` and `cargo audit` now and then.

## Development

Needs Node, pnpm, Rust (pinned in `rust-toolchain.toml`) and a running Docker Engine.

```bash
pnpm install
pnpm tauri dev                 # run the app
pnpm test                      # tsc + vitest
cd src-tauri && cargo test     # Rust unit tests
./make.sh win                  # installer + portable exe into dist/
./make.sh mac                  # universal .dmg into dist/ (must run on a Mac)
MAC_TARGET=aarch64-apple-darwin ./make.sh mac   # one architecture, about twice as fast
```

## Releasing

Commit your changes, then:

```bash
./make.sh release patch        # or minor, major, or an exact version like 0.3.0
```

It shows the plan and asks first (`-y` skips the question, `-n` only shows the plan). It then bumps the version in `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `Cargo.lock`, commits "Release vX.Y.Z", tags it and pushes the branch and tag together. It stops if the working tree is dirty, the branch is behind the remote, or the tag already exists.

The tag starts `.github/workflows/release.yml`, which builds Windows and macOS on GitHub's runners (running the tests first), then publishes the installers and a `SHA256SUMS.txt` as a GitHub Release. The workflow refuses to run if the tag does not match the version in `tauri.conf.json`. It can also be started by hand from the Actions tab for an existing tag.
