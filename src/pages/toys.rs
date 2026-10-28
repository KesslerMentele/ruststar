use std::sync::Arc;
use axum::extract::Path;
use axum::{Json, Router};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use datastar::patch_elements::PatchElements;
use maud::{html, Markup};
use serde::Deserialize;
use crate::game_ttt::{get_game_grid, process_player_action, GameStatus};
use crate::{sse_response, AppState};
use crate::pages::{home_button, layout};
use crate::services::SqliteError;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Game {
    Ttt
}
const GAMES: &[Game] = &[Game::Ttt];
impl Game {
    fn slug(&self) -> &'static str { match self { Game::Ttt => {"ttt"} }}
    fn label(&self) -> &'static str { match self { Game::Ttt => {"Tic Tac Toe"} }}
    fn render_initial(&self) -> Markup { match self { Game::Ttt => tic_tac_toe(get_game_grid()) }}
    fn from_slug(slug: &str) -> Option<Self> { GAMES.iter().copied().find(|g| g.slug() == slug)}
}

#[derive(Deserialize)]
struct GameGrid {
    grid: [char;9]
}

async fn ttt_post(Path(cell): Path<usize>, Json(payload): Json<GameGrid>) -> Result<impl IntoResponse, SqliteError> {
    println!("POST /toys/pages/ttt/{cell}");
    let game = process_player_action(&payload.grid, cell);

    match game {
        Ok((grid, status)) => {
            match status {
                GameStatus::InProgress => {
                    println!("POST /todos/pages/ttt/{cell} \n Status: In Progress, \n grid:{}", grid.iter().collect::<String>() );
                    Ok(sse_response(vec![
                        PatchElements::new(tic_tac_toe(grid)).selector("#toy-container").into()
                    ]))
                }
                GameStatus::Won(c) => {
                    println!("POST /todos/pages/ttt/{cell}  \n Status: Won, \n grid:{}", grid.iter().collect::<String>() );
                    Ok(sse_response(vec![
                        PatchElements::new(ttt_won(grid, c)).selector("#toy-container").into()
                    ])) // display win screen
                }
                GameStatus::Draw => {
                    println!("POST /todos/pages/ttt/{cell}  \n Status: Draw, \n grid:{}", grid.iter().collect::<String>() );
                    Ok(sse_response(vec![
                        PatchElements::new(ttt_draw(grid)).selector("#toy-container").into()
                    ])) // display draw screen
                }
            }

        },
        Err(e) => {
            println!("POST /todos/pages/ttt/{cell}  Error: {e}");
            Ok(sse_response(vec![])) // display same grid with error message
        }
    }


}
async fn game_list_get() -> Result<impl IntoResponse, SqliteError> { 
    println!("GET /toys/pages");
    Ok(games_page())
}
async fn game_get(Path(slug): Path<String>) -> Result<impl IntoResponse, SqliteError> {
    println!("GET /todos/pages/{slug}");
    let markup =render_selected_game(&slug);

    match markup {
        Some(markup) => {
            Ok(sse_response(vec![
                PatchElements::new(markup).selector("#toy-container").into()
            ]))
        },
        None => {
            Ok(sse_response(vec![
                PatchElements::new(game_not_found()).selector("#toy-container").into()
            ]))
        }
    }

}
fn games_page() -> Markup {

    layout("Toys",
           html! {
               div id="toys" {
                   h1 id="toy-header" { "Toys" }
                   (render_game_list())
                   (home_button())
               }
           }
    )
}
fn render_game_list() -> Markup {
    html! {
        div id="toy-container" {
            @for game in GAMES {
               button class="game-selector" data-on:click={"@get('/toys/pages/" (game.slug()) "')"}  {(game.label())}
            }
       }
    }
}
fn tic_tac_toe(board: [char;9]) -> Markup {
    let grid_json = serde_json::to_string(&board).unwrap();

    html! {
        div id="toy-container" {
            div id="ttt" data-signals:grid=(grid_json) {
                h1 id="game-header" { "Tic Tac Toe" }
                div id="ttt-board" {
                    @for (i, &cell) in board.iter().enumerate() {
                        @if cell == '-' {
                            button id={"ttt-cell-" (i)} class="ttt-cell"  data-on:click={ "@post('/toys/pages/ttt/" (i) "')" } {(i + 1)}
                        } @else {
                            div id={"ttt-cell-" (i)} class="ttt-cell" { (cell) }
                        }
                    }
                }
            }
        }
    }
}
fn ttt_won(board: [char;9], winner: char) -> Markup {
    html! {
        div id="toy-container" {
            (tic_tac_toe(board))
            p id="ttt-result" {(winner) " wins!"}
            (restart_button(Game::Ttt))
        }
    }
}
fn ttt_draw(board: [char;9]) -> Markup {
    html! {
        div id="toy-container" {
            (tic_tac_toe(board))
            p id="ttt-result" {"Draw"}
            (restart_button(Game::Ttt))
        }
    }
}
fn render_selected_game(slug:&str) -> Option<Markup> {
    match Game::from_slug(slug) {
        Some(game) => Option::from(game.render_initial()),
        None => None
    }
}
fn game_not_found() -> Markup {
    html! {
          div id="toy-container" {
              p { "No such game." }
              button data-on:click="@get('/toys/pages')" {"Back to list"}
          }
      }
}
fn restart_button(game:Game) -> Markup {
    html! {
        button id="restart" data-on:click={"@get('/toys/pages/" (game.slug()) "')"} {"Restart" }
    }
}

pub fn router() -> Router<Arc<AppState>> {
     Router::new()
        .route("/pages", get(game_list_get))
        .route("/pages/{slug}", get(game_get))
        .route("/pages/ttt/{cell}", post(ttt_post))
}