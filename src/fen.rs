use crate::board::Board;
use crate::types::piece::Piece;
use crate::types::side::Side;
use crate::types::side::Side::{Black, White};
use crate::types::square::Square;
use crate::types::{File, Rank};
use crate::zobrist::Zobrist;

pub const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

impl Board {
    pub fn from_fen(fen: &str) -> Board {
        let mut board = Board::empty();
        let parts: Vec<&str> = fen.split_whitespace().collect();

        let rows: Vec<&str> = parts[0].split('/').collect();
        if rows.len() != 8 {
            panic!("Invalid FEN string");
        }

        for (rank, row) in rows.iter().enumerate() {
            let mut file = 0;
            for ch in row.chars() {
                match ch {
                    '1'..='8' => {
                        file += ch.to_digit(10).unwrap() as usize;
                    }
                    'P' | 'N' | 'B' | 'R' | 'Q' | 'K' | 'p' | 'n' | 'b' | 'r' | 'q' | 'k' => {
                        let sq = Square::from(File::parse(file), Rank::parse(7 - rank));
                        let piece = parse_piece(ch);
                        let side = if ch.is_uppercase() { White } else { Black };
                        board.toggle_sq(sq, piece, side);
                        file += 1;
                    }
                    _ => panic!("Invalid character in FEN string"),
                }
            }
        }

        board.stm = parse_stm(parts[1]);
        board.castle = parse_castle_rights(parts[2]);
        board.ep_sq = parse_ep_sq(parts[3]);
        board.hm = parts.get(4).unwrap_or(&"0").parse().unwrap_or(0);
        board.fm = parts.get(5).unwrap_or(&"0").parse().unwrap_or(0);
        board.hash = Zobrist::get_hash(&board);
        board.pawn_hash = Zobrist::get_pawn_hash(&board);
        board.non_pawn_hashes = Zobrist::get_non_pawn_hashes(&board);
        board.major_hash = Zobrist::get_major_hash(&board);
        board.minor_hash = Zobrist::get_minor_hash(&board);
        board
    }

    pub fn to_fen(self) -> String {
        let mut fen = String::new();

        for rank in (0..8).rev() {
            let mut empty_squares = 0;
            for file in 0..8 {
                let sq = Square::from(File::parse(file), Rank::parse(rank));
                match self.piece_at(sq) {
                    Some(piece) => {
                        if empty_squares > 0 {
                            fen.push_str(&empty_squares.to_string());
                            empty_squares = 0;
                        }
                        fen.push(piece_to_char(
                            piece,
                            self.side_at(sq).expect("Square should be occupied"),
                        ));
                    }
                    None => {
                        empty_squares += 1;
                    }
                }
            }
            if empty_squares > 0 {
                fen.push_str(&empty_squares.to_string());
            }
            if rank > 0 {
                fen.push('/');
            }
        }

        fen.push(' ');
        fen.push(if self.stm == White { 'w' } else { 'b' });

        fen.push(' ');
        if self.castle & 0b0001 != 0 {
            fen.push('K');
        }
        if self.castle & 0b0010 != 0 {
            fen.push('Q');
        }
        if self.castle & 0b0100 != 0 {
            fen.push('k');
        }
        if self.castle & 0b1000 != 0 {
            fen.push('q');
        }
        if self.castle == 0 {
            fen.push('-');
        }

        fen.push(' ');
        if let Some(ep_sq) = self.ep_sq {
            fen.push((b'a' + (ep_sq.0 % 8)) as char);
            fen.push((b'1' + (ep_sq.0 / 8)) as char);
        } else {
            fen.push('-');
        }

        fen.push(' ');
        fen.push_str(&self.hm.to_string());
        fen.push(' ');
        fen.push_str(&self.fm.to_string());
        fen
    }
}

fn parse_castle_rights(castle: &str) -> u8 {
    let mut rights = 0;
    for c in castle.chars() {
        match c {
            'K' => rights |= 0b0001,
            'Q' => rights |= 0b0010,
            'k' => rights |= 0b0100,
            'q' => rights |= 0b1000,
            '-' => (),
            _ => panic!("Invalid character in castle rights"),
        }
    }
    rights
}

fn parse_ep_sq(ep_sq: &str) -> Option<Square> {
    if ep_sq == "-" {
        None
    } else {
        Some(parse_square(ep_sq))
    }
}

fn parse_stm(stm: &str) -> Side {
    match stm {
        "w" => White,
        "b" => Black,
        _ => panic!("Invalid side to move in FEN string"),
    }
}

fn parse_piece(c: char) -> Piece {
    match c.to_uppercase().next().unwrap() {
        'P' => Piece::Pawn,
        'N' => Piece::Knight,
        'B' => Piece::Bishop,
        'R' => Piece::Rook,
        'Q' => Piece::Queen,
        'K' => Piece::King,
        _ => panic!("Invalid piece character"),
    }
}

fn parse_square(s: &str) -> Square {
    let file = s.chars().next().unwrap() as usize - 'a' as usize;
    let rank = s.chars().nth(1).unwrap() as usize - '1' as usize;
    Square::from(File::parse(file), Rank::parse(rank))
}

fn piece_to_char(piece: Piece, side: Side) -> char {
    let ch = match piece {
        Piece::Pawn => 'p',
        Piece::Knight => 'n',
        Piece::Bishop => 'b',
        Piece::Rook => 'r',
        Piece::Queen => 'q',
        Piece::King => 'k',
    };
    if side == White {
        ch.to_ascii_uppercase()
    } else {
        ch
    }
}
