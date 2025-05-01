use std::{
    thread,
    sync::{Arc, RwLock},
    collections::{HashMap, HashSet},
    net::{TcpStream, TcpListener},
    time::{Duration, SystemTime},
    ops::AddAssign,
};

use tungstenite::{
    accept,
    protocol::{Role, WebSocket},
    Message,
};

use chess::{Board, Square, ChessMove, Piece, Color, Rank, BoardStatus};
use crate::game_server::message_queue::MessageQueue;

pub struct ChessBoard {
    pub board: Board,
}

impl ChessBoard {
    pub fn new() -> Self {
        ChessBoard {
            board: Board::default(),
        }
    }

    pub fn get_fen(&self) -> String {
        self.board.to_string()
    }

    pub fn reset(&mut self) {
        self.board = Board::default();
    }

    pub fn move_piece(&mut self, tandem_move: &TandemMove) -> bool {
        let player_color = tandem_move.color(); 

        if !(self.board.side_to_move() == Color::White && player_color == 'W' 
        || self.board.side_to_move() == Color::Black && player_color == 'B') {
            return false;
        }

        let target = match Square::from_string(tandem_move.target.clone()) {
            Some(v) => v,
            None => return false,
        };

        println!("{:?}", tandem_move);

        if tandem_move.source == "spare" {
            let _ = match self.board.piece_on(target) {
                Some(_) => return false,
                None => (),
            };

            let chars = tandem_move.piece.as_bytes();

            if chars.len() != 2 {
                return false;
            }

            let color = match chars[0] as char {
                'w' => Color::White,
                'b' => Color::Black,
                _ => return false,
            };

            let piece = match chars[1] as char {
                'P' => Piece::Pawn,
                'N' => Piece::Knight,
                'B' => Piece::Bishop,
                'R' => Piece::Rook,
                'Q' => Piece::Queen,
                _ => return false,
            };

            if piece == Piece::Pawn {
                if target.get_rank() == Rank::First 
                || target.get_rank() == Rank::Eighth {
                    return false;
                }
            }

            self.board = match set_piece_on_board(&self.board, piece, color, target) {
                Some(v) => v,
                None => return false,
            };

            self.board = match self.board.null_move() {
                Some(v) => v,
                None => return true,
            };

            return true;
        }

        let source = match Square::from_string(tandem_move.source.clone()) {
            Some(v) => v,
            None => return false,
        };

        let chess_move = ChessMove::new(source, target, None);

        if !self.board.legal(chess_move) {
            return false;
        }

        println!("{:?} {:?}", source, target);

        self.board = self.board.make_move_new(chess_move);

        true
    }
}

fn set_piece_on_board(board: &Board, piece: Piece, color: Color, target: Square) -> Option<Board> {
    match board.set_piece(piece, color, target) {
        Some(v) => return Some(v),
        None => ()
    };

    let mut board_new = match board.null_move() {
        Some(v) => v,
        None => return None,
    };

    let board_new = match board_new.set_piece(piece, color, target) {
        Some(v) => v,
        None => return None,
    };

    let target_x = target.get_rank() as i32;
    let target_y = target.get_file() as i32;

    let square_king = match color {
        Color::White => board_new.king_square(Color::Black),
        _ => board_new.king_square(Color::White),
    };

    let king_x = square_king.get_rank() as i32;
    let king_y = square_king.get_file() as i32;

    let close_chess = (king_x - target_x).abs().min((king_y - target_y).abs()) <= 1;

    if board_new.status() == BoardStatus::Checkmate 
    && (close_chess || piece == Piece::Knight) {
        return None;
    }

    Some(board_new)
}

#[derive(Clone)]
pub struct ChessBoardInterface {
    board: Arc<RwLock<ChessBoard>>,
}

impl ChessBoardInterface {
    pub fn new() -> Self {
        ChessBoardInterface {
            board: Arc::new(RwLock::new(ChessBoard::new())),
        }
    }

    pub fn get_fen(&self) -> String {
        self.board.read().unwrap().get_fen()
    }

    pub fn reset(&self) {
        self.board.write().unwrap().reset();
    }

    pub fn move_piece(&self, tandem_move: &TandemMove) -> bool {
        self.board.write().unwrap().move_piece(tandem_move)
    }
}

#[derive(Debug)]
pub struct TandemMove {
    pub player: String,
    pub source: String,
    pub target: String,
    pub piece: String,
}

impl TandemMove {
    pub fn from_string(tandem_string: String) -> Option<Self> {
        let splitted = tandem_string.split(';').collect::<Vec<&str>>();

        if splitted.len() != 4 {
            return None;
        }

        Some(TandemMove {
            player: splitted[0].to_owned(),
            source: splitted[1].to_owned(),
            target: splitted[2].to_owned(),
            piece: splitted[3].to_owned(),
        })
    }

    pub fn color(&self) -> char {
        self.player.chars().next().unwrap_or(' ')
    }
}

pub fn start_server() {
    thread::spawn(|| {
        let server = TcpListener::bind("0.0.0.0:9091").unwrap();
        let board_og = ChessBoardInterface::new();
        let client_map = Arc::new(RwLock::new(HashMap::<usize, MessageQueue<String>>::new()));
        let mut i = 0;

        for stream in server.incoming() {
            let board = board_og.clone();
            let id = i;
            let client_map_c = client_map.clone();
            i += 1;

            thread::spawn(move || {
                let stream_read = stream.unwrap();
                let send_stream = stream_read.try_clone().unwrap();

                let mut websocket_read = accept(stream_read).unwrap();
                let msg_queue = MessageQueue::<String>::new();
                let msg_queue_c = msg_queue.clone();
                let mut websocket_send = WebSocket::from_raw_socket(send_stream, Role::Server, None);

                thread::spawn(move || {
                    loop {
                        let msg = msg_queue_c.consume_blocking();

                        websocket_send.send(Message::Text(msg.into()));
                    }
                });

                msg_queue.produce(board.get_fen());
                client_map_c.write().unwrap().insert(id, msg_queue.clone());

                loop {
                    let msg:String = match websocket_read.read() {
                        Ok(message) => match message {
                            msg @ Message::Text(_) => msg.to_string(),
                            _msg @ Message::Ping(_) | _msg @ Message::Pong(_) => continue,
                            _ => break,
                        },
                        Err(e) => break,
                    };

                    if msg == "Reset Game" {
                        board.reset();

                        for client in client_map_c.read().unwrap().values() {
                            client.produce(board.get_fen());
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
                            client.produce(board.get_fen());
                        }
                    } else {
                        msg_queue.produce(board.get_fen());
                    }
                }
            });
        }
    });
}