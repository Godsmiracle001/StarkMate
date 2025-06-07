use futures_util::{SinkExt, StreamExt};
use serde_json::{from_str, to_string};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

use crate::game::{get_game_log, get_room_sender, join_room, leave_room, send_move};
use crate::models::{ClientMessage, ServerMessage};

// Handle a client message
pub async fn handle_client_message(
    message: &str,
    sender: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        Message,
    >,
    room_senders: &mut Vec<(String, broadcast::Sender<ServerMessage>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse the message
    let client_message: ClientMessage = match from_str(message) {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("Failed to parse client message: {}", e);
            let error_msg = ServerMessage::Error {
                code: "PARSE_ERROR".to_string(),
                message: "Failed to parse message".to_string(),
            };
            sender
                .send(Message::Text(to_string(&error_msg)?))
                .await?;
            return Ok(());
        }
    };

    // Handle the message based on its type
    match client_message {
        ClientMessage::JoinRoom(payload) => {
            log::info!(
                "Player {} joining room {}",
                payload.player_id,
                payload.room_id
            );

            match join_room(&payload.room_id, &payload.player_id, payload.player_name) {
                Ok(response) => {
                    // Send response to client
                    sender.send(Message::Text(to_string(&response)?)).await?;

                    // Subscribe to room messages
                    if let Some(room_sender) = get_room_sender(&payload.room_id) {
                        room_senders.push((payload.room_id, room_sender));
                    }
                }
                Err(e) => {
                    let error_msg = ServerMessage::Error {
                        code: "JOIN_ERROR".to_string(),
                        message: e,
                    };
                    sender.send(Message::Text(to_string(&error_msg)?)).await?;
                }
            }
        }
        ClientMessage::SendMove(payload) => {
            log::info!(
                "Player {} making move {} in room {}",
                payload.player_id,
                payload.move_notation,
                payload.room_id
            );

            match send_move(&payload.room_id, &payload.player_id, &payload.move_notation) {
                Ok(response) => {
                    sender.send(Message::Text(to_string(&response)?)).await?;
                }
                Err(e) => {
                    let error_msg = ServerMessage::Error {
                        code: "MOVE_ERROR".to_string(),
                        message: e,
                    };
                    sender.send(Message::Text(to_string(&error_msg)?)).await?;
                }
            }
        }
        ClientMessage::LeaveRoom(payload) => {
            log::info!(
                "Player {} leaving room {}",
                payload.player_id,
                payload.room_id
            );

            match leave_room(&payload.room_id, &payload.player_id) {
                Ok(response) => {
                    sender.send(Message::Text(to_string(&response)?)).await?;

                    // Unsubscribe from room messages
                    room_senders.retain(|(id, _)| id != &payload.room_id);
                }
                Err(e) => {
                    let error_msg = ServerMessage::Error {
                        code: "LEAVE_ERROR".to_string(),
                        message: e,
                    };
                    sender.send(Message::Text(to_string(&error_msg)?)).await?;
                }
            }
        }
        ClientMessage::RequestGameLog(payload) => {
            log::info!("Game log requested for room {}", payload.room_id);

            match get_game_log(&payload.room_id) {
                Ok(response) => {
                    sender.send(Message::Text(to_string(&response)?)).await?;
                }
                Err(e) => {
                    let error_msg = ServerMessage::Error {
                        code: "LOG_ERROR".to_string(),
                        message: e,
                    };
                    sender.send(Message::Text(to_string(&error_msg)?)).await?;
                }
            }
        }
    }

    Ok(())
}
