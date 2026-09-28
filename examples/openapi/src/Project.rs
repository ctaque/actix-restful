use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use actix_restful::{
    HttpCreate,
    HttpFindListDelete,
    HttpUpdate,
    Model,
    NewModel,
    UpdatableModel,
    gen_documented_endpoint,
    RestfulPathInfo
};
use actix_restful_derive::{HttpCreate, HttpFindListDelete, HttpUpdate, actix_restful_info};
use anyhow::Result;
use apistos::ApiComponent;
use async_trait::async_trait;
use schemars::JsonSchema;
use std::default::Default;
use actix_web;

use crate::shared::{pagination, AppState};

#[derive(Default, Deserialize, JsonSchema, ApiComponent)]
struct FindQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct ListQuery {
    /// Number of projects to skip
    offset: Option<usize>,
    /// Maximum number of projects to return (20 by default, 100 at most)
    limit: Option<usize>,
}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct DeleteQuery {}
type ListResult = Vec<Project>;
type DeleteResult = Project;
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct SaveQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct UpdateQuery {}
type Id = i64;

/// A project
#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpFindListDelete)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[actix_restful_info(path = "project")]
pub struct Project {
    id: Id,
    content: String,
    deleted_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

#[async_trait]
impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Project {
    async fn find(id: Id, _query: &FindQuery, state: &AppState) -> Result<Box<Project>> {
        let project = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
        Ok(Box::new(project))
    }
    async fn list(query: &ListQuery, state: &AppState) -> Result<ListResult> {
        let (offset, limit) = pagination(query.offset, query.limit);
        let projects = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE deleted_at IS NULL ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;
        Ok(projects)
    }
    async fn delete(self: Self, _query: &DeleteQuery, state: &AppState) -> Result<DeleteResult> {
        // Soft deletes the project along with its items, atomically
        let now = Utc::now();
        let mut tx = state.pool.begin().await?;
        sqlx::query("UPDATE items SET deleted_at = ? WHERE project_id = ? AND deleted_at IS NULL")
            .bind(now)
            .bind(self.id)
            .execute(&mut *tx)
            .await?;
        let project = sqlx::query_as::<_, Project>(
            "UPDATE projects SET deleted_at = ? WHERE id = ? RETURNING *",
        )
        .bind(now)
        .bind(self.id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(project)
    }
}

/// The payload to create a project
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate)]
#[http_create(SaveQuery, AppState)]
struct NewProject {
    content: String,
}
#[async_trait]
impl NewModel<Project, SaveQuery, AppState> for NewProject {
    async fn save(self: Self, _query: &SaveQuery, state: &AppState) -> Result<Project> {
        let now = Utc::now();
        let project = sqlx::query_as::<_, Project>(
            "INSERT INTO projects (content, created_at, updated_at) VALUES (?, ?, ?) RETURNING *",
        )
        .bind(self.content)
        .bind(now)
        .bind(now)
        .fetch_one(&state.pool)
        .await?;
        Ok(project)
    }
}

/// The payload to update a project
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate)]
#[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
struct UpdatableProject {
    id: Id,
    content: String,
    updated_at: Option<DateTime<Utc>>,
}
#[async_trait]
impl UpdatableModel<UpdatableProject, UpdateQuery, AppState> for UpdatableProject {
    async fn update(self: Self, _query: &UpdateQuery, state: &AppState) -> Result<UpdatableProject> {
        let project = sqlx::query_as::<_, UpdatableProject>(
            "UPDATE projects SET content = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL
             RETURNING id, content, updated_at",
        )
        .bind(self.content)
        .bind(Utc::now())
        .bind(self.id)
        .fetch_one(&state.pool)
        .await?;
        Ok(project)
    }
}

// Registers the documented routes of the project endpoint
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_endpoint!(Project, NewProject, UpdatableProject)(cfg)
}
