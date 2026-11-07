use std::sync::Arc;
use axum::extract::{Path, State};
use axum::{Json, Router};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use datastar::execute_script::ExecuteScript;
use datastar::patch_elements::PatchElements;
use datastar::patch_signals::PatchSignals;
use maud::{html, Markup};
use uuid::Uuid;
use crate::{sse_response, AppState, CreateSignals};
use crate::pages::layout;
use crate::services::{CreateTodo, SqliteError, Todo};

async fn index_create(State(state): State<Arc<AppState>>, Json(signals): Json<CreateSignals>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    let new_todo = service.create(CreateTodo { title: signals.title }).await?;
    let todos = service.list().await?;
    let remaining = todos.iter().filter(|t| !t.completed).count();

    Ok(sse_response(vec![
        PatchElements::new(todo_list_section(&todos)).into(),
        PatchSignals::new(format!(r#"{{"remaining": {remaining}, "title": ""}}"#))
            .into(),
    ]))

}
async fn index_complete(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    let todo = service.complete(id).await?;
    let todos = service.list().await?;
    let remaining = todos.iter().filter(|t| !t.completed).count();
    Ok(sse_response(vec![
        PatchElements::new(todo_item(&todo)).into(),
        PatchSignals::new(format!(r#"{{"remaining": {remaining}}}"#)).into()
    ]))
}
async fn index_delete(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    service.delete(id).await?;
    let todos = service.list().await?;
    let remaining = todos.iter().filter(|t| !t.completed).count();
    Ok(sse_response(vec![
        PatchElements::new(todo_list_section(&todos)).into(),
        PatchSignals::new(format!(r#"{{"remaining": {remaining}}}"#)).into()
    ]))
}
async fn detail_get(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<Markup, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    let todo = service.get(id).await?;
    Ok(todo_page(&todo))
}
async fn detail_complete(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    let todo = service.complete(id).await?;
    Ok(sse_response(vec![
        PatchElements::new(todo_detail_content(&todo).into_string())
            .selector("#todo-detail")
            .into(),
    ]))
}
async fn detail_delete(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    service.delete(id).await?;
    Ok(sse_response(vec![
        ExecuteScript::new("window.location = '/'").into()
    ]))
}
pub fn todo_list_section(todos: &[Todo]) -> Markup {
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
fn todo_item(todo: &Todo) -> Markup {
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
fn todo_detail_content(todo: &Todo) -> Markup {
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
fn todo_page(todo: &Todo) -> Markup {
    layout(
        &todo.title,
        "",
        html! {
            link rel="stylesheet" href="/assets/todo.css";
        },
        todo_detail_content(todo),
    )
}

pub fn router() ->  Router<Arc<AppState>> {
    Router::new()
        .route("/", post(index_create))
        .route("/{id}/complete", post(index_complete))
        .route("/{id}/delete", post(index_delete))
        .route("/{id}", get(detail_get))
        .route("/{id}/detail/complete", post(detail_complete))
        .route("/{id}/detail/delete", post(detail_delete))
}