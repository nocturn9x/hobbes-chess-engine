use crate::movegen::{is_attacked, is_check};
use crate::types::bitboard::Bitboard;
use crate::types::piece::Piece;
use crate::types::piece::Piece::{King, Pawn};
use crate::types::side::Side;
use crate::types::side::Side::{Black, White};
use crate::types::square::Square;
use crate::types::{File, Rank};
use crate::zobrist::Zobrist;
use crate::{attacks, fen};
use crate::{moves::Move, moves::MoveFlag};

#[derive(Clone, Copy)]
pub struct Board {
    pub bb: [Bitboard; 8],         // bitboards for each piece type (0-5) and for both colours (6-7)
    pub pcs: [Option<Piece>; 64],  // piece type on each square
    pub stm: Side,                 // side to move (White or Black)
    pub hm: u8,                    // number of half moves since last capture or pawn move
    pub fm: u8,                    // number of full moves
    pub ep_sq: Option<Square>,     // en passant square (0-63)
    pub castle: u8,                // encoded castle rights
    pub hash: u64,                 // Zobrist hash
    pub pawn_hash: u64,            // Zobrist hash for pawns
    pub non_pawn_hashes: [u64; 2], // Zobrist hashes for non-pawns
    pub major_hash: u64,           // Zobrist hash for major pieces
    pub minor_hash: u64,           // Zobrist hash for minor pieces
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {

    pub fn new() -> Board {
        Board::from_fen(fen::STARTPOS)
    }

    pub fn empty() -> Board {
        Board {
            bb: [Bitboard::empty(); 8],
            pcs: [None; 64],
            stm: White,
            hm: 0,
            fm: 0,
            ep_sq: None,
            castle: 0,
            hash: 0,
            pawn_hash: 0,
            non_pawn_hashes: [0, 0],
            major_hash: 0,
            minor_hash: 0,
        }
    }

    pub fn make(&mut self, m: &Move) {

        let side = self.stm;
        let (from, to, flag) = (m.from(), m.to(), m.flag());
        let pc = self.piece_at(from).unwrap();
        let new_pc = if let Some(promo) = m.promo_piece() { promo } else { pc };
        let captured = if flag == MoveFlag::EnPassant { Some(Piece::Pawn) } else { self.pcs[to] };

        self.toggle_sq(from, pc, side);
        if let Some(captured) = captured {
            let capture_sq = if flag == MoveFlag::EnPassant { self.ep_capture_sq(to) } else { to };
            self.toggle_sq(capture_sq, captured, side.flip());
        }
        self.toggle_sq(to, new_pc, side);

        if m.is_castle() {
            let (rook_from, rook_to) = self.rook_sqs(to);
            self.toggle_sqs(rook_from, rook_to, Piece::Rook, side);
        }

        self.ep_sq = self.calc_ep(flag, to);
        self.castle = self.calc_castle_rights(from, to, pc);
        self.fm += if side == Black { 1 } else { 0 };
        self.hm = if captured.is_some() || pc == Piece::Pawn { 0 } else { self.hm + 1 };
        self.hash ^= Zobrist::stm();
        self.stm = self.stm.flip();

    }

    #[inline]
    pub fn toggle_sq(&mut self, sq: Square, pc: Piece, side: Side) {
        let bb: Bitboard = Bitboard::of_sq(sq);
        self.bb[pc] ^= bb;
        self.bb[side.idx()] ^= bb;
        self.pcs[sq] = if self.pcs[sq] == Some(pc) { None } else { Some(pc) };
        self.hash ^= Zobrist::sq(pc, side, sq);
        if pc == Pawn {
            self.pawn_hash ^= Zobrist::sq(Pawn, side, sq);
        } else {
            self.non_pawn_hashes[side] ^= Zobrist::sq(pc, side, sq);
            if pc.is_major() {
                self.major_hash ^= Zobrist::sq(pc, side, sq);
            }
            if pc.is_minor() {
                self.minor_hash ^= Zobrist::sq(pc, side, sq);
            }
        }
    }

    #[inline]
    pub fn toggle_sqs(&mut self, from: Square, to: Square, piece: Piece, side: Side) {
        self.toggle_sq(from, piece, side);
        self.toggle_sq(to, piece, side);
    }

    #[inline]
    fn rook_sqs(self, king_to_sq: Square) -> (Square, Square) {
        match king_to_sq.0 {
            2 => (Square(0), Square(3)),
            6 => (Square(7), Square(5)),
            58 => (Square(56), Square(59)),
            62 => (Square(63), Square(61)),
            _ => unreachable!()
        }
    }

    #[inline]
    fn ep_capture_sq(&self, to: Square) -> Square {
        if self.stm == White { Square(to.0 - 8) } else { Square(to.0 + 8) }
    }

    #[inline]
    fn calc_castle_rights(&mut self, from: Square, to: Square, piece_type: Piece) -> u8 {
        let original_rights = self.castle;
        let mut new_rights = self.castle;
        if new_rights == Rights::None as u8 {
            // Both sides already lost castling rights, so nothing to calculate.
            return new_rights;
        }
        // Any move by the king removes castling rights.
        if piece_type == Piece::King {
            new_rights &= if self.stm == White { Rights::Black as u8 } else { Rights::White as u8 };
        }
        // Any move starting from/ending at a rook square removes castling rights for that corner.
        if from.0 == 7 || to.0 == 7    { new_rights &= !(Rights::WKS as u8); }
        if from.0 == 63 || to.0 == 63  { new_rights &= !(Rights::BKS as u8); }
        if from.0 == 0 || to.0 == 0    { new_rights &= !(Rights::WQS as u8); }
        if from.0 == 56 || to.0 == 56  { new_rights &= !(Rights::BQS as u8); }
        self.hash ^= Zobrist::castle(original_rights) ^ Zobrist::castle(new_rights);
        new_rights
    }

    #[inline]
    fn calc_ep(&mut self, flag: MoveFlag, sq: Square) -> Option<Square>{
        if self.ep_sq.is_some() {
            self.hash ^= Zobrist::ep(self.ep_sq.unwrap());
        }
        let ep_sq = if flag == MoveFlag::DoublePush { Some(self.ep_capture_sq(sq)) } else { None };
        if ep_sq.is_some() {
            self.hash ^= Zobrist::ep(ep_sq.unwrap());
        }
        ep_sq
    }

    pub fn has_kingside_rights(&self, side: Side) -> bool {
        if side == White {
            self.castle & Rights::WKS as u8 != 0
        } else {
            self.castle & Rights::BKS as u8 != 0
        }
    }

    pub fn has_queenside_rights(&self, side: Side) -> bool {
        if side == White {
            self.castle & Rights::WQS as u8 != 0
        } else {
            self.castle & Rights::BQS as u8 != 0
        }
    }

    pub fn make_null_move(&mut self) {
        self.hm = 0;
        self.stm = self.stm.flip();
        self.hash ^= Zobrist::stm();
        if let Some(ep_sq) = self.ep_sq {
            self.hash ^= Zobrist::ep(ep_sq);
            self.ep_sq = None;
        }
    }

    pub fn pawns(self, side: Side) -> Bitboard {
        self.bb[Piece::Pawn] & self.bb[side.idx()]
    }

    pub fn knights(self, side: Side) -> Bitboard {
        self.bb[Piece::Knight] & self.bb[side.idx()]
    }

    pub fn bishops(self, side: Side) -> Bitboard {
        self.bb[Piece::Bishop] & self.bb[side.idx()]
    }

    pub fn rooks(self, side: Side) -> Bitboard {
        self.bb[Piece::Rook] & self.bb[side.idx()]
    }

    pub fn queens(self, side: Side) -> Bitboard {
        self.bb[Piece::Queen] & self.bb[side.idx()]
    }

    pub fn king(self, side: Side) -> Bitboard {
        self.bb[Piece::King] & self.bb[side.idx()]
    }

    pub fn king_sq(self, side: Side) -> Square {
        self.king(side).lsb()
    }

    pub fn occ(self) -> Bitboard {
        self.bb[White.idx()] | self.bb[Black.idx()]
    }

    pub fn pcs(self, piece: Piece) -> Bitboard {
        self.bb[piece]
    }

    pub fn side(self, side: Side) -> Bitboard {
        self.bb[side.idx()]
    }

    pub fn white(self) -> Bitboard {
        self.bb[White.idx()]
    }

    pub fn black(self) -> Bitboard {
        self.bb[Black.idx()]
    }

    pub fn us(self) -> Bitboard {
        self.bb[self.stm.idx()]
    }

    pub fn them(self) -> Bitboard {
        self.bb[self.stm.flip().idx()]
    }

    pub fn our(self, piece: Piece) -> Bitboard {
        self.bb[piece] & self.bb[self.stm.idx()]
    }

    pub fn their(self, piece: Piece) -> Bitboard {
        self.bb[piece] & self.bb[self.stm.flip().idx()]
    }

    pub fn piece_at(self, sq: Square) -> Option<Piece> {
        self.pcs[sq]
    }

    pub fn captured(self, mv: &Move) -> Option<Piece> {
        if mv.is_castle() { return None; }
        if mv.is_ep() { return Some(Piece::Pawn); }
        self.piece_at(mv.to())
    }

    pub fn is_noisy(self, mv: &Move) -> bool {
        mv.is_promo() || self.captured(mv).is_some()
    }

    pub fn side_at(self, sq: Square) -> Option<Side> {
        if !(self.bb[White.idx()] & Bitboard::of_sq(sq)).is_empty() { Some(White) }
        else if !(self.bb[Black.idx()] & Bitboard::of_sq(sq)).is_empty() { Some(Black) }
        else { None }
    }

    pub fn has_non_pawns(self) -> bool {
        self.our(Piece::King) | self.our(Piece::Pawn) != self.us()
    }

    pub fn is_fifty_move_rule(self) -> bool {
        self.hm >= 100
    }

    pub fn is_insufficient_material(&self) -> bool {
        let pawns    = self.bb[Piece::Pawn];
        let knights  = self.bb[Piece::Knight];
        let bishops  = self.bb[Piece::Bishop];
        let rooks    = self.bb[Piece::Rook];
        let queens   = self.bb[Piece::Queen];

        if !(pawns | rooks | queens).is_empty() {
            return false;
        }

        let minor_pieces = knights | bishops;
        let piece_count = minor_pieces.count();
        if piece_count <= 1 {
            return true;
        }

        if knights.is_empty() && !bishops.is_empty()
            && (bishops & self.white()).count() == 2 || (bishops & self.black()).count() == 2 {
            return false;
        }
        piece_count <= 3
    }

    pub fn is_pseudo_legal(&self, mv: &Move) -> bool {

        if !mv.exists() {
            return false;
        }

        let from = mv.from();
        let to = mv.to();

        if from == to {
            // Cannot move to the same square
            return false;
        }

        let pc = self.piece_at(from);
        let us = self.us();
        let them = self.them();
        let occ = us | them;
        let captured = self.captured(mv);

        // Can't move without a piece
        if pc.is_none() {
            return false;
        }

        let pc = pc.unwrap();

        // Cannot move a piece that is not ours
        if !self.us().contains(from) {
            return false;
        }

        // Cannot capture our own piece
        if us.contains(to) {
            return false;
        }

        if let Some(captured) = captured {

            // Cannot capture a king
            if captured == King {
                return false;
            }

        }

        if mv.is_castle() {

            // Can only castle with the king
            if pc != Piece::King {
                return false;
            }

            let rank = if self.stm == White { Rank::One } else { Rank::Eight };
            let rank_bb = rank.to_bb();
            if !rank_bb.contains(from) || !rank_bb.contains(to) {
                // Castling must be on the first rank
                return false;
            }

            let kingside_sq = if self.stm == White { Square(6) } else { Square(62) };
            let queenside_sq = if self.stm == White { Square(2) } else { Square(58) };

            // Castling must be to the kingside or queenside square
            if to != kingside_sq && to != queenside_sq {
                return false;
            }

            // Cannot castle kingside if no rights
            if to == kingside_sq && !self.has_kingside_rights(self.stm) {
                return false;
            }

            // Cannot castle queenside if no rights
            if to == queenside_sq && !self.has_queenside_rights(self.stm) {
                return false;
            }

            let kingside = to == kingside_sq;
            let travel_sqs = if kingside {
                if self.stm == White { CastleTravel::WKS } else { CastleTravel::BKS }
            } else {
                if self.stm == White { CastleTravel::WQS } else { CastleTravel::BQS }
            };

            // Cannot castle through occupied squares
            if !(occ & travel_sqs).is_empty() {
                return false;
            }

            let safety_sqs = if kingside {
                if self.stm == White { CastleSafety::WKS } else { CastleSafety::BKS }
            } else {
                if self.stm == White { CastleSafety::WQS } else { CastleSafety::BQS }
            };

            // Cannot castle through check
            if is_attacked(safety_sqs, self.stm, occ, self) {
                return false;
            }

        }

        if pc == Pawn {

            if mv.is_ep() {
                // Cannot en passant if no en passant square
                if self.ep_sq.is_none() {
                    return false;
                }

                let ep_capture_sq = self.ep_capture_sq(to);

                // Cannot en passant if no pawn on the capture square
                if !them.contains(ep_capture_sq) {
                    return false;
                }
            }

            let from_rank = Rank::of(from);
            let to_rank = Rank::of(to);

            // Cannot move a pawn backwards
            if (self.stm == White && to_rank < from_rank) || (self.stm == Black && to_rank > from_rank) {
                return false;
            }

            let promo_rank = if self.stm == White { Rank::Eight } else { Rank::One };

            // Cannot promote a pawn if not to the promotion rank
            if mv.is_promo() && !promo_rank.to_bb().contains(to) {
                return false;
            }

            let from_file = File::of(from);
            let to_file = File::of(to);

            if from_file != to_file {

                // Must capture on an adjacent file
                if to_file as usize != from_file as usize + 1
                    && to_file as usize != from_file as usize - 1 {
                    return false;
                }

                // Must be capturing a piece
                captured.is_some() || mv.is_ep()

            } else {

                // Cannot capture a piece with a pawn push
                if captured.is_some() {
                    return false;
                }

                if mv.is_double_push() {

                    let start_rank = if self.stm == White { Rank::Two } else { Rank::Seven };
                    // Cannot double push a pawn if not on the starting rank
                    if !start_rank.to_bb().contains(from) {
                        return false;
                    }

                    let between_sq = if self.stm == White { Square(from.0 + 8) } else { Square(from.0 - 8) };
                    // Cannot double push a pawn if the square in between is occupied
                    if occ.contains(between_sq) {
                        return false;
                    }

                    // Cannot double push to an occupied square
                    !occ.contains(to)

                } else {
                    // Must be a single push
                    if to.0 != if self.stm == White { from.0 + 8 } else { from.0 - 8 } {
                        return false;
                    }

                    !occ.contains(to)
                }
            }

        } else {
            // Can't make a pawn-specific move with a non-pawn
            if mv.is_ep() || mv.is_promo() || mv.is_double_push() {
                return false;
            }

            let attacks = attacks::attacks(from, pc, self.stm, occ);
            attacks.contains(to)
        }
    }

    pub fn is_legal(&self, mv: &Move) -> bool {
        let mut new_board = *self;
        new_board.make(mv);
        !is_check(&new_board, self.stm)
    }

}

pub enum Rights {
    None = 0b0000,
    WKS = 0b0001,
    WQS = 0b0010,
    BKS = 0b0100,
    BQS = 0b1000,
    White = 0b0011,
    Black = 0b1100,
}

// Squares that must not be attacked when the king castles
pub struct CastleSafety;

impl CastleSafety {
    pub const WQS: Bitboard = Bitboard(0x000000000000001C);
    pub const WKS: Bitboard = Bitboard(0x0000000000000070);
    pub const BQS: Bitboard = Bitboard(0x1C00000000000000);
    pub const BKS: Bitboard = Bitboard(0x7000000000000000);
}

// Squares that must be unoccupied when the king castles
pub struct CastleTravel;

impl CastleTravel {
    pub const WKS: Bitboard = Bitboard(0x0000000000000060);
    pub const WQS: Bitboard = Bitboard(0x000000000000000E);
    pub const BKS: Bitboard = Bitboard(0x6000000000000000);
    pub const BQS: Bitboard = Bitboard(0x0E00000000000000);
}

#[cfg(test)]
mod tests {
    use crate::board::Board;
    use crate::moves::{Move, MoveFlag};

    #[test]
    fn standard_move() {
        assert_make_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                         "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1",
                         Move::parse_uci("g1f3"));
    }

    #[test]
    fn capture_move() {
        assert_make_move("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2",
                         "rnbqkbnr/ppp1pppp/8/3P4/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 2",
                         Move::parse_uci("e4d5"));
    }

    #[test]
    fn double_push() {
        assert_make_move("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
                         "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2",
                         Move::parse_uci_with_flag("c7c5", MoveFlag::DoublePush));
    }

    #[test]
    fn en_passant() {
        assert_make_move("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
                         "rnbqkbnr/ppp1p1pp/5P2/3p4/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 3",
                         Move::parse_uci_with_flag("e5f6", MoveFlag::EnPassant));
    }

    #[test]
    fn castle_kingside_white() {
        assert_make_move("r1bqk1nr/pppp1ppp/2n5/1Bb1p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
                         "r1bqk1nr/pppp1ppp/2n5/1Bb1p3/4P3/5N2/PPPP1PPP/RNBQ1RK1 b kq - 5 4",
                         Move::parse_uci_with_flag("e1g1", MoveFlag::CastleK));
    }

    #[test]
    fn castle_kingside_black() {
        assert_make_move("rnbqk2r/pppp1ppp/5n2/2b1p3/2B1P3/2P2N2/PP1P1PPP/RNBQK2R b KQkq - 0 4",
                         "rnbq1rk1/pppp1ppp/5n2/2b1p3/2B1P3/2P2N2/PP1P1PPP/RNBQK2R w KQ - 1 5",
                         Move::parse_uci_with_flag("e8g8", MoveFlag::CastleQ));
    }

    #[test]
    fn castle_queenside_white() {
        assert_make_move("r3kbnr/pppqpppp/2n5/3p1b2/3P1B2/2N5/PPPQPPPP/R3KBNR w KQkq - 6 5",
                         "r3kbnr/pppqpppp/2n5/3p1b2/3P1B2/2N5/PPPQPPPP/2KR1BNR b kq - 7 5",
                         Move::parse_uci_with_flag("e1c1", MoveFlag::CastleQ));
    }

    #[test]
    fn castle_queenside_black() {
        assert_make_move("r3kbnr/pppqpppp/2n5/3p1b2/8/2N2NP1/PPPPPPBP/R1BQ1K1R b kq - 6 5",
                         "2kr1bnr/pppqpppp/2n5/3p1b2/8/2N2NP1/PPPPPPBP/R1BQ1K1R w - - 7 6",
                         Move::parse_uci_with_flag("e8c8", MoveFlag::CastleQ));
    }

    #[test]
    fn queen_promotion() {
        assert_make_move("rn1q1bnr/pppbkPpp/8/8/8/8/PPPP1PPP/RNBQKBNR w KQ - 1 5",
                         "rn1q1bQr/pppbk1pp/8/8/8/8/PPPP1PPP/RNBQKBNR b KQ - 0 5",
                         Move::parse_uci("f7g8q"));
    }

    #[test]
    fn insufficient_material() {
        assert!(Board::from_fen("8/1k6/2n5/8/8/5N2/6K1/8 w - - 0 1").is_insufficient_material());
        assert!(!Board::from_fen("8/1k6/2np4/8/8/5N2/6K1/8 w - - 0 1").is_insufficient_material());
        assert!(Board::from_fen("8/1k6/2b5/8/8/5B2/6K1/8 w - - 0 1").is_insufficient_material());
        assert!(Board::from_fen("8/1k6/2b5/8/8/5N2/6K1/8 w - - 0 1").is_insufficient_material());
        assert!(Board::from_fen("8/1k6/2bN4/8/8/5N2/6K1/8 w - - 0 1").is_insufficient_material());
        assert!(!Board::from_fen("8/1k6/2bb4/8/8/8/6K1/8 w - - 0 1").is_insufficient_material());
    }

    fn assert_make_move(start_fen: &str, end_fen: &str, m: Move) {
        let mut board = Board::from_fen(start_fen);
        board.make(&m);
        assert_eq!(board.to_fen(), end_fen);
    }

}