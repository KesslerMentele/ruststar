use sqlx::{ FromRow, Pool, Sqlite};
use uuid::Uuid;
use std::sync::Arc;
use async_trait::async_trait;
use crate::services::{CreateTodo, Todo, SqliteError, TodoService, User, UserService, CreateUser, UpdateUser};

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
pub struct SqliteTodoService {
    pub(crate) pool: Pool<Sqlite>
}
#[async_trait]
impl TodoService for SqliteTodoService {
    fn name(&self) -> &'static str { "SQLite" }
    async fn list(&self) -> Result<Vec<Todo>, SqliteError> {
        println!("sqlite: list todos");
        let result = sqlx::query_as::<_,TodoRow>("select * from todos")
            .fetch_all(&self.pool)
            .await
            .map_err(SqliteError::Internal)
            .map(|rows| rows.into_iter().map(|row| row.into()).collect::<Vec<Todo>>());
        match &result {
            Ok(todos) => println!("sqlite: list succeeded count={}", todos.len()),
            Err(e) => println!("sqlite: list failed error={e:?}"),
        }
        result
    }
    async fn get(&self, id: Uuid) -> Result<Todo, SqliteError> {
        println!("sqlite: get id={id}");
        sqlx::query_as::<_, TodoRow>("SELECT * FROM todos WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => SqliteError::NotFound(id),
                _ => SqliteError::Internal(e),
            })
            .map(|row| row.into())
    }
    async fn create(&self, input: CreateTodo) -> Result<Todo, SqliteError> {
        let id = Uuid::new_v4();
        println!("sqlite: create id={id} title={:?}", input.title);
        let result = sqlx::query_as::<_, TodoRow>("INSERT INTO todos (id, title, completed) VALUES (?, ?, false) RETURNING *")
            .bind(id)
            .bind(&input.title)
            .fetch_one(&self.pool)
            .await
            .map_err(SqliteError::Internal)
            .map(|row| row.into());
        match &result {
            Ok(_) => println!("sqlite: create succeeded id={id}"),
            Err(e) => println!("sqlite: create failed id={id} error={e:?}"),
        }
        result
    }
    async fn complete(&self, id: Uuid) -> Result<Todo, SqliteError> {
        println!("sqlite: complete id={id}");
        sqlx::query_as::<_, TodoRow>("UPDATE todos SET completed = TRUE WHERE id = ? RETURNING *")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => SqliteError::NotFound(id),
                _ => SqliteError::Internal(e),
            })
            .map(|row| row.into())
    }
    async fn delete(&self, id: Uuid) -> Result<Todo, SqliteError> {
        println!("sqlite: delete id={id}");
        sqlx::query_as::<_, TodoRow>("DELETE FROM todos WHERE id = ? RETURNING *")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => SqliteError::NotFound(id),
                _ => SqliteError::Internal(e),
            })
            .map(|row| row.into())
    }
}
#[derive(FromRow)]
pub struct UserRow {
    pub id:Uuid,
    pub name: String,
    pub pass_hash: String,
}
impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            id: row.id,
            name: Arc::from(row.name.as_str()),
            pass_hash: Arc::from(row.pass_hash.as_str()),
        }
    }
}
pub struct SqliteUserService {
    pub(crate) pool: Pool<Sqlite>,
}
#[async_trait]
impl UserService for SqliteUserService {
    fn name(&self) -> &'static str {
        "SQLite"
    }
    async fn list(&self) -> Result<Vec<User>, SqliteError> {
        let result = sqlx::query_as::<_, UserRow>("select * from users")
            .fetch_all(&self.pool)
            .await
            .map_err(SqliteError::Internal)
            .map(|rows| rows.into_iter().map(|row| row.into()).collect::<Vec<User>>());
        result
    }
    async fn create(&self, input: CreateUser) -> Result<User, SqliteError> {
        let id = Uuid::new_v4();
        let result = sqlx::query_as::<_, UserRow>("INSERT INTO users (id, name, pass_hash) VALUES (?, ?, ?) RETURNING *")
            .bind(id)
            .bind(&input.name)
            .bind(&input.pass_hash)
            .fetch_one(&self.pool)
            .await
            .map_err(SqliteError::Internal)
            .map(|row| row.into());
        result
    }
    async fn get(&self, id: Uuid) -> Result<User, SqliteError> {
        sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => SqliteError::NotFound(id),
                _ => SqliteError::Internal(e),
            })
            .map(|row| row.into())
    }
    async fn delete(&self, id: Uuid) -> Result<User, SqliteError> {
        sqlx::query_as::<_, UserRow>("DELETE FROM users WHERE id = ? RETURNING *")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => SqliteError::NotFound(id),
                _ => SqliteError::Internal(e),
            })
            .map(|row| row.into())
    }
    async fn update(&self, user: UpdateUser) -> Result<User, SqliteError> {
        let result = sqlx::query_as::<_, UserRow>("UPDATE users SET name = ?, pass_hash = ? WHERE id = ? RETURNING *")
            .bind(&user.name)
            .bind(&user.pass_hash)
            .bind(user.id)
            .fetch_one(&self.pool)
            .await
            .map_err(SqliteError::Internal)
            .map(|row| row.into());
        result
    }
}
