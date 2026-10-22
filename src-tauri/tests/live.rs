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
fn image_history_save_and_load() {
    let image = local_image();
    let history = docker::image_history(&image).expect("history");
    assert!(history.lines().count() > 0 && history.contains("CreatedBy"));

    let dir = std::env::temp_dir().join("bdd-live-save");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tar = dir.join("image.tar");
    let tar_s = tar.to_str().unwrap();

    let status = docker::stream_command("save", &image, tar_s, 1, false, "").unwrap().status().unwrap();
    assert!(status.success(), "docker save failed");
    assert!(std::fs::metadata(&tar).unwrap().len() > 1000, "archive is empty");

    let status = docker::stream_command("load", "", tar_s, 1, false, "").unwrap().status().unwrap();
    assert!(status.success(), "docker load failed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore]
fn volume_users_are_listed() {
    let (vol, name) = ("bdd-live-users-vol", "bdd-live-users");
    cleanup(name);
    let _ = docker::volume_remove(vol, true);
    docker::volume_create(vol).unwrap();
    assert!(docker::containers_using_volume(vol).unwrap().is_empty(), "fresh volume has no users");
    let o = docker::RunOptions {
        image: local_image(),
        name: name.into(),
        volumes: vec![format!("{vol}:/data")],
        command: "sleep 30".into(),
        ..Default::default()
    };
    docker::image_run(&o).expect("run");
    assert!(docker::containers_using_volume(vol).unwrap().contains(name));
    cleanup(name);
    docker::volume_remove(vol, false).unwrap();
}

#[test]
#[ignore]
fn compose_project_stop_start_down_by_name() {
    let dir = std::env::temp_dir().join("bdd-live-compose");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("compose.yaml");
    std::fs::write(&file, format!("services:\n  one:\n    image: {}\n    command: sleep 60\n  two:\n    image: {}\n    command: sleep 60\n", local_image(), local_image())).unwrap();
    let project = "bddlive";
    docker::run(&["compose", "-p", project, "-f", file.to_str().unwrap(), "up", "-d"]).expect("compose up");

    let running = || docker::run(&["ps", "--filter", &format!("label=com.docker.compose.project={project}"), "-q"]).unwrap();
    assert_eq!(running().lines().count(), 2, "both services should be running");
    docker::compose_project_action(project, "stop").unwrap();
    assert_eq!(running().lines().count(), 0, "stop should stop every container");
    docker::compose_project_action(project, "start").unwrap();
    assert_eq!(running().lines().count(), 2);
    docker::compose_project_action(project, "restart").unwrap();
    assert_eq!(running().lines().count(), 2);
    docker::compose_project_action(project, "down").unwrap();
    let all = docker::run(&["ps", "-a", "--filter", &format!("label=com.docker.compose.project={project}"), "-q"]).unwrap();
    assert!(all.is_empty(), "down should remove the containers");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore]
fn volume_contents_can_be_browsed_read_only() {
    let Some(helper) = docker::volume_helper_image().unwrap() else {
        eprintln!("skipped: no local alpine or busybox image (the app offers to pull one)");
        return;
    };
    let vol = "bdd-live-browse";
    let _ = docker::volume_remove(vol, true);
    docker::volume_create(vol).unwrap();
    // Seed the volume through a writable mount (this is test setup, not the app's read path).
    docker::run(&[
        "run", "--rm", "-v", &format!("{vol}:/d"), "--entrypoint", "sh", &helper, "-c",
        "mkdir -p /d/sub && echo hello volume > /d/a.txt && echo nested > '/d/sub/b c.txt' && head -c 100000 /dev/zero > /d/big.bin",
    ])
    .expect("seed volume");

    let root = docker::volume_ls(vol, &helper, "/").expect("ls /");
    assert!(root.contains("a.txt") && root.contains("sub") && root.contains("big.bin"), "root listing:\n{root}");
    let sub = docker::volume_ls(vol, &helper, "/sub").expect("ls /sub");
    assert!(sub.contains("b c.txt"), "names with spaces survive:\n{sub}");
    assert_eq!(docker::volume_read(vol, &helper, "/a.txt").unwrap().trim(), "hello volume");
    assert_eq!(docker::volume_read(vol, &helper, "/big.bin").unwrap().len(), 65536, "reads are capped at 64 KiB");

    // The mount really is read-only, and the app cannot be pointed outside the volume.
    assert!(docker::run(&["run", "--rm", "-v", &format!("{vol}:/v:ro"), "--entrypoint", "touch", &helper, "/v/new"]).is_err());
    assert!(docker::volume_ls(vol, &helper, "/../etc").is_err());
    assert!(docker::run(&["ps", "-a", "--filter", "ancestor=alpine", "-q"]).unwrap().is_empty(), "helper containers are removed");

    docker::volume_remove(vol, false).unwrap();
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
