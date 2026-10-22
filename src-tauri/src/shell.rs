//! Interactive shells inside containers, run through a real pseudo-terminal
//! (ConPTY on Windows) so prompts, colours and line editing work.

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct Session {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub type Shells = Mutex<HashMap<String, Arc<Mutex<Session>>>>;

#[derive(Clone, serde::Serialize)]
struct ShellOut {
    id: String,
    data: String,
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 }
}

/// Split off a trailing incomplete UTF-8 sequence so multi-byte characters survive chunking.
pub fn split_incomplete_utf8(buf: &[u8]) -> usize {
    match std::str::from_utf8(buf) {
        Ok(_) => buf.len(),
        Err(e) if e.error_len().is_none() => e.valid_up_to(),
        Err(_) => buf.len(), // invalid bytes: let from_utf8_lossy replace them
    }
}

#[tauri::command]
pub fn start_shell(
    app: AppHandle,
    shells: State<'_, Shells>,
    id: String,
    container: String,
    cols: u16,
    rows: u16,
    kube_context: Option<String>,
    kube_namespace: Option<String>,
    kube_pod: Option<String>,
) -> Result<(), String> {
    // A pod target runs `kubectl exec`; otherwise `container` is a Docker container.
    let (program, args) = match (kube_context, kube_namespace, kube_pod) {
        (Some(c), Some(n), Some(p)) => (std::path::PathBuf::from(crate::kube::program()), crate::kube::shell_args(&c, &n, &p, &container)?),
        _ => (crate::docker::program(), crate::docker::shell_args(&container)?),
    };
    let pair = native_pty_system().openpty(size(cols, rows)).map_err(|e| e.to_string())?;
    let mut cmd = CommandBuilder::new(program);
    cmd.args(&args);
    cmd.env("TERM", "xterm-256color");
    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let session = Arc::new(Mutex::new(Session { writer, master: pair.master, child }));

    if let Some(old) = shells.lock().unwrap().insert(id.clone(), session.clone()) {
        let _ = old.lock().unwrap().child.kill();
    }

    std::thread::spawn(move || {
        let mut pending: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&chunk[..n]);
                    let end = split_incomplete_utf8(&pending);
                    let data = String::from_utf8_lossy(&pending[..end]).into_owned();
                    pending.drain(..end);
                    if !data.is_empty() {
                        let _ = app.emit("shell-out", ShellOut { id: id.clone(), data });
                    }
                }
            }
        }
        let _ = app.emit("shell-end", id.clone());
        let state = app.state::<Shells>();
        let mut map = state.lock().unwrap();
        if map.get(&id).is_some_and(|s| Arc::ptr_eq(s, &session)) {
            map.remove(&id);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn shell_write(shells: State<'_, Shells>, id: String, data: String) -> Result<(), String> {
    let session = shells.lock().unwrap().get(&id).cloned().ok_or("no such shell")?;
    let mut s = session.lock().unwrap();
    s.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    s.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shell_resize(shells: State<'_, Shells>, id: String, cols: u16, rows: u16) -> Result<(), String> {
    let session = shells.lock().unwrap().get(&id).cloned().ok_or("no such shell")?;
    session.lock().unwrap().master.resize(size(cols, rows)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_shell(shells: State<'_, Shells>, id: String) {
    if let Some(s) = shells.lock().unwrap().remove(&id) {
        let _ = s.lock().unwrap().child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::split_incomplete_utf8;

    #[test]
    fn keeps_whole_text() {
        assert_eq!(split_incomplete_utf8("héllo".as_bytes()), 6);
    }

    #[test]
    fn holds_back_a_split_character() {
        let bytes = "aé".as_bytes(); // 'é' is 2 bytes
        assert_eq!(split_incomplete_utf8(&bytes[..2]), 1);
    }

    #[test]
    fn passes_invalid_bytes_through() {
        assert_eq!(split_incomplete_utf8(&[0xff, 0x41]), 2);
    }
}
