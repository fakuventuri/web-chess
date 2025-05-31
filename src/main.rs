use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
// use tower::ServiceExt;
use futures_util::{SinkExt, StreamExt};
use tower_http::services::ServeDir;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<String>,
    players: Arc<Mutex<HashMap<Uuid, usize>>>,
    fen: Arc<Mutex<String>>,
}

#[tokio::main]
async fn main() {
    let (tx, _) = broadcast::channel(16);
    let state = AppState {
        tx,
        players: Arc::new(Mutex::new(HashMap::new())),
        fen: Arc::new(Mutex::new("startpos".to_string())),
    };

    let serve_dir = get_service(ServeDir::new("static"));

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route_service("/", serve_dir)
        .with_state(state);

    // let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    // println!("Listening on {}", addr);
    // axum::serve(&addr)
    //     .serve(app.into_make_service())
    //     .await
    //     .unwrap();

    let addr = "0.0.0.0:3001";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let id = Uuid::new_v4();
    // let mut role = "spectator"; // default spectator

    {
        let mut players = state.players.lock().unwrap();
        let len = players.len();

        if len < 2 {
            if players.values().any(|&x| x == 0) {
                players.insert(id, 1);
            } else {
                players.insert(id, 0);
            }
            // role = if len == 0 { "white" } else { "black" };
        }
    }

    let mut rx = state.tx.subscribe();
    let (mut sender, mut receiver) = socket.split();
    let tx = state.tx.clone();

    // ✅ Send role message to the connected client
    let role = match state.players.lock().unwrap().get(&id) {
        Some(0) => "white",
        Some(1) => "black",
        Some(_) => panic!("MORE THAN 2 PLAYERS"),
        None => "spectator",
    };

    let fen = state.fen.lock().unwrap().clone();

    let welcome_message = serde_json::json!({
        "type": "welcome",
        "role": role,
        "fen": fen,
    });

    if let Err(e) = sender
        .send(Message::Text(welcome_message.to_string().into()))
        .await
    {
        eprintln!("Failed to send role + fen info: {}", e);
        return;
    }

    // ✅ Spawn task to send broadcast messages to client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // ✅ Receive messages from client and broadcast
    while let Some(Ok(Message::Text(msg))) = receiver.next().await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
            if json["type"] == "move" {
                // Save current fen in state
                if let Some(fen) = json["fen"].as_str() {
                    let mut fen_lock = state.fen.lock().unwrap();
                    *fen_lock = fen.to_string();
                }

                // Broadcast msg
                let _ = tx.send(msg.to_string());
                continue;
            }
            if json["type"] == "reset" {
                // Reset state fen to "starpos"
                let mut fen_lock = state.fen.lock().unwrap();
                *fen_lock = "startpos".to_string();

                // Broadcast msg
                let _ = tx.send(msg.to_string());
                continue;
            }
        }
    }

    // ✅ Clean up on disconnect
    state.players.lock().unwrap().remove(&id);
    send_task.abort(); // stop the sending task
}
