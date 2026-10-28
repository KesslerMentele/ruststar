use std::sync::Arc;
use axum::extract::{Path, State};
use axum::{Json, Router};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use datastar::consts::ElementPatchMode;
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
    println!("POST /todos - creating todo with title={:?}", signals.title);
    let new_todo = service.create(CreateTodo { title: signals.title }).await?;
    println!("POST /todos - created todo id={}", new_todo.id);
    let todos = service.list().await?;
    let remaining = todos.iter().filter(|t| !t.completed).count();

    Ok(sse_response(vec![
        PatchElements::new(todo_item(&new_todo))
            .selector("#todo-list")
            .mode(ElementPatchMode::Append)
            .into(),
        PatchSignals::new(format!(r#"{{"remaining": {remaining}, "title": ""}}"#))
            .into(),
    ]))

}
async fn index_complete(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    println!("POST /todos/{id}/complete - completing todo");
    let todo = service.complete(id).await?;
    println!("POST /todos/{id}/complete - completed todo");
    let todos = service.list().await?;
    let remaining = todos.iter().filter(|t| !t.completed).count();
    Ok(sse_response(vec![
        PatchElements::new(todo_item(&todo)).into(),
        PatchSignals::new(format!(r#"{{"remaining": {remaining}}}"#)).into()
    ]))
}
async fn index_delete(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    println!("POST /todos/{id}/delete - deleting todo");
    service.delete(id).await?;
    println!("POST /todos/{id}/delete - deleted todo");
    let todos = service.list().await?;
    let remaining = todos.iter().filter(|t| !t.completed).count();
    Ok(sse_response(vec![
        PatchElements::new_remove(format!("#todo-{id}")).into(),
        PatchSignals::new(format!(r#"{{"remaining": {remaining}}}"#)).into()
    ]))
}
async fn detail_get(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<Markup, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    println!("GET /todos/{id} - fetching todo detail");
    let todo = service.get(id).await?;
    println!("GET /todos/{id} - found todo");
    Ok(todo_page(&todo))
}
async fn detail_complete(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    println!("POST /todos/{id}/detail/complete - completing todo");
    let todo = service.complete(id).await?;
    println!("POST /todos/{id}/detail/complete - completed todo");
    Ok(sse_response(vec![
        PatchElements::new(todo_detail_content(&todo).into_string())
            .selector("#todo-detail")
            .into(),
    ]))
}
async fn detail_delete(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, ) -> Result<impl IntoResponse, SqliteError> {
    let service = state.selected_todo_backend.read().await.clone();
    println!("POST /todos/{id}/detail/delete - deleting todo");
    service.delete(id).await?;
    println!("POST /todos/{id}/detail/delete - deleted todo");
    Ok(sse_response(vec![
        ExecuteScript::new("window.location = '/'").into()
    ]))
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
    layout(&todo.title, todo_detail_content(todo))
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