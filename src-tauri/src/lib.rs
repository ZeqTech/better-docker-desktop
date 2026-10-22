pub mod docker;
mod shell;

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::Child;
use std::sync::{Arc, Mutex};
use tauri::async_runtime::spawn_blocking;
use tauri::{AppHandle, Emitter, Manager, State};

type Streams = Mutex<HashMap<String, Arc<Mutex<Child>>>>;

#[derive(Clone, serde::Serialize)]
struct StreamLine {
    id: String,
    line: String,
}

#[derive(Clone, serde::Serialize)]
struct StreamEnd {
    id: String,
    code: Option<i32>,
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
async fn docker_status() -> Result<docker::DockerStatus, String> {
    blocking(|| Ok(docker::status())).await
}

#[tauri::command]
async fn start_engine() -> Result<String, String> {
    blocking(docker::start_engine).await
}

#[tauri::command]
fn set_docker_path(path: String) -> Result<(), String> {
    docker::set_docker_path(&path)
}

#[tauri::command]
async fn read_engine_config() -> Result<docker::EngineConfig, String> {
    blocking(docker::read_engine_config).await
}

#[tauri::command]
async fn write_engine_config(content: String) -> Result<String, String> {
    blocking(move || docker::write_engine_config(&content)).await
}

#[tauri::command]
async fn container_ls(container: String, path: String) -> Result<String, String> {
    blocking(move || docker::container_ls(&container, &path)).await
}

#[tauri::command]
async fn container_copy_out(container: String, path: String, dest: String) -> Result<String, String> {
    blocking(move || docker::container_copy_out(&container, &path, &dest)).await
}

#[tauri::command]
async fn container_copy_in(container: String, src: String, dest: String) -> Result<String, String> {
    blocking(move || docker::container_copy_in(&container, &src, &dest)).await
}

#[tauri::command]
async fn image_history(image: String) -> Result<String, String> {
    blocking(move || docker::image_history(&image)).await
}

#[tauri::command]
async fn volume_containers(name: String) -> Result<String, String> {
    blocking(move || docker::containers_using_volume(&name)).await
}

#[tauri::command]
async fn volume_helper_image() -> Result<Option<String>, String> {
    blocking(docker::volume_helper_image).await
}

#[tauri::command]
async fn volume_ls(volume: String, image: String, path: String) -> Result<String, String> {
    blocking(move || docker::volume_ls(&volume, &image, &path)).await
}

#[tauri::command]
async fn volume_read(volume: String, image: String, path: String) -> Result<String, String> {
    blocking(move || docker::volume_read(&volume, &image, &path)).await
}

#[tauri::command]
async fn list_containers() -> Result<String, String> {
    blocking(docker::list_containers).await
}

#[tauri::command]
async fn container_stats() -> Result<String, String> {
    blocking(docker::stats).await
}

#[tauri::command]
async fn list_images() -> Result<String, String> {
    blocking(docker::list_images).await
}

#[tauri::command]
async fn disk_usage() -> Result<String, String> {
    blocking(docker::disk_usage).await
}

#[tauri::command]
async fn build_history() -> Result<String, String> {
    blocking(docker::build_history).await
}

#[tauri::command]
async fn inspect(kind: String, target: String) -> Result<String, String> {
    blocking(move || docker::inspect(&kind, &target)).await
}

#[tauri::command]
async fn container_action(id: String, action: String) -> Result<String, String> {
    blocking(move || docker::container_action(&id, &action)).await
}

#[tauri::command]
async fn compose_project_action(project: String, action: String) -> Result<String, String> {
    blocking(move || docker::compose_project_action(&project, &action)).await
}

#[tauri::command]
async fn image_remove(image: String, force: bool) -> Result<String, String> {
    blocking(move || docker::image_remove(&image, force)).await
}

#[tauri::command]
async fn image_tag(source: String, target: String) -> Result<String, String> {
    blocking(move || docker::image_tag(&source, &target)).await
}

#[tauri::command]
async fn image_run(opts: docker::RunOptions) -> Result<String, String> {
    blocking(move || docker::image_run(&opts)).await
}

#[tauri::command]
async fn prune(kind: String) -> Result<String, String> {
    blocking(move || match kind.as_str() {
        "images" => docker::prune_images(),
        "containers" => docker::prune_containers(),
        "volumes" => docker::prune_volumes(),
        "buildcache" => docker::prune_build_cache(),
        other => Err(format!("cannot prune {other}")),
    })
    .await
}

#[tauri::command]
async fn volume_create(name: String) -> Result<String, String> {
    blocking(move || docker::volume_create(&name)).await
}

#[tauri::command]
async fn volume_remove(name: String, force: bool) -> Result<String, String> {
    blocking(move || docker::volume_remove(&name, force)).await
}

#[tauri::command]
async fn build_history_remove(reference: String) -> Result<String, String> {
    blocking(move || docker::build_history_remove(&reference)).await
}

/// Start a long-running command (`logs`, `pull`, `build`). Output arrives as `stream-line`
/// events and the end as `stream-end`; starting a second stream under the same id replaces it.
#[tauri::command]
fn start_stream(
    app: AppHandle,
    streams: State<'_, Streams>,
    id: String,
    kind: String,
    target: String,
    context: Option<String>,
    tail: Option<u32>,
    timestamps: Option<bool>,
    since: Option<String>,
) -> Result<(), String> {
    let mut cmd = docker::stream_command(
        &kind,
        &target,
        context.as_deref().unwrap_or(""),
        tail.unwrap_or(500),
        timestamps.unwrap_or(true),
        since.as_deref().unwrap_or(""),
    )?;
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let out = child.stdout.take().ok_or("no stdout")?;
    let err = child.stderr.take().ok_or("no stderr")?;
    let child = Arc::new(Mutex::new(child));
    {
        let mut map = streams.lock().unwrap();
        if let Some(old) = map.insert(id.clone(), child.clone()) {
            let _ = old.lock().unwrap().kill();
        }
    }

    let pump = |reader: Box<dyn std::io::Read + Send>, app: AppHandle, id: String| {
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                let _ = app.emit("stream-line", StreamLine { id: id.clone(), line });
            }
        })
    };
    let t1 = pump(Box::new(out), app.clone(), id.clone());
    let t2 = pump(Box::new(err), app.clone(), id.clone());

    std::thread::spawn(move || {
        let _ = t1.join();
        let _ = t2.join();
        let code = child.lock().unwrap().wait().ok().and_then(|s| s.code());
        let state = app.state::<Streams>();
        let mut map = state.lock().unwrap();
        // Only announce and drop our own entry; a restart under the same id may have replaced it.
        if map.get(&id).is_some_and(|c| Arc::ptr_eq(c, &child)) {
            map.remove(&id);
            let _ = app.emit("stream-end", StreamEnd { id, code });
        }
    });
    Ok(())
}

#[tauri::command]
fn stop_stream(streams: State<'_, Streams>, id: String) {
    if let Some(child) = streams.lock().unwrap().remove(&id) {
        let _ = child.lock().unwrap().kill();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(Streams::default())
        .manage(shell::Shells::default())
        .invoke_handler(tauri::generate_handler![
            app_version,
            docker_status,
            start_engine,
            image_history,
            compose_project_action,
            volume_containers,
            volume_helper_image,
            volume_ls,
            volume_read,
            container_ls,
            container_copy_out,
            container_copy_in,
            set_docker_path,
            read_engine_config,
            write_engine_config,
            list_containers,
            container_stats,
            list_images,
            disk_usage,
            build_history,
            inspect,
            container_action,
            image_remove,
            image_tag,
            image_run,
            prune,
            volume_create,
            volume_remove,
            build_history_remove,
            start_stream,
            stop_stream,
            shell::start_shell,
            shell::shell_write,
            shell::shell_resize,
            shell::stop_shell
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
