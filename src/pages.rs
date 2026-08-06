use maud::{html, Markup, DOCTYPE};
use crate::sqlite::Todo;
use crate::game_ttt::get_game_grid;

pub fn layout(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) " - Todos" }
                script type="module" src="https://cdn.jsdelivr.net/gh/starfederation/datastar@v1.0.2/bundles/datastar.js" {}
                style {
                    "body {font-family: system-ui, sans-serif; max-width: 600px; margin: 2rem auto; padding: 0 1rem; background-color: gray;}"
                    "form {display: inline}"
                    "input[type=text] { padding: 0.4rem; font-size: 1rem; }"
                    "button { padding: 0.4rem 0.8rem; font-sizeL 1rem; cursor: pointer; }"
                    ".todo { display: flex; align-items: center; gap: 0.5rem; padding: 0.5rem 0; border-bottom: 1px solid #eee; }"
                    ".todo .title { flex: 1; }"
                    ".completed .title { text-decoration: line-through; color: #999; }"
                    "#toys { justify-content: center; align-items: center; display: flex; flex-direction: column;} }"
                    "#toy-header { justify-content: center } "
                    "#game-header { justify-content: center } "
                    "#ttt-board {display: grid; grid-template-columns: repeat(3, 1fr); grid-template-rows: repeat(3, 1fr); width: 300px; height: 300px; gap: 0; }"
                    ".ttt-cell { height: 100%; width: 100% }"
                }
            }
            body {
                (content)
            }
        }
    }
}

pub fn todo_item(todo: &Todo) -> Markup {
    html! {
        div id={ "todo-" (todo.id)} class={"todo" @if todo.completed { " completed" } } {
            span class="title" {
                a href={ "/todos/" (todo.id) } { (todo.title) }
            }
            @if !todo.completed {
                button data-on:click={ "@post('/todos/" (todo.id) "/complete')" } { "Complete" }
            }
            button data-on:click={ "@post('/todos/" (todo.id) "/delete')" } { "Delete"}
        }
    }
}

pub fn index_page(todos: &[Todo]) -> Markup {
    let remaining = todos.iter().filter(|t| !t.completed).count();
    layout(
        "Home",
        html! {
            h1 { "Todos" }
            form data-signals-title="''" data-on:submit__prevent="@post('/todos')" {
                input type="text" placeholder="What needs to be done?" data-bind="title" required;
                " "
                button type="submit" { "Add" }
            }
            p {
                span data-signals-remaining=(remaining) data-text="$remaining + ' remaining'" {
                    (remaining) " remaining"
                }
            }
            div id="todo-list" {
                @for todo in todos {
                    (todo_item(todo))
                }
            }
            @if todos.is_empty() {
                p { "No todos yet. Add one above!"}
            }
        },
    )
}

pub fn todo_detail_content(todo: &Todo) -> Markup {
    html! {
        div id="todo-detail" {
            h1 { (todo.title) }
            p id="todo-status" {
                "Status: "
                @if todo.completed { "Completed" } @else { "Pending" }
            }
            p { "ID: " code { (todo.id) } }
            div id="todo-actions" {
                @if !todo.completed {
                    button data-on:click={ "@post('/todos/" (todo.id) "/detail/complete')" } { "Complete" }
                    " "
                }
                button data-on:click={ "@post('/todos/" (todo.id) "/detail/delete')" } { "Delete" }
                " "
                a href="/" { "Back to list" }
            }
        }
    }
}

pub fn todo_page(todo: &Todo) -> Markup {
    layout(&todo.title, todo_detail_content(todo))
}


pub fn games_page() -> Markup {

    layout("Toys",
           html! {
               div id="toys" {
                   h1 id="toy-header" { "Toys" }
                   (render_game_list())
               }
           }
    )
}

pub fn render_game_list() -> Markup {
    html! {
        div id="toy-container" {
            @for game in GAMES {
               button class="game-selector" data-on:click={"@get('/toys/games/" (game.slug()) "')"}  {(game.label())}
            }
       }
    }
}
pub fn tic_tac_toe(board: [char;9]) -> Markup {

    html! {
        div id="toy-container" {
            div id="ttt" {
                h1 id="game-header" { "Tic Tac Toe" }
                div id="ttt-board" {
                    @for (i, &cell) in board.iter().enumerate() {
                        @if cell == '-' {
                            button class="ttt-cell" data-on:click={ "@post('/toys/ttt/" (i + 1) "')" } {}
                        } @else {
                            div class="ttt-cell" { (cell) }
                        }
                    }
                }
            }
        }
    }
}


pub fn render_selected_game(slug:&str) -> Option<Markup> {
    match Game::from_slug(slug) {
        Some(game) => Option::from(game.render_initial()),
        None => None
    }
}


pub fn game_not_found() -> Markup {
    html! {
          div id="toy-container" {
              p { "No such game." }
              button data-on:click="@get('/toys/games')" {"Back to list"}
          }
      }
}

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
