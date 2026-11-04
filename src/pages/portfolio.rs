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
use tracing::span::Attributes;
use crate::{sse_response, AppState};
use crate::pages::{layout};
use crate::pages::todo::todo_list_section;
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
    html! { div data-init="@get('/live/clock')" id="clock-value" { (time) } }
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
            link rel="stylesheet" href="/assets/portfolio.css";
            link rel="stylesheet" href="/assets/todo.css";
        },
        html! {
            div id="portfolio" {

                (portfolio_section(
                    "Reactive without a framework",
                    Option::from("No client-side JS beyond Datastar. The server pushes this over SSE, once a second."),
                    clock_fragment(clock)
                ))
                (portfolio_section(
                    "Real Time Dependency injection",
                    Option::from("The todo app underneath is written against a hot-swappable backend service."),
                    html!{
                        div class="article-container" {
                            button data-on:click="@post('/backend/toggle')" { "Swap backend" }
                            p { "Active backend: " (backend_badge(backend_name)) }
                        }
                        form class="demo-form" data-signals-title="''" data-on:submit__prevent="@post('/todos')" {
                            input type="text" placeholder="What needs to be done?" data-bind="title" required;
                            button type="submit" { "Add" }
                        }
                        (todo_list_section(todos))
                    }
                ))

                (portfolio_section(
                    "More",
                    Option::from("Some more SSE-driven creations"),
                    html!{p {
                        a href="/toys/pages" { "Toys" }
                        a href="/toys/pages" { "Toys" }
                    }}
                ))
                
            }
        },
    )
}


fn portfolio_section(header: &str, sub: Option<&str>, content: Markup) -> Markup {
    html! {
         section class="article-section papered" id="clock-widget" {
                    h2 class="article-header" { (header) }
                    @if sub.is_some() {p class="article-sub-header"  { (sub.unwrap()) }}
                    (content)
                }
    }

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
