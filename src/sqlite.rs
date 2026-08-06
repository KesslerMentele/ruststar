use sqlx::{ FromRow, Pool, Sqlite};
use uuid::Uuid;
use std::sync::Arc;
use light_clone::LightClone;
use serde::Deserialize;
use async_trait::async_trait;
use axum::http::StatusCode;
use axum::response::IntoResponse;

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

#[derive(FromRow)]
pub struct TodoRow {
    pub id:Uuid,
    pub title: String,
    pub completed: bool,
}

impl From<TodoRow> for Todo {
    fn from(row: TodoRow) -> Self {
        Todo {
            id: row.id,
            title: Arc::from(row.title.as_str()),
            completed: row.completed,
        }
    }
}

#[derive(Debug)]
pub enum TodoError {
    NotFound(Uuid),
    Internal(sqlx::Error),
}

impl IntoResponse for TodoError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            TodoError::NotFound(id) => {
                println!("todo not found: id={id}");
                (StatusCode::NOT_FOUND, "Not Found")
            },
            TodoError::Internal(e) => {
                println!("sqlite error: {}", e.to_string().as_str());
                ( StatusCode::INTERNAL_SERVER_ERROR, "sqlite Error")
            },
        };
        (status,error_message).into_response()
    }
}


#[async_trait]
pub trait TodoService: Send + Sync {
    async fn list(&self) -> Result<Vec<Todo>, TodoError>;
    async fn get(&self, id: Uuid) -> Result<Todo, TodoError>;
    async fn create(&self, input: CreateTodo) -> Result<Todo, TodoError>;
    async fn complete(&self, id: Uuid) -> Result<Todo, TodoError>;
    async fn delete(&self, id: Uuid) -> Result<Todo, TodoError>;
}

pub struct SqliteTodoService {
    pub(crate) pool: Pool<Sqlite>,
}

impl SqliteTodoService {
    async fn fetch_single(&self, query: &str, id: Uuid) -> Result<Todo, TodoError> {
        println!("sqlite: fetch_single query={query:?} id={id}");
        let result = sqlx::query_as::<_, TodoRow>(query)
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => TodoError::NotFound(id),
                _ => TodoError::Internal(e),
            })
            .map(|row| row.into());
        match &result {
            Ok(_) => println!("sqlite: fetch_single succeeded id={id}"),
            Err(TodoError::NotFound(_)) => println!("sqlite: fetch_single not found id={id}"),
            Err(TodoError::Internal(e)) => println!("sqlite: fetch_single failed id={id} error={e}"),
        }
        result
    }
}

#[async_trait]
impl TodoService for SqliteTodoService {
    async fn list(&self) -> Result<Vec<Todo>, TodoError> {
        println!("sqlite: list todos");
        let result = sqlx::query_as::<_,TodoRow>("select * from todos")
            .fetch_all(&self.pool)
            .await
            .map_err(TodoError::Internal)
            .map(|rows| rows.into_iter().map(|row| row.into()).collect::<Vec<Todo>>());
        match &result {
            Ok(todos) => println!("sqlite: list succeeded count={}", todos.len()),
            Err(e) => println!("sqlite: list failed error={e:?}"),
        }
        result
    }

    async fn get(&self, id: Uuid) -> Result<Todo, TodoError> {
        println!("sqlite: get id={id}");
        self.fetch_single("select * from todos where id = ?", id).await
    }

    async fn create(&self, input: CreateTodo) -> Result<Todo, TodoError> {
        let id = Uuid::new_v4();
        println!("sqlite: create id={id} title={:?}", input.title);
        let result = sqlx::query_as::<_, TodoRow>("INSERT INTO todos (id, title, completed) VALUES (?, ?, false) RETURNING *")
            .bind(id)
            .bind(&input.title)
            .fetch_one(&self.pool)
            .await
            .map_err(TodoError::Internal)
            .map(|row| row.into());
        match &result {
            Ok(_) => println!("sqlite: create succeeded id={id}"),
            Err(e) => println!("sqlite: create failed id={id} error={e:?}"),
        }
        result
    }

    async fn complete(&self, id: Uuid) -> Result<Todo, TodoError> {
        println!("sqlite: complete id={id}");
        self.fetch_single("UPDATE todos SET completed = TRUE WHERE id = ? RETURNING *", id).await
    }

    async fn delete(&self, id: Uuid) -> Result<Todo, TodoError> {
        println!("sqlite: delete id={id}");
        self.fetch_single("DELETE FROM todos WHERE id = ? RETURNING *", id).await
    }
}

