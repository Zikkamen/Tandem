use chess::{Board, Piece, Color};

use serde_json::json;

static FIVE_MINUTES:i64 = 5 * 60 * 1000;

pub struct ChessGame {
    pub board: Board,
    pub white_sp: [i32; 5],
    pub black_sp: [i32; 5],
    white_time: i64,
    black_time: i64,
    turn: Color,
    last_move_capture: bool,
    last_time_sum: i64,
    last_move: String,
}

impl ChessGame {
    pub fn new() -> Self {
        ChessGame {
            board: Board::default(),
            white_sp: [0; 5],
            black_sp: [0; 5],
            white_time: FIVE_MINUTES,
            black_time: FIVE_MINUTES,
            turn: Color::White,
            last_move_capture: false,
            last_time_sum: 0,
            last_move: String::new(),
        }
    }

    pub fn last_move_capture(&mut self, capture: bool) {
        self.last_move_capture = capture;
    }

    pub fn flagged(&self) -> bool {
        self.white_time == 0 || self.black_time == 0
    }

    pub fn should_update(&mut self) -> bool {
        let old_time_sum = self.last_time_sum;
        self.last_time_sum = (self.white_time + 999) / 1_000 
            + (self.black_time + 999) / 1_000;

        old_time_sum != self.last_time_sum
    }

    pub fn synchronize_time(&mut self, time_diff: i64) {
        match self.turn {
            Color::White => self.white_time -= time_diff,
            _ => self.black_time -= time_diff,
        };

        self.white_time = self.white_time.max(0);
        self.black_time = self.black_time.max(0);
    }

    pub fn change_turn(&mut self, chess_move: String) {
        self.turn = match self.turn {
            Color::White => Color::Black,
            _ => Color::White,
        };

        self.last_move = chess_move;
        let _ = self.should_update();
    }

    pub fn to_string(&self) -> String {
        let time_white_seconds = (self.white_time + 999) / 1000;
        let time_black_seconds = (self.black_time + 999) / 1000;

        json!({
            "fen": self.board.to_string(),
            "last_move_capture": self.last_move_capture,
            "white_sp": self.white_sp,
            "black_sp": self.black_sp,
            "white_time": format!("{}:{:02}", time_white_seconds / 60, time_white_seconds % 60),
            "black_time": format!("{}:{:02}", time_black_seconds / 60, time_black_seconds % 60),
            "last_move": self.last_move,
        }).to_string()
    }

    pub fn add_piece(&mut self, color: &Color, piece: Piece) {
        let sp_array = match color {
            Color::Black => &mut self.white_sp,
            _ => &mut self.black_sp,
        };

        let i = match piece {
            Piece::Queen => 0,
            Piece::Rook => 1,
            Piece::Bishop => 2,
            Piece::Knight => 3,
            Piece::Pawn => 4,
            _ => return,
        };

        sp_array[i] += 1;
    }

    pub fn add_pawn(&mut self, color: &Color) {
        let sp_array = match color {
            Color::White => &mut self.white_sp,
            _ => &mut self.black_sp,
        };

        sp_array[4] += 1;
    }

    pub fn decrease_count(&mut self, color: &Color, piece: Piece) -> bool {
        let sp_array = match color {
            Color::White => &mut self.white_sp,
            _ => &mut self.black_sp,
        };

        let i = match piece {
            Piece::Queen => 0,
            Piece::Rook => 1,
            Piece::Bishop => 2,
            Piece::Knight => 3,
            Piece::Pawn => 4,
            _ => return false,
        };

        if sp_array[i] <= 0 {
            return false;
        }

        sp_array[i] -= 1;

        true
    }
}