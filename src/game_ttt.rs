use std::fmt;
use std::fmt::Formatter;
use rand::random_range;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Outcome {
    Winning,
    Losing,
    Even,
    Won(char)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum GameStatus {
    InProgress,
    Won(char),
    Draw
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MoveError {
    OutOfBounds,
    CellOccupied
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MoveError::OutOfBounds => { write!(f, "Out of bounds") }
            MoveError::CellOccupied => { write!(f, "Cell occupied") }
        }
    }
}

const LINES: [[usize; 3]; 8] = [
    [0, 1, 2], [3, 4, 5], [6, 7, 8],
    [0, 3, 6], [1, 4, 7], [2, 5, 8],
    [0, 4, 8], [2, 4, 6],
];

pub fn get_game_grid() -> [char; 9] {
    ['-';9]
}
pub fn take_computer_turn(grid: &[char; 9]) -> ([char; 9], GameStatus) {
    let mut new_grid = *grid;
    let mut status:GameStatus = GameStatus::InProgress;
    // If we can win, do it
    if let Some(i) = find_actionable_move(grid, Outcome::Winning) {
        new_grid[i] = 'o';
        return (new_grid, GameStatus::Won('o'));
    }

    // If we would lose next turn, block it
    if let Some(i) = find_actionable_move(grid, Outcome::Losing) {
        new_grid[i] = 'o';
    } else if let Some(i) = random_empty_cell(grid) {
        // else, random
        new_grid[i] = 'o';
    }

    if grid.iter().all(|&c| c != '-') {
        status = GameStatus::Draw
    }
    (new_grid, status)
}

pub fn process_player_action(grid: &[char; 9], action:usize) ->  Result<([char;9], GameStatus), MoveError> {
    println!("process_player_action");
    let mut new_grid = *grid;
    if !(0..=9).contains(&action) { return Err(MoveError::OutOfBounds) }
    if grid[action] != '-' { return Err(MoveError::CellOccupied) }
    new_grid[action] = 'x';
    for set in get_winning_sets(&new_grid) {
        if check_set(&set) == Outcome::Won('x') { return  Ok((new_grid, GameStatus::Won('x'))) }
    }
    Ok(take_computer_turn(&new_grid))
}

fn find_actionable_move(grid: &[char; 9], target: Outcome) -> Option<usize> {
    let sets = get_winning_sets(grid);

    for (line, set) in LINES.iter().zip(sets.iter()) {
        if check_set(set) == target && let Some(&i) = line.iter().find(|&&i| grid[i] == '-') {
            return Some(i);
        }
    }

    None
}

fn random_empty_cell(grid: &[char; 9]) -> Option<usize> {
    let empties: Vec<usize> = (0..9).filter(|&i| grid[i] == '-').collect();

    if empties.is_empty() {
        return None;
    }

    Some(empties[random_range(0..empties.len())])
}

fn get_winning_sets(grid: &[char; 9]) -> [[char; 3]; 8] {
    LINES.map(|[a, b, c]| [grid[a], grid[b], grid[c]])
}

fn check_set(set:&[char;3]) -> Outcome {

    if set[0] != '-' && set[0] == set[1] && set[1] == set[2] {
        return Outcome::Won(set[0])
    }

    if set.iter().filter(|&&c| c== 'o').count() > 1 {
        return Outcome::Winning
    }

    if set.iter().filter(|&&c| c== 'x').count() > 1 {
        return Outcome::Losing
    }

    Outcome::Even
}


