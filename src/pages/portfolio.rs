use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use axum::extract::State;
use axum::response::{IntoResponse, Sse};
use axum::response::sse::{Event, KeepAlive};
use axum::Router;
use axum::routing::{get, post};
use datastar::patch_elements::PatchElements;
use futures::stream;
use maud::{html, Markup};
use crate::{sse_response, AppState};
use crate::pages::{layout};
use crate::pages::todo::todo_item;
use crate::services::{SqliteError, Todo};

fn current_time_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02} UTC")
}
fn clock_fragment(time: &str) -> Markup {
    html! { div id="clock-value" { (time) } }
}

fn todo_list_section(todos: &[Todo]) -> Markup {
    html! {
        div id="todo-list-section" {
            div id="todo-list" {
                @for todo in todos {
                    (todo_item(todo))
                }
            }
            @if todos.is_empty() {
                p { "No todos yet. Add one above!"}
            }
        }
    }
}

fn backend_badge(name: &str) -> Markup {
    html! { span id="backend-badge" { (name) } }
}

async fn backend_toggle(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, SqliteError> {
    let current_name = state.selected_todo_backend.read().await.name();
    let next = if current_name == state.sqlite_todo_service.name() {
        state.memory_todo_service.clone()
    } else {
        state.sqlite_todo_service.clone()
    };

    let next_name = next.name();
    *state.selected_todo_backend.write().await = next.clone();
    println!("POST /backend/toggle - now serving from {next_name}");
    let todos = next.list().await?;
    Ok(sse_response(vec![
        PatchElements::new(backend_badge(next_name)).into(),
        PatchElements::new(todo_list_section(&todos)).into(),
    ]))
}

async fn live_clock() -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    println!("Live clock");
    let stream = stream::unfold((), |_| async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let event: Event = PatchElements::new(clock_fragment(&current_time_string())).selector("#clock-value").into();
        Some((Ok::<Event, Infallible>(event), ()))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}


fn portfolio_page(backend_name: &str, clock: &str, todos: &[Todo]) -> Markup {
    layout(
        "Home",
        html! {
            div id="portfolio" {
                header id="portfolio-header" {
                    h1 { "Kessler Mentele" }
                    p id="portfolio-tagline" { "Backend-leaning developer." }
                }

                section class="demo-card" id="clock-widget" data-init="@get('/live/clock')" {
                    h2 { "Reactive without a framework" }
                    p class="demo-sub" { "No client-side JS beyond Datastar. The server pushes this over SSE, once a second." }
                    (clock_fragment(clock))
                }

                section class="demo-card" id="backend-widget" {
                    h2 { "Real Time Dependency injection" }
                    p class="demo-sub" { "The todo app underneath is written against a " code { "TodoService" } " trait. The instance backing it is swappable at runtime — not just at compile time." }
                    p { "Active backend: " (backend_badge(backend_name)) }
                    button data-on:click="@post('/backend/toggle')" { "Swap backend" }
                     form data-signals-title="''" data-on:submit__prevent="@post('/todos')" {
                        input type="text" placeholder="What needs to be done?" data-bind="title" required;
                        " "
                        button type="submit" { "Add" }
                    }
                    (todo_list_section(todos))
                }

                section class="demo-card" {
                    h2 { "More" }
                    p class="demo-sub" { "Small server-rendered, SSE-driven pieces." }
                    p { a href="/toys/pages" { "Tic-tac-toe →" } }
                }
            }
        },
    )
}


#[axum::debug_handler]
async fn portfolio_get(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, SqliteError> {
    let backend_name = state.selected_todo_backend.read().await.name();
    let service = state.selected_todo_backend.read().await.clone();
    let todos = service.list().await?;
    Ok(portfolio_page(backend_name, &current_time_string(), &todos))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
    .route("/", get(portfolio_get))
    .route("/backend/toggle", post(backend_toggle))
    .route("/live/clock", get(live_clock))
}
