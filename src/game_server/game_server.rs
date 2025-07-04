use std::{
    thread,
    sync::{Arc, RwLock},
    collections::HashMap,
    net::TcpListener,
    time::Duration,
};

use tungstenite::{
    accept,
    protocol::{Role, WebSocket},
    Message,
};

use crate::game_server::message_queue::MessageQueue;
use crate::game_server::tandem_game::{TandemGameInterface, TandemMove};


pub fn start_server() {
    thread::spawn(|| {
        let server = TcpListener::bind("0.0.0.0:9091").unwrap();
        let board_og = TandemGameInterface::new();
        let client_map = Arc::new(RwLock::new(HashMap::<usize, MessageQueue<String>>::new()));
        let client_sync_map = client_map.clone();
        let tandem_sync = board_og.clone();
        let mut i = 0;

        thread::spawn(move || {
            let mut ping_cnt = 0;
    
            loop {
                if tandem_sync.should_update() || ping_cnt >= 100 {
                    for client in client_sync_map.read().unwrap().values() {
                        client.produce(tandem_sync.get_fen(true));
                    }

                    ping_cnt = 0;
                }

                thread::sleep(Duration::from_millis(50));
                ping_cnt += 1;
            }
        });

        for stream in server.incoming() {
            let board = board_og.clone();
            let id = i;
            let client_map_c = client_map.clone();
            i += 1;

            thread::spawn(move || {
                let stream_read = stream.unwrap();
                let send_stream = stream_read.try_clone().unwrap();

                let mut websocket_read = match accept(stream_read) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let msg_queue = MessageQueue::<String>::new();
                let msg_queue_c = msg_queue.clone();
                let mut websocket_send = WebSocket::from_raw_socket(send_stream, Role::Server, None);

                thread::spawn(move || {
                    loop {
                        let msg = msg_queue_c.consume_blocking();

                        match websocket_send.send(Message::Text(msg.into())) {
                            Ok(_) => (),
                            Err(_) => break, 
                        };
                    }
                });

                msg_queue.produce(board.get_fen(true));
                client_map_c.write().unwrap().insert(id, msg_queue.clone());

                loop {
                    let msg:String = match websocket_read.read() {
                        Ok(message) => match message {
                            msg @ Message::Text(_) => msg.to_string(),
                            _msg @ Message::Ping(_) | _msg @ Message::Pong(_) => continue,
                            _ => break,
                        },
                        Err(_) => break,
                    };

                    if msg == "Reset Game" {
                        board.reset();

                        for client in client_map_c.read().unwrap().values() {
                            client.produce(board.get_fen(true));
                        }

                        continue;
                    }

                    let tandem_move = match TandemMove::from_string(msg) {
                        Some(v) => v,
                        None => continue,
                    };

                    let changed = board.move_piece(&tandem_move);

                    if changed {
                        for client in client_map_c.read().unwrap().values() {
                            client.produce(board.get_fen(true));
                        }
                    } else {
                        msg_queue.produce(board.get_fen(false));
                    }
                }
            });
        }
    });
}