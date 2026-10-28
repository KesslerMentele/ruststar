use std::sync::Arc;
use async_trait::async_trait;
use uuid::Uuid;
use crate::services::{CreateTodo, Todo, SqliteError, TodoService};

/// In-process TodoService backed by a Vec, no persistence. Exists to prove the
/// TodoService trait is a real seam: swapping this in for SqliteTodoService at
/// runtime requires no changes to handlers, pages, or routing.
#[derive(Default)]
pub struct InMemoryTodoService {
    todos: std::sync::Mutex<Vec<Todo>>,
}

impl InMemoryTodoService {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl TodoService for InMemoryTodoService {
    fn name(&self) -> &'static str { "In-Memory" }

    async fn list(&self) -> Result<Vec<Todo>, SqliteError> {
        Ok(self.todos.lock().unwrap().clone())
    }

    async fn get(&self, id: Uuid) -> Result<Todo, SqliteError> {
        self.todos.lock().unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or(SqliteError::NotFound(id))
    }

    async fn create(&self, input: CreateTodo) -> Result<Todo, SqliteError> {
        let todo = Todo { id: Uuid::new_v4(), title: Arc::from(input.title.as_str()), completed: false };
        self.todos.lock().unwrap().push(todo.clone());
        Ok(todo)
    }

    async fn complete(&self, id: Uuid) -> Result<Todo, SqliteError> {
        let mut todos = self.todos.lock().unwrap();
        let todo = todos.iter_mut().find(|t| t.id == id).ok_or(SqliteError::NotFound(id))?;
        todo.completed = true;
        Ok(todo.clone())
    }

    async fn delete(&self, id: Uuid) -> Result<Todo, SqliteError> {
        let mut todos = self.todos.lock().unwrap();
        let idx = todos.iter().position(|t| t.id == id).ok_or(SqliteError::NotFound(id))?;
        Ok(todos.remove(idx))
    }
}

