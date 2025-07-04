use std::{
    sync::{Arc, RwLock},
};
use chess::{Board, Square, ChessMove, Piece, Color, Rank, BoardStatus, BoardBuilder};

use serde_json::json;
use chrono::Utc;

use crate::game_server::chess_game::ChessGame;

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

        let board = splitted[0].parse::<u8>().unwrap_or(0);

        if board != 1 && board != 2 {
            return None;
        }

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

pub struct TandemGame {
    pub games: [ChessGame; 2],
    started: bool,
    finished: bool,
    last_sync: i64,
}

impl TandemGame {
    pub fn new() -> Self {
        TandemGame {
            games: [ChessGame::new(), ChessGame::new()],
            finished: false,
            started: false,
            last_sync: 0,
        }
    }

    pub fn get_fen(&self, valid: bool) -> String {
        json!({
            "valid": valid,
            "board_1": self.games[0].to_string(),
            "board_2": self.games[1].to_string(),
        }).to_string()
    }

    pub fn should_update(&mut self) -> bool {
        if self.finished {
            return false;
        }

        self.synchronize_time();

        self.games[0].should_update() 
        || self.games[1].should_update()
    }

    pub fn reset(&mut self) {
        for i in 0..2 {
            self.games[i] = ChessGame::new();
        }

        self.started = false;
        self.finished = false;
        self.last_sync = 0;
    }

    pub fn synchronize_time(&mut self) {
        if !self.started {
            return;
        }

        let now = Utc::now().timestamp_millis();

        if self.last_sync == 0 {
            self.last_sync = now;
        }

        let time_dif = (now - self.last_sync).max(0);
        self.last_sync = now;

        for i in 0..2 {
            self.games[i].synchronize_time(time_dif);

            self.finished |= self.games[i].flagged();
        }
    }

    pub fn move_piece(&mut self, tandem_move: &TandemMove) -> bool {
        println!("{:?}", tandem_move);
        self.synchronize_time();

        if self.finished {
            return false;
        }

        if tandem_move.board <= 0 {
            return false;
        }

        let b_ind = (tandem_move.board - 1) as usize;
        let o_ind = (b_ind + 1) % 2;

        let board = self.games[b_ind].board;
        let other_board = self.games[o_ind].board;

        if board.side_to_move() != tandem_move.color {
            return false;
        }

        let target = match Square::from_string(tandem_move.target.clone()) {
            Some(v) => v,
            None => return false,
        };

        match board.piece_on(target) {
            Some(v) => { 
                if v == Piece::King {
                    println!("Tried to capture King");
                    return false;
                }
            },
            None => (),
        };

        if tandem_move.source == "spare" {
            let _ = match board.piece_on(target) {
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

            let board_new = match set_piece_on_board(&board, piece, color, target) {
                Some(v) => v,
                None => return false,
            };

            if !self.games[b_ind].decrease_count(&color, piece) {
                return false;
            }

            self.games[b_ind].board = board_new;
            self.games[b_ind].change_turn(tandem_move.source.clone() + "-" + &tandem_move.target);

            self.started = true;
            return true;
        }

        let source = match Square::from_string(tandem_move.source.clone()) {
            Some(v) => v,
            None => return false,
        };
        let piece_source = match board.piece_on(source) {
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
                    promotion_piece_op = other_board.piece_on(v);

                    match other_board.color_on(v) {
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

        if !board.legal(chess_move) {
            return false;
        }

        if is_promotion {
            let promotion_target = match promotion_target_op {
                Some(v) => v,
                None => return false,
            };

            println!("Checking Promotion valid");

            let board_other = match other_board.clear_square(promotion_target) {
                Some(v) => v,
                None => return false,
            };

            match board_other.null_move() {
                Some(_) => (),
                None => return false,
            };

            self.games[o_ind].board = board_other;
            self.games[o_ind].add_pawn(&tandem_move.color);
        }

        match board.piece_on(target) {
            Some(v) => {
                self.games[o_ind].add_piece(&tandem_move.color, v);
                self.games[b_ind].last_move_capture(true);
            },
            None => self.games[b_ind].last_move_capture(false),
        };

        println!("{:?} {:?}", source, target);
        self.games[b_ind].change_turn(tandem_move.source.clone() + "-" + &tandem_move.target);
        self.games[b_ind].board = board.make_move_new(chess_move);

        if is_mate(&self.games[b_ind].board, piece_source, target, tandem_move.color) {
            self.finished = true;
        }

        self.started = true;
        true
    }
}

fn set_piece_on_board(board: &Board, piece: Piece, color: Color, target: Square) -> Option<Board> {
    let target_x = target.get_rank() as i32;

    if (target_x == 7 || target_x == 0) && piece == Piece::Pawn {
        return None;
    }

    let mut board_builder = BoardBuilder::from(board);
    board_builder.piece(target, piece, color);

    match color {
        Color::White => board_builder.side_to_move(Color::Black),
        _ => board_builder.side_to_move(Color::White),
    };

    let new_board = match Board::try_from(board_builder) {
        Ok(v) => v,
        Err(_) => return None,
    };

    if is_mate(&new_board, piece, target, color) {
        None
    } else {
        Some(new_board)
    }
}

fn is_mate(board: &Board, piece: Piece, target: Square, color: Color) -> bool {
    let target_x = target.get_rank() as i32;
    let target_y = target.get_file() as i32;

    let square_king = match color {
        Color::White => board.king_square(Color::Black),
        _ => board.king_square(Color::White),
    };

    let king_x = square_king.get_rank() as i32;
    let king_y = square_king.get_file() as i32;

    let close_chess = (king_x - target_x).abs().max((king_y - target_y).abs()) <= 1;

    board.status() == BoardStatus::Checkmate && (close_chess || piece == Piece::Knight)
}

#[derive(Clone)]
pub struct TandemGameInterface {
    board: Arc<RwLock<TandemGame>>,
}

impl TandemGameInterface {
    pub fn new() -> Self {
        TandemGameInterface {
            board: Arc::new(RwLock::new(TandemGame::new())),
        }
    }

    pub fn get_fen(&self, valid: bool) -> String {
        self.board.read().unwrap().get_fen(valid)
    }

    pub fn should_update(&self) -> bool {
        self.board.write().unwrap().should_update()
    }

    pub fn reset(&self) {
        self.board.write().unwrap().reset();
    }

    pub fn move_piece(&self, tandem_move: &TandemMove) -> bool {
        self.board.write().unwrap().move_piece(tandem_move)
    }
}
