use std::sync::Arc;
use async_trait::async_trait;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use light_clone::LightClone;
use serde::Deserialize;
use uuid::Uuid;

pub mod sqlite;
pub mod memory;
pub mod qr_code;

#[derive(Clone, LightClone)]
pub struct Todo {
    pub id: Uuid,
    pub title: Arc<str>,
    pub completed: bool,
}
#[derive(Deserialize)]
pub struct CreateTodo {
    pub title: String,
}
#[derive(Debug)]
pub enum SqliteError {
    NotFound(Uuid),
    Internal(sqlx::Error),
}
impl IntoResponse for SqliteError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            SqliteError::NotFound(id) => {
                println!("item not found: id={id}");
                (StatusCode::NOT_FOUND, "Not Found")
            },
            SqliteError::Internal(e) => {
                println!("sqlite error: {}", e.to_string().as_str());
                ( StatusCode::INTERNAL_SERVER_ERROR, "sqlite Error")
            },
        };
        (status,error_message).into_response()
    }
}
#[async_trait]
pub trait TodoService: Send + Sync {
    fn name(&self) -> &'static str;
    async fn list(&self) -> Result<Vec<Todo>, SqliteError>;
    async fn get(&self, id: Uuid) -> Result<Todo, SqliteError>;
    async fn create(&self, input: CreateTodo) -> Result<Todo, SqliteError>;
    async fn complete(&self, id: Uuid) -> Result<Todo, SqliteError>;
    async fn delete(&self, id: Uuid) -> Result<Todo, SqliteError>;
}

#[derive(Clone, LightClone)]
pub struct User {
    pub id: Uuid,
    pub name: Arc<str>,
    pub pass_hash: Arc<str>,
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub name: String,
    pub pass_hash: String,
}

#[derive(Deserialize)]
pub struct UpdateUser {
    pub id: Uuid,
    pub name: String,
    pub pass_hash: String,
}

#[async_trait]
pub trait UserService: Send + Sync {
    fn name(&self) -> &'static str;
    async fn list(&self) -> Result<Vec<User>, SqliteError>;
    async fn create(&self, input: CreateUser) -> Result<User, SqliteError>;
    async fn get(&self, id: Uuid) -> Result<User, SqliteError>;
    async fn delete(&self, id: Uuid) -> Result<User, SqliteError>;
    async fn update(&self, input: UpdateUser) -> Result<User, SqliteError>;
}