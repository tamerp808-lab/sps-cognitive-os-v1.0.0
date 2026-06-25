//! Interactive terminal — WebSocket-based PTY terminal.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new().route("/api/terminal/ws", get(ws_terminal))
}

async fn ws_terminal(
    State(state): State<Arc<ServerState>>,
    ws: WebSocketUpgrade,
) -> Response {
    let root = state.workspace_root.read().clone();
    ws.on_upgrade(move |socket| handle_terminal(socket, root))
}

async fn handle_terminal(mut socket: WebSocket, root: Option<std::path::PathBuf>) {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let cwd = root.unwrap_or_else(|| std::path::PathBuf::from("."));

    // Send welcome.
    let _ = socket.send(Message::Text(
        json!({"type": "output", "data": "SPS Terminal ready. Type commands and press Enter.\r\n"}).to_string().into(),
    ).into());

    // Track if we have an active process.
    let mut active_process: Option<std::process::Child> = None;

    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Parse the command.
                let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                let cmd_str = parsed.get("command").and_then(|c| c.as_str()).unwrap_or("");

                if cmd_str.is_empty() {
                    continue;
                }

                // Handle built-in commands.
                if cmd_str == "clear" {
                    let _ = socket.send(Message::Text(
                        json!({"type": "clear"}).to_string().into(),
                    ).into());
                    continue;
                }

                if cmd_str == "exit" || cmd_str == "quit" {
                    let _ = socket.send(Message::Text(
                        json!({"type": "output", "data": "Goodbye!\r\n"}).to_string().into(),
                    ).into());
                    break;
                }

                // Parse command + args.
                let parts: Vec<&str> = cmd_str.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                // Execute the command.
                let output = Command::new(parts[0])
                    .args(&parts[1..])
                    .current_dir(&cwd)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output();

                match output {
                    Ok(o) => {
                        let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                        let exit_code = o.status.code().unwrap_or(-1);

                        if !stdout.is_empty() {
                            let _ = socket.send(Message::Text(
                                json!({"type": "output", "data": stdout}).to_string().into(),
                            ).into());
                        }
                        if !stderr.is_empty() {
                            let _ = socket.send(Message::Text(
                                json!({"type": "output", "data": stderr, "is_error": true}).to_string().into(),
                            ).into());
                        }
                        let _ = socket.send(Message::Text(
                            json!({"type": "exit", "code": exit_code}).to_string().into(),
                        ).into());
                    }
                    Err(e) => {
                        let _ = socket.send(Message::Text(
                            json!({"type": "output", "data": format!("Error: {}\r\n", e), "is_error": true}).to_string().into(),
                        ).into());
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // Clean up any active process.
    if let Some(mut child) = active_process {
        let _ = child.kill();
    }
}
