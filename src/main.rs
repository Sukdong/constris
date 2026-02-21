use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{self, Color, Stylize},
    terminal::{self, ClearType},
};
use rand::Rng;
use std::io::{self, Write};
use std::time::{Duration, Instant};

// 보드 크기(칸 단위)
const BOARD_W: usize = 10;
const BOARD_H: usize = 20;

// 각 칸은 가로 4문자 × 세로 2줄(터미널에서 정사각형에 가깝게)
const CELL_W: usize = 4;
const CELL_H: usize = 2;

// 보드 그리기 시작 y좌표(테두리 내부)
const BOARD_Y: u16 = 1;

// ── 테트로미노 정의 ─────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum PieceKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

const ALL_PIECES: [PieceKind; 7] = [
    PieceKind::I,
    PieceKind::O,
    PieceKind::T,
    PieceKind::S,
    PieceKind::Z,
    PieceKind::J,
    PieceKind::L,
];

impl PieceKind {
    fn color(self) -> Color {
        match self {
            PieceKind::I => Color::Cyan,
            PieceKind::O => Color::Yellow,
            PieceKind::T => Color::Magenta,
            PieceKind::S => Color::Green,
            PieceKind::Z => Color::Red,
            PieceKind::J => Color::Blue,
            PieceKind::L => Color::DarkYellow,
        }
    }

    /// 회전 상태 0의 셀 좌표(각 피스는 4x4 그리드 기준).
    fn cells(self) -> Vec<(i32, i32)> {
        match self {
            PieceKind::I => vec![(0, 1), (1, 1), (2, 1), (3, 1)],
            PieceKind::O => vec![(1, 0), (2, 0), (1, 1), (2, 1)],
            PieceKind::T => vec![(0, 1), (1, 1), (2, 1), (1, 0)],
            PieceKind::S => vec![(0, 1), (1, 1), (1, 0), (2, 0)],
            PieceKind::Z => vec![(0, 0), (1, 0), (1, 1), (2, 1)],
            PieceKind::J => vec![(0, 0), (0, 1), (1, 1), (2, 1)],
            PieceKind::L => vec![(2, 0), (0, 1), (1, 1), (2, 1)],
        }
    }
}

#[derive(Clone)]
struct Piece {
    kind: PieceKind,
    cells: Vec<(i32, i32)>,
    x: i32,
    y: i32,
}

impl Piece {
    fn new(kind: PieceKind) -> Self {
        let cells = kind.cells();
        Self {
            kind,
            cells,
            // 4x4 기준으로 중앙에 스폰
            x: (BOARD_W as i32 - CELL_W as i32) / 2,
            // 보드 위에서 시작해 자연스럽게 내려오도록 음수 y
            y: -1,
        }
    }

    // 현재 피스의 절대 좌표 목록
    fn absolute_cells(&self) -> Vec<(i32, i32)> {
        self.cells
            .iter()
            .map(|&(cx, cy)| (self.x + cx, self.y + cy))
            .collect()
    }

    // 시계 방향 90도 회전 좌표
    fn rotated_cw(&self) -> Vec<(i32, i32)> {
        if self.kind == PieceKind::O {
            return self.cells.clone();
        }
        let size = if self.kind == PieceKind::I { 4 } else { 3 };
        self.cells
            .iter()
            .map(|&(cx, cy)| (size - 1 - cy, cx))
            .collect()
    }
}

// ── 보드 ────────────────────────────────────────────────────────────

type Cell = Option<Color>;

struct Board {
    grid: [[Cell; BOARD_W]; BOARD_H],
}

impl Board {
    fn new() -> Self {
        Self {
            grid: [[None; BOARD_W]; BOARD_H],
        }
    }

    /// 해당 좌표가 비어 있고 보드 범위를 만족하는지 확인
    fn is_free(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= BOARD_W as i32 {
            return false;
        }
        if y >= BOARD_H as i32 {
            return false;
        }
        // 보드 위쪽(y < 0)은 스폰/회전을 위해 허용
        if y < 0 {
            return true;
        }
        self.grid[y as usize][x as usize].is_none()
    }

    /// 모든 좌표가 유효한 위치인지 확인
    fn fits(&self, cells: &[(i32, i32)]) -> bool {
        cells.iter().all(|&(x, y)| self.is_free(x, y))
    }

    /// 피스를 보드에 고정
    fn lock(&mut self, cells: &[(i32, i32)], color: Color) {
        for &(x, y) in cells {
            if y >= 0 && y < BOARD_H as i32 && x >= 0 && x < BOARD_W as i32 {
                self.grid[y as usize][x as usize] = Some(color);
            }
        }
    }

    /// 가득 찬 줄을 제거하고 제거한 줄 수를 반환.
    fn clear_lines(&mut self) -> u32 {
        let mut cleared = 0u32;
        let mut kept: Vec<[Cell; BOARD_W]> = Vec::new();
        for y in 0..BOARD_H {
            if self.grid[y].iter().all(|c| c.is_some()) {
                cleared += 1;
            } else {
                kept.push(self.grid[y]);
            }
        }
        // 위에 빈 줄을 채우고, 남은 줄은 아래로 내린다(순서 유지)
        let empty_count = BOARD_H - kept.len();
        for y in 0..BOARD_H {
            if y < empty_count {
                self.grid[y] = [None; BOARD_W];
            } else {
                self.grid[y] = kept[y - empty_count];
            }
        }
        cleared
    }
}

// ── 게임 상태 ───────────────────────────────────────────────────────

struct Game {
    board: Board,
    current: Piece,
    next: PieceKind,
    score: u32,
    lines: u32,
    level: u32,
    game_over: bool,
}

impl Game {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        let kind = ALL_PIECES[rng.gen_range(0..ALL_PIECES.len())];
        let next = ALL_PIECES[rng.gen_range(0..ALL_PIECES.len())];
        Self {
            board: Board::new(),
            current: Piece::new(kind),
            next,
            score: 0,
            lines: 0,
            level: 1,
            game_over: false,
        }
    }

    fn spawn_next(&mut self) {
        let mut rng = rand::thread_rng();
        self.current = Piece::new(self.next);
        self.next = ALL_PIECES[rng.gen_range(0..ALL_PIECES.len())];
        // 스폰 위치가 막혀 있으면 게임 오버
        if !self.board.fits(&self.current.absolute_cells()) {
            self.game_over = true;
        }
    }

    /// 이동 시도(성공 시 true)
    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        let mut moved = self.current.clone();
        moved.x += dx;
        moved.y += dy;
        if self.board.fits(&moved.absolute_cells()) {
            self.current = moved;
            true
        } else {
            false
        }
    }

    fn try_rotate(&mut self) -> bool {
        if self.current.kind == PieceKind::O {
            return true;
        }
        let rotated_cells = self.current.rotated_cw();

        // 벽 킥 오프셋 후보
        let kicks: &[(i32, i32)] = if self.current.kind == PieceKind::I {
            &[
                (0, 0),
                (-1, 0),
                (1, 0),
                (-2, 0),
                (2, 0),
                (0, -1),
                (0, -2),
            ]
        } else {
            &[(0, 0), (-1, 0), (1, 0), (0, -1), (-1, -1), (1, -1)]
        };

        for &(kx, ky) in kicks {
            let abs: Vec<(i32, i32)> = rotated_cells
                .iter()
                .map(|&(cx, cy)| (self.current.x + cx + kx, self.current.y + cy + ky))
                .collect();
            if self.board.fits(&abs) {
                self.current.cells = rotated_cells;
                self.current.x += kx;
                self.current.y += ky;
                return true;
            }
        }
        false
    }

    /// 하드 드롭: 내려갈 수 있을 때까지 이동 후 고정
    fn hard_drop(&mut self) {
        while self.try_move(0, 1) {}
        self.lock_and_advance();
    }

    /// 현재 피스를 고정하고 점수/레벨 갱신 후 다음 피스 스폰
    fn lock_and_advance(&mut self) {
        let cells = self.current.absolute_cells();
        let color = self.current.kind.color();
        self.board.lock(&cells, color);

        let cleared = self.board.clear_lines();
        if cleared > 0 {
            self.lines += cleared;
            self.score += match cleared {
                1 => 100 * self.level,
                2 => 300 * self.level,
                3 => 500 * self.level,
                4 => 800 * self.level,
                _ => 0,
            };
            self.level = self.lines / 10 + 1;
        }

        self.spawn_next();
    }

    /// 레벨에 따른 낙하 간격(ms).
    fn drop_interval_ms(&self) -> u64 {
        let base = 1000u64;
        let min_interval = 50u64;
        base.saturating_sub((self.level as u64 - 1) * 80)
            .max(min_interval)
    }

    /// 고스트 피스(하드 드롭 그림자) 위치 계산.
    fn ghost_cells(&self) -> Vec<(i32, i32)> {
        let mut ghost = self.current.clone();
        loop {
            let next_cells: Vec<(i32, i32)> = ghost
                .cells
                .iter()
                .map(|&(cx, cy)| (ghost.x + cx, ghost.y + cy + 1))
                .collect();
            if self.board.fits(&next_cells) {
                ghost.y += 1;
            } else {
                break;
            }
        }
        ghost.absolute_cells()
    }
}

// ── 렌더링 ──────────────────────────────────────────────────────────

fn draw(stdout: &mut io::Stdout, game: &Game) -> io::Result<()> {
    // 화면 상에서의 보드 크기(문자 단위)
    let board_char_w = BOARD_W * CELL_W; // 20
    let board_char_h = BOARD_H * CELL_H; // 40

    // 현재 피스와 고스트 좌표
    let piece_cells = game.current.absolute_cells();
    let ghost_cells = game.ghost_cells();
    let piece_color = game.current.kind.color();

    // ── 위쪽 테두리 ──
    queue!(stdout, cursor::MoveTo(0, 0), style::Print("\u{250c}"))?;
    for _ in 0..board_char_w {
        queue!(stdout, style::Print("\u{2500}"))?;
    }
    queue!(stdout, style::Print("\u{2510}"))?;

    // ── 보드 영역 ──
    for row in 0..BOARD_H {
        for sub in 0..CELL_H {
            let screen_y = BOARD_Y + (row * CELL_H + sub) as u16;
            queue!(
                stdout,
                cursor::MoveTo(0, screen_y),
                style::Print("\u{2502}")
            )?;

            for col in 0..BOARD_W {
                let (cx, cy) = (col as i32, row as i32);

                let is_piece = piece_cells.contains(&(cx, cy));
                let is_ghost = !is_piece && ghost_cells.contains(&(cx, cy));
                let board_color = game.board.grid[row][col];

                if is_piece {
                    queue!(
                        stdout,
                        style::PrintStyledContent("\u{2588}\u{2588}\u{2588}\u{2588}".with(piece_color))
                    )?;
                } else if let Some(c) = board_color {
                    queue!(
                        stdout,
                        style::PrintStyledContent("\u{2588}\u{2588}\u{2588}\u{2588}".with(c))
                    )?;
                } else if is_ghost {
                    queue!(
                        stdout,
                        style::PrintStyledContent("\u{2591}\u{2591}\u{2591}\u{2591}".with(Color::DarkGrey))
                    )?;
                } else if sub == 0 {
                    queue!(
                        stdout,
                        style::PrintStyledContent("  . ".with(Color::DarkGrey))
                    )?;
                } else {
                    queue!(stdout, style::Print("    "))?;
                }
            }
            queue!(stdout, style::Print("\u{2502}"))?;

            // ── 사이드 패널 ──
            draw_side_panel(stdout, game, screen_y)?;
        }
    }

    // ── 아래쪽 테두리 ──
    let bot_y = BOARD_Y + board_char_h as u16;
    queue!(
        stdout,
        cursor::MoveTo(0, bot_y),
        style::Print("\u{2514}")
    )?;
    for _ in 0..board_char_w {
        queue!(stdout, style::Print("\u{2500}"))?;
    }
    queue!(stdout, style::Print("\u{2518}"))?;

    // 조작 안내(보드 아래)
    let help_y = bot_y + 1;
    queue!(
        stdout,
        cursor::MoveTo(0, help_y),
        style::Print("  \u{2190}\u{2192} Move  \u{2193} Soft  Space Hard  \u{2191}/Z Rotate  Q Quit")
    )?;

    stdout.flush()
}

const PANEL_W: usize = 20;

fn draw_side_panel(
    stdout: &mut io::Stdout,
    game: &Game,
    screen_y: u16,
) -> io::Result<()> {
    let panel_x = (BOARD_W * CELL_W) as u16 + 2 + 2; // after right border + gap

    // 보드 상단 기준 라인 인덱스(0부터)
    let line = screen_y - BOARD_Y;

    queue!(stdout, cursor::MoveTo(panel_x, screen_y))?;

    match line {
        0 => {
            queue!(
                stdout,
                style::PrintStyledContent(
                    format!("{:<PANEL_W$}", "NEXT").with(Color::White)
                )
            )?;
        }
        2..=9 => {
            let preview_row = ((line - 2) / CELL_H as u16) as i32;
            let next_cells = game.next.cells();
            let next_color = game.next.color();

            for pcol in 0..4i32 {
                if next_cells.contains(&(pcol, preview_row)) {
                    queue!(
                        stdout,
                        style::PrintStyledContent("\u{2588}\u{2588}\u{2588}\u{2588}".with(next_color))
                    )?;
                } else {
                    queue!(stdout, style::Print("    "))?;
                }
            }
            // 남는 칸 채우기(4셀×4문자=16, PANEL_W=20이므로 4문자 추가)
            queue!(stdout, style::Print("    "))?;
        }
        12 => {
            queue!(
                stdout,
                style::PrintStyledContent(
                    format!("{:<PANEL_W$}", format!("Score: {}", game.score))
                        .with(Color::White)
                )
            )?;
        }
        14 => {
            queue!(
                stdout,
                style::PrintStyledContent(
                    format!("{:<PANEL_W$}", format!("Lines: {}", game.lines))
                        .with(Color::White)
                )
            )?;
        }
        16 => {
            queue!(
                stdout,
                style::PrintStyledContent(
                    format!("{:<PANEL_W$}", format!("Level: {}", game.level))
                        .with(Color::White)
                )
            )?;
        }
        _ => {}
    }

    Ok(())
}

fn draw_game_over(stdout: &mut io::Stdout, game: &Game) -> io::Result<()> {
    // 화면 중앙에 GAME OVER 패널 배치
    let cx = (BOARD_W * CELL_W / 2) as u16;
    let cy = (BOARD_H * CELL_H / 2) as u16;

    let msg = "  GAME OVER  ";
    let score_msg = format!("  Score: {}  ", game.score);
    let quit_msg = "  R Retry  Q Quit  ";

    let w = msg.len().max(score_msg.len()).max(quit_msg.len()) as u16;
    let left = cx - w / 2;

    queue!(
        stdout,
        cursor::MoveTo(left, cy - 1),
        style::PrintStyledContent(msg.on(Color::Red).with(Color::White)),
        cursor::MoveTo(left, cy),
        style::PrintStyledContent(score_msg.on(Color::Red).with(Color::White)),
        cursor::MoveTo(left, cy + 1),
        style::PrintStyledContent(quit_msg.on(Color::Red).with(Color::White)),
    )?;
    stdout.flush()
}

// ── 메인 ────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();

    // 입력 처리와 화면 제어를 위해 raw mode + 대체 화면 진입
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
        terminal::Clear(ClearType::All)
    )?;

    let result = run_game(&mut stdout);

    // 종료 시 터미널 상태 복구
    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    result
}

fn run_game(stdout: &mut io::Stdout) -> io::Result<()> {
    let mut game = Game::new();
    let mut last_drop = Instant::now();

    loop {
        // ── 화면 그리기 ──
        draw(stdout, &game)?;

        if game.game_over {
            draw_game_over(stdout, &game)?;
            loop {
                // 게임 오버 상태에서는 재시작/종료 입력만 처리
                if event::poll(Duration::from_millis(200))?
                    && let Event::Key(KeyEvent {
                        code,
                        kind: KeyEventKind::Press,
                        ..
                    }) = event::read()?
                {
                    match code {
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            game = Game::new();
                            last_drop = Instant::now();
                            queue!(stdout, terminal::Clear(ClearType::All))?;
                            break;
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }

        // ── 입력 처리 ──
        let tick = Duration::from_millis(50);
        if event::poll(tick)?
            && let Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) = event::read()?
        {
            match code {
                KeyCode::Left => {
                    game.try_move(-1, 0);
                }
                KeyCode::Right => {
                    game.try_move(1, 0);
                }
                KeyCode::Down => {
                    if !game.try_move(0, 1) {
                        game.lock_and_advance();
                    }
                    last_drop = Instant::now();
                }
                KeyCode::Up | KeyCode::Char('z') | KeyCode::Char('Z') => {
                    game.try_rotate();
                }
                KeyCode::Char(' ') => {
                    game.hard_drop();
                    last_drop = Instant::now();
                }
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                    return Ok(());
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(());
                }
                _ => {}
            }
        }

        // ── 자동 낙하(중력) ──
        let interval = Duration::from_millis(game.drop_interval_ms());
        if last_drop.elapsed() >= interval {
            if !game.try_move(0, 1) {
                game.lock_and_advance();
            }
            last_drop = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn clears_single_full_row() {
        // 한 줄이 가득 찼을 때 정확히 1줄 제거되는지 확인
        let mut board = Board::new();
        let y = BOARD_H - 1;
        for x in 0..BOARD_W {
            board.grid[y][x] = Some(Color::Red);
        }

        let cleared = board.clear_lines();
        assert_eq!(cleared, 1);
        assert!(board.grid[y].iter().all(|c| c.is_none()));
    }

    #[test]
    fn clears_multiple_rows_and_compacts() {
        // 여러 줄 제거 후 아래로 내려오는지 확인
        let mut board = Board::new();
        let bottom = BOARD_H - 1;
        let above = BOARD_H - 2;
        for x in 0..BOARD_W {
            board.grid[bottom][x] = Some(Color::Blue);
            board.grid[above][x] = Some(Color::Blue);
        }
        board.grid[BOARD_H - 3][0] = Some(Color::Green);

        let cleared = board.clear_lines();
        assert_eq!(cleared, 2);
        assert!(board.grid[bottom][0].is_some());
        assert!(board.grid[above].iter().all(|c| c.is_none()));
    }

    #[test]
    fn rotates_t_piece_clockwise() {
        // T피스 회전 결과가 기대 좌표와 일치하는지 확인
        let piece = Piece::new(PieceKind::T);
        let rotated = piece.rotated_cw();
        let actual: HashSet<(i32, i32)> = rotated.into_iter().collect();
        let expected: HashSet<(i32, i32)> =
            [(1, 0), (1, 1), (1, 2), (2, 1)].into_iter().collect();
        assert_eq!(actual, expected);
    }
}
