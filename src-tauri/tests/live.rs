//! Tests against a real, local Docker engine. They are ignored by default:
//!   cargo test --test live -- --ignored --test-threads=1
//! Each test creates a uniquely named disposable container and removes it afterwards.
//! They need at least one image already present locally (no pulls); the smallest one is used.

use better_docker_desktop_lib::docker;

/// Any local image with a `sleep` command; prefers small ones.
fn local_image() -> String {
    let out = docker::run(&["images", "--format", "{{.Repository}}:{{.Tag}}|{{.Size}}"]).expect("docker images");
    out.lines()
        .filter(|l| !l.contains("<none>"))
        .map(|l| l.split('|').next().unwrap().to_string())
        .find(|r| r.contains("alpine") || r.contains("busybox"))
        .or_else(|| out.lines().next().map(|l| l.split('|').next().unwrap().to_string()))
        .expect("no local image available; pull one first")
}

fn cleanup(name: &str) {
    let _ = docker::container_action(name, "remove");
}

#[test]
#[ignore]
fn engine_is_reachable() {
    let s = docker::status();
    assert!(s.daemon_running, "engine not running: {:?}", s.error);
    assert!(s.docker_version.is_some());
}

#[test]
#[ignore]
fn run_with_options_then_list_inspect_and_remove() {
    let name = "bdd-live-run";
    cleanup(name);
    let o = docker::RunOptions {
        image: local_image(),
        name: name.into(),
        ports: vec!["18099:80".into()],
        env: vec!["BDD_TEST=hello world".into()],
        volumes: vec![],
        restart: "no".into(),
        command: "sleep 60".into(),
    };
    docker::image_run(&o).expect("run");
    let list = docker::list_containers().unwrap();
    assert!(list.contains(name), "container missing from `ps -a`");
    let inspect = docker::inspect("container", name).unwrap();
    assert!(inspect.contains("BDD_TEST=hello world"), "env not applied");
    assert!(inspect.contains("18099"), "port not applied");
    docker::container_action(name, "pause").unwrap();
    docker::container_action(name, "unpause").unwrap();
    docker::container_action(name, "stop").unwrap();
    docker::container_action(name, "remove").unwrap();
    assert!(!docker::list_containers().unwrap().contains(name));
}

#[test]
#[ignore]
fn events_stream_reports_container_lifecycle() {
    use std::io::{BufRead, BufReader};
    use std::time::{Duration, Instant};

    let name = "bdd-live-events";
    cleanup(name);
    let mut child = docker::stream_command("events", "", "", 500, true, "").unwrap().spawn().unwrap();
    let out = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(l).is_err() {
                break;
            }
        }
    });
    std::thread::sleep(Duration::from_secs(1));
    let o = docker::RunOptions { image: local_image(), name: name.into(), command: "sleep 30".into(), ..Default::default() };
    docker::image_run(&o).expect("run");
    let (mut saw_start, mut noise) = (false, false);
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline && !saw_start {
        if let Ok(line) = rx.recv_timeout(Duration::from_secs(1)) {
            let ev = line.replace(' ', "");
            noise |= ev.contains("exec_create") || ev.contains("exec_start");
            saw_start |= ev.contains("\"Action\":\"start\"") && ev.contains(name);
        }
    }
    let _ = child.kill();
    cleanup(name);
    assert!(saw_start, "no start event seen for {name}");
    assert!(!noise, "exec_* events should be filtered out");
}

#[test]
#[ignore]
fn stats_stream_emits_json_samples() {
    use std::io::{BufRead, BufReader};
    use std::time::{Duration, Instant};

    let name = "bdd-live-stats";
    cleanup(name);
    let o = docker::RunOptions { image: local_image(), name: name.into(), command: "sleep 60".into(), ..Default::default() };
    docker::image_run(&o).expect("run");
    let mut child = docker::stream_command("stats", name, "", 500, true, "").unwrap().spawn().unwrap();
    let out = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(l).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut got = None;
    while Instant::now() < deadline && got.is_none() {
        if let Ok(l) = rx.recv_timeout(Duration::from_secs(1)) {
            if l.contains("\"CPUPerc\"") {
                got = Some(l);
            }
        }
    }
    let _ = child.kill();
    cleanup(name);
    let line = got.expect("no stats sample within 20s");
    assert!(line.contains("\"MemUsage\"") && line.contains(name));
}

#[test]
#[ignore]
fn container_files_list_and_copy_round_trip() {
    let name = "bdd-live-files";
    cleanup(name);
    let o = docker::RunOptions { image: local_image(), name: name.into(), command: "sleep 60".into(), ..Default::default() };
    docker::image_run(&o).expect("run");

    let listing = docker::container_ls(name, "/").expect("ls /");
    assert!(listing.contains("etc"), "root listing should contain etc:\n{listing}");

    let dir = std::env::temp_dir().join("bdd-live-files");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("hello.txt");
    std::fs::write(&src, "hello from the host").unwrap();
    docker::container_copy_in(name, src.to_str().unwrap(), "/tmp").expect("copy in");
    assert!(docker::container_ls(name, "/tmp").unwrap().contains("hello.txt"));

    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    docker::container_copy_out(name, "/tmp/hello.txt", out.to_str().unwrap()).expect("copy out");
    assert_eq!(std::fs::read_to_string(out.join("hello.txt")).unwrap(), "hello from the host");

    cleanup(name);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore]
fn volumes_create_list_remove() {
    let name = "bdd-live-vol";
    let _ = docker::volume_remove(name, true);
    docker::volume_create(name).unwrap();
    let df = docker::disk_usage().unwrap();
    assert!(df.contains(name), "volume missing from system df");
    docker::volume_remove(name, false).unwrap();
}

#[test]
#[ignore]
fn invalid_requests_never_reach_docker() {
    let o = docker::RunOptions { image: local_image(), name: "-bad".into(), ..Default::default() };
    assert!(docker::image_run(&o).is_err());
    assert!(docker::container_action("web; rm -rf /", "start").is_err());
}
