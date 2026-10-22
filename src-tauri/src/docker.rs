//! Docker CLI wrapper. Pure std, no Tauri types. Arguments are always passed as an argv
//! list, never through a shell, and every user-supplied name is validated first.

use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Serialize, Default)]
pub struct DockerStatus {
    pub docker_version: Option<String>,
    pub context: Option<String>,
    pub daemon_running: bool,
    pub error: Option<String>,
}

/// Docker CLI chosen in Settings; `None` means "docker" from PATH.
static DOCKER_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Set (or with an empty string, clear) the Docker CLI to use. Must be an existing file named docker.
pub fn set_docker_path(path: &str) -> Result<(), String> {
    let path = path.trim();
    let new = if path.is_empty() {
        None
    } else {
        let p = PathBuf::from(path);
        let stem_ok = p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("docker"));
        if !p.is_file() || !stem_ok {
            return Err(format!("{path} is not a docker executable"));
        }
        Some(p)
    };
    *DOCKER_PATH.lock().unwrap() = new;
    Ok(())
}

/// The docker executable to run: the one chosen in Settings, else `docker` from PATH. On Windows
/// the `.exe` is named explicitly because CreateProcess can resolve a bare name to a non-Win32 shim
/// (that broke the PTY spawn).
pub fn program() -> PathBuf {
    DOCKER_PATH
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "docker.exe" } else { "docker" }))
}

/// argv for an interactive shell in a container: bash when present, else sh.
pub fn shell_args(container: &str) -> Result<Vec<String>, String> {
    let c = name_arg(container)?;
    Ok(["exec", "-it", c, "sh", "-c", "command -v bash >/dev/null 2>&1 && exec bash || exec sh"].map(String::from).to_vec())
}

fn command(args: &[&str]) -> Command {
    let mut cmd = Command::new(program());
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// A wedged engine makes the CLI hang rather than fail, so every call has a deadline.
const TIMEOUT: Duration = Duration::from_secs(90);
const STATUS_TIMEOUT: Duration = Duration::from_secs(8);

fn output_within(args: &[&str], limit: Duration) -> Result<Output, String> {
    let mut child = command(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Docker CLI not found: {e}"))?;
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = drain(child.stdout.take().map(|p| Box::new(p) as _));
    let err = drain(child.stderr.take().map(|p| Box::new(p) as _));
    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break s,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "Docker did not respond within {}s. The engine may be stuck; restart Docker Desktop.",
                    limit.as_secs()
                ));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    Ok(Output { status, stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default() })
}

fn output(args: &[&str]) -> Result<Output, String> {
    output_within(args, TIMEOUT)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

/// Run `docker <args>`; Ok = trimmed stdout, Err = trimmed stderr (or stdout if stderr is empty).
pub fn run(args: &[&str]) -> Result<String, String> {
    let o = output(args)?;
    if o.status.success() {
        Ok(text(&o.stdout))
    } else {
        let err = text(&o.stderr);
        Err(if err.is_empty() { text(&o.stdout) } else { err })
    }
}

pub fn status() -> DockerStatus {
    let mut s = DockerStatus::default();
    if let Ok(o) = output_within(&["context", "show"], STATUS_TIMEOUT) {
        if o.status.success() {
            s.context = Some(text(&o.stdout));
        }
    }
    match output_within(&["version", "--format", "{{.Server.Version}}"], STATUS_TIMEOUT) {
        Ok(o) if o.status.success() => {
            s.docker_version = Some(text(&o.stdout));
            s.daemon_running = true;
        }
        Ok(o) => s.error = Some(text(&o.stderr)),
        Err(e) => s.error = Some(e),
    }
    s
}

/// Launch Docker Desktop (detached) so the engine comes up. Returns once it is spawned, not
/// once the engine is ready; callers poll `status()`.
pub fn start_engine() -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let roots = ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"];
        let exe = roots
            .iter()
            .filter_map(|v| std::env::var_os(v))
            .flat_map(|r| {
                let r = std::path::PathBuf::from(r);
                [r.join(r"Docker\Docker\Docker Desktop.exe"), r.join(r"Programs\Docker\Docker\Docker Desktop.exe")]
            })
            .find(|p| p.is_file())
            .ok_or("Docker Desktop.exe not found. Install Docker Desktop or start your engine manually.")?;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP so it outlives this window.
        Command::new(&exe)
            .creation_flags(0x0000_0008 | 0x0000_0200)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("could not start {}: {e}", exe.display()))?;
        Ok(format!("Starting {}", exe.display()))
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").args(["-a", "Docker"]).spawn().map_err(|e| e.to_string())?;
        Ok("Starting Docker Desktop".to_string())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err("Start the Docker service yourself, e.g. `sudo systemctl start docker`.".to_string())
    }
}

// ---- validation -------------------------------------------------------------------------

/// Container / volume names and short IDs.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with(['-', '.'])
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Image references: `repo/name:tag`, `name@sha256:...`, IDs and build refs.
pub fn valid_ref(r: &str) -> bool {
    !r.is_empty()
        && r.len() <= 512
        && !r.starts_with('-')
        && r.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '@'))
}

/// RFC 3339 timestamp such as `2026-10-21T22:00:00.123Z` (digits and `-:.TZ+` only).
pub fn valid_since(s: &str) -> bool {
    !s.is_empty() && s.len() <= 40 && s.starts_with(|c: char| c.is_ascii_digit()) && s.chars().all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | '.' | 'T' | 'Z' | '+'))
}

/// `host:container` port mapping, e.g. `8080:80` or `5353:53/udp`.
pub fn valid_port_map(p: &str) -> bool {
    let p = p.strip_suffix("/tcp").or_else(|| p.strip_suffix("/udp")).unwrap_or(p);
    let mut it = p.split(':');
    let ok = |s: Option<&str>| s.is_some_and(|s| !s.is_empty() && s.len() <= 5 && s.chars().all(|c| c.is_ascii_digit()));
    ok(it.next()) && ok(it.next()) && it.next().is_none()
}

fn name_arg(name: &str) -> Result<&str, String> {
    if valid_name(name) { Ok(name) } else { Err(format!("invalid name: {name}")) }
}

fn ref_arg(r: &str) -> Result<&str, String> {
    if valid_ref(r) { Ok(r) } else { Err(format!("invalid reference: {r}")) }
}

/// Existing directory, with the `\\?\` prefix `canonicalize` adds on Windows removed.
pub fn validate_dir(path: &str) -> Result<String, String> {
    let canon = Path::new(path).canonicalize().map_err(|e| format!("{path}: {e}"))?;
    if !canon.is_dir() {
        return Err(format!("{path} is not a folder"));
    }
    Ok(canon.to_string_lossy().trim_start_matches(r"\\?\").to_string())
}

// ---- listings (raw JSON lines; parsed in the frontend) ----------------------------------

pub fn list_containers() -> Result<String, String> {
    run(&["ps", "-a", "--no-trunc", "--format", "{{json .}}"])
}

pub fn stats() -> Result<String, String> {
    run(&["stats", "--no-stream", "--format", "{{json .}}"])
}

pub fn list_images() -> Result<String, String> {
    run(&["images", "--no-trunc", "--format", "{{json .}}"])
}

/// `system df -v` as one JSON object: Images, Containers, Volumes (with sizes), BuildCache.
pub fn disk_usage() -> Result<String, String> {
    run(&["system", "df", "-v", "--format", "{{json .}}"])
}

/// Build history; needs buildx 0.20+. An old buildx yields an error the UI shows as a hint.
pub fn build_history() -> Result<String, String> {
    run(&["buildx", "history", "ls", "--format", "json"])
}

// ---- engine configuration (daemon.json) ---------------------------------------------------

#[derive(Serialize)]
pub struct EngineConfig {
    pub path: String,
    pub exists: bool,
    pub content: String,
}

fn daemon_json_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).ok_or("cannot find the home folder")?;
    Ok(PathBuf::from(home).join(".docker").join("daemon.json"))
}

/// Parse and re-format engine config. It must be a JSON object; blank means `{}`.
pub fn normalize_engine_config(text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok("{}\n".to_string());
    }
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    if !v.is_object() {
        return Err("the engine config must be a JSON object".to_string());
    }
    serde_json::to_string_pretty(&v).map(|s| s + "\n").map_err(|e| e.to_string())
}

pub fn read_engine_config() -> Result<EngineConfig, String> {
    let path = daemon_json_path()?;
    let shown = path.to_string_lossy().into_owned();
    match std::fs::metadata(&path) {
        Ok(m) if m.len() > 1_000_000 => Err(format!("{shown} is too large")),
        Ok(_) => Ok(EngineConfig { path: shown, exists: true, content: std::fs::read_to_string(&path).map_err(|e| e.to_string())? }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(EngineConfig { path: shown, exists: false, content: "{}\n".into() }),
        Err(e) => Err(e.to_string()),
    }
}

/// Validate, back up the old file to `daemon.json.bak`, then replace it atomically.
pub fn write_engine_config(text: &str) -> Result<String, String> {
    let normalized = normalize_engine_config(text)?;
    let path = daemon_json_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if path.is_file() {
        std::fs::copy(&path, path.with_extension("json.bak")).map_err(|e| format!("backup failed: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &normalized).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(normalized)
}

// ---- container files ------------------------------------------------------------------------

/// Absolute path inside a container: no control characters, not just a flag-like string.
pub fn valid_container_path(p: &str) -> bool {
    p.starts_with('/') && p.len() <= 4096 && !p.chars().any(|c| c.is_control())
}

fn container_path(p: &str) -> Result<&str, String> {
    if valid_container_path(p) { Ok(p) } else { Err(format!("invalid container path: {p}")) }
}

/// `ls -la` of a directory inside a running container (works with busybox and GNU ls).
pub fn container_ls(container: &str, path: &str) -> Result<String, String> {
    run(&["exec", name_arg(container)?, "ls", "-la", "--", container_path(path)?])
}

fn host_path_exists(p: &str) -> Result<String, String> {
    let canon = Path::new(p).canonicalize().map_err(|e| format!("{p}: {e}"))?;
    Ok(canon.to_string_lossy().trim_start_matches(r"\\?\").to_string())
}

/// Copy `path` out of the container into the existing host folder `dest_dir`.
pub fn container_copy_out(container: &str, path: &str, dest_dir: &str) -> Result<String, String> {
    let src = format!("{}:{}", name_arg(container)?, container_path(path)?);
    let dest = validate_dir(dest_dir)?;
    run(&["cp", &src, &dest]).map(|_| format!("Copied to {dest}"))
}

/// Copy a host file or folder into the container directory `dest_dir`.
pub fn container_copy_in(container: &str, src: &str, dest_dir: &str) -> Result<String, String> {
    let host = host_path_exists(src)?;
    let dest = format!("{}:{}/", name_arg(container)?, container_path(dest_dir)?.trim_end_matches('/'));
    run(&["cp", &host, &dest]).map(|_| format!("Copied into {dest_dir}"))
}

// ---- inspect ------------------------------------------------------------------------------

pub fn inspect(kind: &str, target: &str) -> Result<String, String> {
    let kind = match kind {
        "container" | "image" | "volume" => kind,
        _ => return Err(format!("cannot inspect {kind}")),
    };
    let t = if kind == "volume" { name_arg(target)? } else { ref_arg(target)? };
    run(&[kind, "inspect", t])
}

// ---- actions ------------------------------------------------------------------------------

pub fn container_action(id: &str, action: &str) -> Result<String, String> {
    let id = name_arg(id)?;
    match action {
        "start" | "stop" | "restart" | "pause" | "unpause" | "kill" => run(&[action, id]),
        "remove" => run(&["rm", "-f", id]),
        _ => Err(format!("unknown action: {action}")),
    }
}

pub fn image_remove(image: &str, force: bool) -> Result<String, String> {
    let image = ref_arg(image)?;
    if force { run(&["rmi", "-f", image]) } else { run(&["rmi", image]) }
}

pub fn image_tag(source: &str, target: &str) -> Result<String, String> {
    run(&["tag", ref_arg(source)?, ref_arg(target)?])
}

/// Everything the Run dialog can set. Lists hold one entry per line the user typed.
#[derive(serde::Deserialize, Default, Debug)]
pub struct RunOptions {
    pub image: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub restart: String,
    #[serde(default)]
    pub command: String,
}

/// `KEY=VALUE` with a conventional variable name.
pub fn valid_env(e: &str) -> bool {
    let Some((k, v)) = e.split_once('=') else { return false };
    !k.is_empty()
        && !k.starts_with(|c: char| c.is_ascii_digit())
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !v.contains(['\n', '\r', '\0'])
}

/// `src:dst[:ro|rw]`. `src` is a volume name or a host path (it may contain a Windows drive colon).
pub fn valid_volume(v: &str) -> bool {
    if v.contains(['\n', '\r', '\0']) {
        return false;
    }
    let mut parts: Vec<&str> = v.split(':').collect();
    if matches!(parts.last(), Some(&"ro") | Some(&"rw")) {
        parts.pop();
    }
    let Some(dst) = parts.pop() else { return false };
    let src = parts.join(":");
    dst.starts_with('/') && dst.len() > 1 && !src.is_empty() && !src.starts_with('-')
}

/// Split a command line on whitespace, honouring single and double quotes.
pub fn split_command(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                has = true;
            }
            (None, c) if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if quote.is_some() {
        return Err("unclosed quote in the command".to_string());
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

/// The full argv (after `docker`) for a validated run request.
pub fn run_args(o: &RunOptions) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = vec!["run".into(), "-d".into()];
    ref_arg(&o.image)?;
    if !o.name.trim().is_empty() {
        a.extend(["--name".into(), name_arg(o.name.trim())?.to_string()]);
    }
    for p in o.ports.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        if !valid_port_map(p) {
            return Err(format!("invalid port mapping: {p} (use host:container, e.g. 8080:80)"));
        }
        a.extend(["-p".into(), p.into()]);
    }
    for e in o.env.iter().map(|e| e.trim()).filter(|e| !e.is_empty()) {
        if !valid_env(e) {
            return Err(format!("invalid environment variable: {e} (use KEY=VALUE)"));
        }
        a.extend(["-e".into(), e.into()]);
    }
    for v in o.volumes.iter().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        if !valid_volume(v) {
            return Err(format!("invalid volume: {v} (use name-or-path:/container/path[:ro])"));
        }
        a.extend(["-v".into(), v.into()]);
    }
    match o.restart.as_str() {
        "" | "no" => {}
        "always" | "unless-stopped" | "on-failure" => a.extend(["--restart".into(), o.restart.clone()]),
        other => return Err(format!("unknown restart policy: {other}")),
    }
    a.push(o.image.clone());
    a.extend(split_command(&o.command)?);
    Ok(a)
}

/// `docker run -d` with the options from the Run dialog.
pub fn image_run(o: &RunOptions) -> Result<String, String> {
    let args = run_args(o)?;
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs)
}

pub fn prune_images() -> Result<String, String> {
    run(&["image", "prune", "-a", "-f"])
}

pub fn prune_containers() -> Result<String, String> {
    run(&["container", "prune", "-f"])
}

pub fn volume_create(name: &str) -> Result<String, String> {
    run(&["volume", "create", name_arg(name)?])
}

pub fn volume_remove(name: &str, force: bool) -> Result<String, String> {
    let name = name_arg(name)?;
    if force { run(&["volume", "rm", "-f", name]) } else { run(&["volume", "rm", name]) }
}

pub fn prune_volumes() -> Result<String, String> {
    run(&["volume", "prune", "-a", "-f"])
}

pub fn build_history_remove(reference: &str) -> Result<String, String> {
    run(&["buildx", "history", "rm", ref_arg(reference)?])
}

pub fn prune_build_cache() -> Result<String, String> {
    run(&["builder", "prune", "-a", "-f"])
}

// ---- streaming commands -------------------------------------------------------------------

/// Build (but do not spawn) a long-running command with piped output. `kind` is one of
/// `logs` (target = container), `pull` (target = image) or `build` (target = tag,
/// context = folder holding a Dockerfile).
pub fn stream_command(
    kind: &str,
    target: &str,
    context: &str,
    tail: u32,
    timestamps: bool,
    since: &str,
) -> Result<Command, String> {
    let mut cmd = match kind {
        "logs" => {
            let tail = tail.clamp(1, 100_000).to_string();
            let mut args = vec!["logs", "-f"];
            // Re-attaching after a restart continues from a point in time instead of repeating history.
            if since.is_empty() {
                args.extend(["--tail", tail.as_str()]);
            } else if valid_since(since) {
                args.extend(["--since", since]);
            } else {
                return Err(format!("invalid time: {since}"));
            }
            if timestamps {
                args.push("--timestamps");
            }
            args.push(name_arg(target)?);
            command(&args)
        }
        "pull" => command(&["pull", ref_arg(target)?]),
        "stats" => command(&["stats", "--format", "{{json .}}", name_arg(target)?]),
        // Lifecycle events only; health checks otherwise flood the stream with exec_* events.
        "events" => {
            const EVENTS: [&str; 18] = [
                "create", "start", "stop", "die", "destroy", "kill", "oom", "pause", "unpause", "rename", "restart",
                "health_status", "pull", "delete", "tag", "untag", "import", "load",
            ];
            let filters: Vec<String> = ["type=container", "type=image", "type=volume"]
                .iter()
                .map(|s| s.to_string())
                .chain(EVENTS.iter().map(|e| format!("event={e}")))
                .collect();
            let mut args: Vec<&str> = vec!["events", "--format", "{{json .}}"];
            for f in &filters {
                args.extend(["--filter", f.as_str()]);
            }
            command(&args)
        }
        "build" => {
            let dir = validate_dir(context)?;
            if !Path::new(&dir).join("Dockerfile").is_file() {
                return Err(format!("no Dockerfile in {dir}"));
            }
            let mut c = command(&["build", "--progress=plain"]);
            if !target.is_empty() {
                c.args(["-t", ref_arg(target)?]);
            }
            c.arg(dir);
            c
        }
        _ => return Err(format!("unknown stream: {kind}")),
    };
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_reject_flags_and_separators() {
        assert!(valid_name("web-1"));
        assert!(valid_name("my_vol.data"));
        assert!(!valid_name("-rf"));
        assert!(!valid_name(".hidden"));
        assert!(!valid_name("a b"));
        assert!(!valid_name("a;b"));
        assert!(!valid_name(""));
    }

    #[test]
    fn refs_allow_registry_paths_and_digests() {
        assert!(valid_ref("postgres:17-alpine"));
        assert!(valid_ref("ghcr.io/org/app:1.2"));
        assert!(valid_ref("app@sha256:abcdef"));
        assert!(valid_ref("desktop-linux/desktop-linux/0mz8"));
        assert!(!valid_ref("--privileged"));
        assert!(!valid_ref("a b"));
        assert!(!valid_ref("a&b"));
    }

    #[test]
    fn port_maps_need_two_numbers() {
        assert!(valid_port_map("8080:80"));
        assert!(!valid_port_map("8080"));
        assert!(!valid_port_map("8080:80:1"));
        assert!(!valid_port_map("a:80"));
        assert!(!valid_port_map("123456:80"));
    }

    #[test]
    fn env_volume_and_command_parsing() {
        assert!(valid_env("DB_URL=postgres://u:p@h/db"));
        assert!(valid_env("EMPTY="));
        assert!(!valid_env("1BAD=x"));
        assert!(!valid_env("NOEQUALS"));
        assert!(!valid_env("A B=x"));
        assert!(valid_volume("data:/var/lib/data"));
        assert!(valid_volume("data:/var/lib/data:ro"));
        assert!(valid_volume(r"C:\Users\me\app:/app"));
        assert!(valid_volume(r"C:\Users\me\app:/app:rw"));
        assert!(!valid_volume("data"));
        assert!(!valid_volume("data:relative"));
        assert!(!valid_volume("-v:/x"));
        assert_eq!(split_command(r#"sh -c "echo hi there" 'a b'"#).unwrap(), vec!["sh", "-c", "echo hi there", "a b"]);
        assert_eq!(split_command("  ").unwrap(), Vec::<String>::new());
        assert!(split_command("echo \"oops").is_err());
        assert!(valid_port_map("5353:53/udp"));
    }

    #[test]
    fn run_args_are_assembled_in_order() {
        let o = RunOptions {
            image: "nginx:latest".into(),
            name: "web".into(),
            ports: vec!["8080:80".into(), " ".into()],
            env: vec!["A=1".into()],
            volumes: vec!["data:/d:ro".into()],
            restart: "unless-stopped".into(),
            command: "nginx -g 'daemon off;'".into(),
        };
        assert_eq!(
            run_args(&o).unwrap(),
            ["run", "-d", "--name", "web", "-p", "8080:80", "-e", "A=1", "-v", "data:/d:ro", "--restart", "unless-stopped", "nginx:latest", "nginx", "-g", "daemon off;"]
        );
        let bad = RunOptions { image: "x".into(), ports: vec!["nope".into()], ..Default::default() };
        assert!(run_args(&bad).is_err());
        let flag = RunOptions { image: "--privileged".into(), ..Default::default() };
        assert!(run_args(&flag).is_err());
        let policy = RunOptions { image: "x".into(), restart: "sometimes".into(), ..Default::default() };
        assert!(run_args(&policy).is_err());
    }

    #[test]
    fn container_paths_must_be_absolute_and_clean() {
        assert!(valid_container_path("/var/log"));
        assert!(valid_container_path("/"));
        assert!(!valid_container_path("var/log"));
        assert!(!valid_container_path("/a\nb"));
        assert!(!valid_container_path("--help"));
        assert!(container_ls("web", "relative").is_err());
        assert!(container_ls("-x", "/").is_err());
        assert!(container_copy_in("web", "Z:\\definitely\\missing", "/tmp").is_err());
        assert!(container_copy_out("web", "/etc", "Z:\\definitely\\missing").is_err());
    }

    #[test]
    fn shell_args_reject_injection() {
        assert!(shell_args("--privileged").is_err());
        assert!(shell_args("web;rm").is_err());
        assert_eq!(shell_args("web-1").unwrap()[2], "web-1");
    }

    #[test]
    fn actions_and_inspect_kinds_are_whitelisted() {
        assert!(container_action("web", "rm -rf").is_err());
        assert!(container_action("-x", "start").is_err());
        assert!(inspect("network", "x").is_err());
    }

    #[test]
    fn hung_commands_are_killed_at_the_deadline() {
        // `docker` exists or not, neither may hang the caller past the deadline.
        let t = Instant::now();
        let _ = output_within(&["events"], Duration::from_millis(300));
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn stream_rejects_unknown_kind_and_bad_dir() {
        assert!(stream_command("exec", "x", "", 500, true, "").is_err());
        assert!(stream_command("build", "t", "Z:\\definitely\\missing", 500, true, "").is_err());
        assert!(stream_command("logs", "web", "", 500, true, "now; calc").is_err());
    }

    #[test]
    fn since_accepts_only_timestamps() {
        assert!(valid_since("2026-10-21T22:00:00.123Z"));
        assert!(valid_since("2026-10-21T22:00:00+02:00"));
        assert!(!valid_since(""));
        assert!(!valid_since("--follow"));
        assert!(!valid_since("1h ago"));
    }

    #[test]
    fn engine_config_must_be_a_json_object() {
        assert_eq!(normalize_engine_config("  ").unwrap(), "{}\n");
        assert!(normalize_engine_config(r#"{"debug":true}"#).unwrap().contains("\"debug\": true"));
        assert!(normalize_engine_config("[1]").is_err());
        assert!(normalize_engine_config("{oops").is_err());
    }

    #[test]
    fn docker_path_must_be_a_docker_file() {
        assert!(set_docker_path("Z:\\missing\\docker.exe").is_err());
        assert!(set_docker_path("   ").is_ok()); // clears
    }
}
