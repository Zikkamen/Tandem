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

use serde_json::json;

use chess::{Board, Square, ChessMove, Piece, Color, Rank, BoardStatus};
use crate::game_server::message_queue::MessageQueue;

pub struct ChessBoard {
    pub boards: [Board; 2],
}

impl ChessBoard {
    pub fn new() -> Self {
        ChessBoard {
            boards: [Board::default(), Board::default()],
        }
    }

    pub fn get_fen(&self) -> String {
        json!({
            "board_1": self.boards[0].to_string(),
            "board_2": self.boards[1].to_string(),
        }).to_string()
    }

    pub fn reset(&mut self) {
        for i in 0..2 {
            self.boards[i] = Board::default();
        }
    }

    pub fn move_piece(&mut self, tandem_move: &TandemMove) -> bool {
        println!("{:?}", tandem_move);
        let b_ind = (tandem_move.board - 1) as usize;
        let o_ind = (b_ind + 1) % 2;

        if self.boards[b_ind].side_to_move() != tandem_move.color {
            return false;
        }

        let target = match Square::from_string(tandem_move.target.clone()) {
            Some(v) => v,
            None => return false,
        };

        if tandem_move.source == "spare" {
            let _ = match self.boards[b_ind].piece_on(target) {
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

            self.boards[b_ind] = match set_piece_on_board(&self.boards[b_ind], piece, color, target) {
                Some(v) => v,
                None => return false,
            };

            return true;
        }

        let source = match Square::from_string(tandem_move.source.clone()) {
            Some(v) => v,
            None => return false,
        };
        let piece_source = match self.boards[o_ind].piece_on(source) {
            Some(v) => v,
            None => return false,
        };
        let rank = target.get_rank() as u8;
        let is_promotion = piece_source == Piece::Pawn && (rank == 0 || rank == 7);

        let promotion_target_op = Square::from_string(tandem_move.promotion.clone());
        let mut promotion_piece_op = None;

        if is_promotion {
            match promotion_target_op {
                Some(v) =>  {
                    promotion_piece_op = self.boards[o_ind].piece_on(v);

                    match self.boards[o_ind].color_on(v) {
                        Some(v) => {
                            if v != tandem_move.color {
                                return false;
                            }
                        },
                        None => return false,
                    };
                },
                None => (),
            };
        }

        let chess_move = ChessMove::new(source, target, promotion_piece_op);

        if !self.boards[b_ind].legal(chess_move) {
            return false;
        }

        if is_promotion {
            let promotion_target = match promotion_target_op {
                Some(v) => v,
                None => return false,
            };

            println!("Checking Promotion valid");

            let board_other = match self.boards[o_ind].clear_square(promotion_target) {
                Some(v) => v,
                None => return false,
            };

            match board_other.null_move() {
                Some(_) => (),
                None => return false,
            };

            self.boards[o_ind] = board_other;
        }

        println!("{:?} {:?}", source, target);
        self.boards[b_ind] = self.boards[b_ind].make_move_new(chess_move);

        true
    }
}

fn set_piece_on_board(board: &Board, piece: Piece, color: Color, target: Square) -> Option<Board> {
    let target_x = target.get_rank() as i32;

    if (target_x == 7 || target_x == 0) && piece == Piece::Pawn {
        return None;
    }
    
    match board.set_piece(piece, color, target) {
        Some(v) => return v.null_move(),
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
    pub board: u8,
    pub color: Color,
    pub source: String,
    pub target: String,
    pub piece: String,
    pub promotion: String,
}

impl TandemMove {
    pub fn from_string(tandem_string: String) -> Option<Self> {
        let splitted = tandem_string.split(';').collect::<Vec<&str>>();

        if splitted.len() != 6 {
            return None;
        }

        let color = match splitted[1] {
            "W" => Color::White,
            _ => Color::Black,
        };

        let board = splitted[0].parse::<u8>().unwrap_or(1);

        Some(TandemMove {
            board: board,
            color: color,
            source: splitted[2].to_owned(),
            target: splitted[3].to_owned(),
            piece: splitted[4].to_owned(),
            promotion: splitted[5].to_owned(),
        })
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