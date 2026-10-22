//! kubectl wrapper. Same rules as `docker.rs`: argv only, no shell, validated names. Every call
//! names its context with `--context=` so the app never changes the user's kubeconfig.

use crate::docker::output_of;
use serde::Serialize;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Serialize, Default)]
pub struct KubeStatus {
    pub available: bool,
    pub contexts: Vec<String>,
    pub current: Option<String>,
    pub error: Option<String>,
}

const TIMEOUT: Duration = Duration::from_secs(30);
const LOCAL_TIMEOUT: Duration = Duration::from_secs(8);

pub fn program() -> std::path::PathBuf {
    crate::docker::find_tool(if cfg!(windows) { "kubectl.exe" } else { "kubectl" })
}

fn command(args: &[String]) -> Command {
    let mut cmd = Command::new(program());
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).trim().to_string()
}

fn run_args(args: &[String], limit: Duration) -> Result<String, String> {
    let o = output_of(command(args), limit, "kubectl", "kubectl")?;
    if o.status.success() {
        Ok(text(&o.stdout))
    } else {
        let err = text(&o.stderr);
        Err(if err.is_empty() { text(&o.stdout) } else { err })
    }
}

// ---- validation -------------------------------------------------------------------------

/// Context names come from the kubeconfig and may hold `:`, `/`, `@` (EKS, GKE, OpenShift).
pub fn valid_context(c: &str) -> bool {
    !c.is_empty()
        && c.len() <= 253
        && !c.starts_with('-')
        && c.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/' | '@' | '=' | '+'))
}

/// Namespace, pod, node and similar object names (DNS-1123 plus the odd uppercase or underscore).
pub fn valid_object(n: &str) -> bool {
    !n.is_empty() && n.len() <= 253 && !n.starts_with(['-', '.']) && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | ':'))
}

/// Resource kinds the app may read. Anything else is refused.
pub const KINDS: [&str; 12] = [
    "nodes", "namespaces", "pods", "deployments", "statefulsets", "daemonsets", "services", "events",
    "configmaps", "ingresses", "jobs", "cronjobs",
];

fn kind_arg(k: &str) -> Result<&str, String> {
    if KINDS.contains(&k) { Ok(k) } else { Err(format!("unsupported resource kind: {k}")) }
}

fn ctx_arg(c: &str) -> Result<String, String> {
    if valid_context(c) { Ok(format!("--context={c}")) } else { Err(format!("invalid context: {c}")) }
}

fn ns_args(ns: &str) -> Result<Vec<String>, String> {
    if ns.is_empty() {
        Ok(vec!["--all-namespaces".into()])
    } else if valid_object(ns) {
        Ok(vec![format!("--namespace={ns}")])
    } else {
        Err(format!("invalid namespace: {ns}"))
    }
}

fn object_arg(n: &str) -> Result<String, String> {
    if valid_object(n) { Ok(n.to_string()) } else { Err(format!("invalid name: {n}")) }
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ---- reads ------------------------------------------------------------------------------

/// kubectl present, plus the contexts in the kubeconfig. Reads local files only, so it is quick
/// even when every cluster is unreachable.
pub fn status() -> KubeStatus {
    let mut s = KubeStatus::default();
    match run_args(&strings(&["config", "get-contexts", "-o", "name"]), LOCAL_TIMEOUT) {
        Ok(out) => {
            s.available = true;
            s.contexts = out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        }
        Err(e) => {
            s.available = !e.contains("not found");
            s.error = Some(e);
            return s;
        }
    }
    s.current = run_args(&strings(&["config", "current-context"]), LOCAL_TIMEOUT).ok().filter(|c| !c.is_empty());
    s
}

/// `kubectl get <kind> -o json` for one namespace, or all when `ns` is empty.
pub fn get(context: &str, kind: &str, ns: &str) -> Result<String, String> {
    let mut args = vec![ctx_arg(context)?, "get".into(), kind_arg(kind)?.into(), "-o".into(), "json".into()];
    if kind != "nodes" && kind != "namespaces" {
        args.extend(ns_args(ns)?);
    }
    run_args(&args, TIMEOUT)
}

/// Live usage from metrics-server (`pods` or `nodes`) as the raw metrics.k8s.io list. Fails when
/// the cluster has no metrics-server; callers treat that as "no usage data".
pub fn metrics(context: &str, kind: &str) -> Result<String, String> {
    if !matches!(kind, "pods" | "nodes") {
        return Err(format!("no metrics for {kind}"));
    }
    let args = vec![ctx_arg(context)?, "get".into(), "--raw".into(), format!("/apis/metrics.k8s.io/v1beta1/{kind}")];
    run_args(&args, TIMEOUT)
}

/// `describe` or `yaml` of one object.
pub fn show(context: &str, mode: &str, kind: &str, name: &str, ns: &str) -> Result<String, String> {
    let mut args = vec![ctx_arg(context)?];
    match mode {
        "describe" => args.push("describe".into()),
        "yaml" => args.extend(strings(&["get", "-o", "yaml"])),
        other => return Err(format!("unknown view: {other}")),
    }
    args.push(kind_arg(kind)?.into());
    args.push(object_arg(name)?);
    if kind != "nodes" && kind != "namespaces" {
        args.push(format!("--namespace={}", object_arg(ns)?));
    }
    run_args(&args, TIMEOUT)
}

// ---- changes ----------------------------------------------------------------------------

/// `delete`, `restart` (rollout restart) or `scale=N` on one object.
pub fn act(context: &str, action: &str, kind: &str, name: &str, ns: &str) -> Result<String, String> {
    let kind = kind_arg(kind)?;
    let mut args = vec![ctx_arg(context)?];
    let target = || -> Result<Vec<String>, String> { Ok(vec![format!("{kind}/{}", object_arg(name)?), format!("--namespace={}", object_arg(ns)?)]) };
    match action {
        "delete" => {
            if !matches!(kind, "pods" | "deployments" | "statefulsets" | "daemonsets" | "services" | "jobs" | "cronjobs" | "configmaps" | "ingresses") {
                return Err(format!("cannot delete {kind} from here"));
            }
            args.push("delete".into());
            args.extend(target()?);
        }
        "restart" => {
            if !matches!(kind, "deployments" | "statefulsets" | "daemonsets") {
                return Err(format!("cannot restart {kind}"));
            }
            args.extend(strings(&["rollout", "restart"]));
            args.extend(target()?);
        }
        "cordon" | "uncordon" => {
            if kind != "nodes" {
                return Err(format!("cannot {action} {kind}"));
            }
            args.push(action.into());
            args.push(object_arg(name)?);
        }
        a if a.starts_with("scale=") => {
            let n: u32 = a["scale=".len()..].parse().map_err(|_| format!("invalid replica count in {a}"))?;
            if n > 1000 || !matches!(kind, "deployments" | "statefulsets") {
                return Err(format!("cannot scale {kind} to {n}"));
            }
            args.extend(strings(&["scale", &format!("--replicas={n}")]));
            args.extend(target()?);
        }
        other => return Err(format!("unknown action: {other}")),
    }
    run_args(&args, TIMEOUT)
}

// ---- streams and shells -----------------------------------------------------------------

/// `kubectl logs -f` for `target` = `namespace/pod/container`.
pub fn logs_command(context: &str, target: &str, tail: u32, timestamps: bool) -> Result<Command, String> {
    let mut parts = target.split('/');
    let (ns, pod, container) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(n), Some(p), Some(c), None) => (n, p, c),
        _ => return Err(format!("invalid log target: {target}")),
    };
    let mut args = vec![
        ctx_arg(context)?,
        format!("--namespace={}", object_arg(ns)?),
        "logs".into(),
        "-f".into(),
        format!("--tail={}", tail.clamp(1, 100_000)),
    ];
    if timestamps {
        args.push("--timestamps".into());
    }
    args.extend([format!("--container={}", object_arg(container)?), object_arg(pod)?]);
    let mut cmd = command(&args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    Ok(cmd)
}

/// argv for an interactive shell in a pod container: bash when present, else sh.
pub fn shell_args(context: &str, ns: &str, pod: &str, container: &str) -> Result<Vec<String>, String> {
    Ok(vec![
        ctx_arg(context)?,
        format!("--namespace={}", object_arg(ns)?),
        "exec".into(),
        "-it".into(),
        object_arg(pod)?,
        format!("--container={}", object_arg(container)?),
        "--".into(),
        "sh".into(),
        "-c".into(),
        "command -v bash >/dev/null 2>&1 && exec bash || exec sh".into(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contexts_allow_cloud_names_but_not_flags() {
        assert!(valid_context("arn:aws:eks:us-east-1:123:cluster/prod"));
        assert!(valid_context("do-nyc3-prod"));
        assert!(!valid_context("--kubeconfig=x"));
        assert!(!valid_context("a b"));
        assert!(!valid_context(""));
    }

    #[test]
    fn objects_reject_flags_and_separators() {
        assert!(valid_object("kube-system"));
        assert!(!valid_object("-A"));
        assert!(!valid_object("a/b"));
        assert!(!valid_object("a;b"));
    }

    #[test]
    fn unknown_kinds_are_refused() {
        assert!(get("ctx", "secrets", "").is_err());
        assert!(show("ctx", "yaml", "secrets", "x", "default").is_err());
    }

    #[test]
    fn metrics_only_for_pods_and_nodes() {
        assert!(metrics("ctx", "secrets").is_err());
        assert!(metrics("--x", "pods").is_err());
    }

    #[test]
    fn shell_args_pin_context_and_container() {
        let a = shell_args("ctx", "default", "web-1", "app").unwrap();
        assert_eq!(a[0], "--context=ctx");
        assert!(a.contains(&"--container=app".to_string()));
        assert!(shell_args("ctx", "default", "-x", "app").is_err());
    }

    #[test]
    fn log_target_needs_three_parts() {
        assert!(logs_command("ctx", "default/web", 10, true).is_err());
        assert!(logs_command("ctx", "default/web/app/x", 10, true).is_err());
        assert!(logs_command("ctx", "default/web/app", 10, true).is_ok());
    }

    #[test]
    fn actions_are_restricted_by_kind() {
        assert!(act("ctx", "restart", "pods", "x", "default").is_err());
        assert!(act("ctx", "scale=2", "services", "x", "default").is_err());
        assert!(act("ctx", "scale=99999", "deployments", "x", "default").is_err());
        assert!(act("ctx", "cordon", "pods", "x", "").is_err());
    }
}
