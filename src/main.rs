use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, get_service},
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
}

#[tokio::main]
async fn main() {
    let (tx, _) = broadcast::channel(16);
    let state = AppState {
        tx,
        players: Arc::new(Mutex::new(HashMap::new())),
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

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
     println!("Listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let id = Uuid::new_v4();

    {
        let mut players = state.players.lock().unwrap();

        let len = players.len();

        if len < 2 {
            players.insert(id, len);
        }
    }

    let mut rx = state.tx.subscribe();
    let (mut sender, mut receiver) = socket.split();
    let tx = state.tx.clone();

    // Spawn task to send broadcast messages to client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive messages from client and broadcast
    while let Some(Ok(Message::Text(msg))) = receiver.next().await {
        let _ = tx.send(msg.to_string());
    }

    // Clean up on disconnect
    state.players.lock().unwrap().remove(&id);
    send_task.abort(); // stop the sending task
}
