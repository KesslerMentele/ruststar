mod services;
mod pages;
mod game_ttt;
mod sessions;

use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{Router};
use axum::response::{Sse, sse::{Event}};
use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use futures::stream;
use tower_http::services::ServeDir;
use crate::pages::{portfolio, todo, toys};
use crate::services::{TodoService, memory::InMemoryTodoService, sqlite::SqliteTodoService, UserService, sqlite::SqliteUserService};
use tracing_subscriber::{EnvFilter, prelude::*};

struct AppState {
    selected_todo_backend: RwLock<Arc<dyn TodoService>>,
    sqlite_todo_service: Arc<dyn TodoService>,
    memory_todo_service: Arc<dyn TodoService>,
    sqlite_user_service: Arc<dyn UserService>,
}


#[derive(Deserialize)]
struct CreateSignals {
    title: String,
}

fn sse_response(events: Vec<Event>, ) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    Sse::new(stream::iter(events.into_iter().map(Ok)))
}


#[tokio::main]
async fn main() {
    // Tracing setup
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "with_axum_htmx_askama=debug".into())
        )
        .with(
            tracing_subscriber::fmt::layer()
        )
        .init();

    // Connect to or create the DB
    println!("connecting to sqlite://todos.db");
    let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(SqliteConnectOptions::from_str("sqlite://todos.db").unwrap().create_if_missing(true))
            .await
            .unwrap();
    println!("connected to sqlite://todos.db");
    sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&pool)
            .await
            .unwrap();
    println!("schema initialized");

    // Initialize the services and create the state object
    let sqlite_todo_service: Arc<dyn TodoService> = Arc::new(SqliteTodoService { pool: pool.clone() });
    let sqlite_user_service: Arc<dyn UserService> = Arc::new(SqliteUserService { pool: pool.clone() });
    let memory_service: Arc<dyn TodoService> = Arc::new(InMemoryTodoService::new());

    let state = Arc::new(AppState {
        selected_todo_backend: RwLock::new(sqlite_todo_service.clone()),
        sqlite_todo_service,
        sqlite_user_service,
        memory_todo_service: memory_service,
    });

    // collect all the routers
    let todo_routes = todo::router();
    let toy_routes = toys::router();
    let portfolio_routes = portfolio::router();

    let assets_path = std::env::current_dir().unwrap();

    let app = Router::new()
        .merge(portfolio_routes)
        .nest_service("/assets", ServeDir::new(assets_path.to_str().unwrap()))
        .nest("/todos", todo_routes)
        .nest("/toys", toy_routes)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7878").await.unwrap();
    println!("listening on http://127.0.0.1:7878");

    axum::serve(listener, app).await.unwrap();
}

