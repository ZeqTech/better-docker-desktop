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
