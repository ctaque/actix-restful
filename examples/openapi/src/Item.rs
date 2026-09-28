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
use chrono::prelude::*;

use crate::shared::{pagination, AppState};

// Query structs are documented as query parameters
#[derive(Default, Deserialize, JsonSchema, ApiComponent)]
struct FindQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct ListQuery {
    /// Number of items to skip
    offset: Option<usize>,
    /// Maximum number of items to return (20 by default, 100 at most)
    limit: Option<usize>,
    /// Only return the items of this project
    project_id: Option<i64>,
}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct DeleteQuery {}
type ListResult = Vec<Item>;
type DeleteResult = Item;
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct SaveQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct UpdateQuery {}
type Id = i64;

/// An item of a project
#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpFindListDelete)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[actix_restful_info(path = "item")]
pub struct Item {
    id: Id,
    project_id: i64,
    content: String,
    deleted_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

#[async_trait]
impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Item {
    async fn find(id: Id, _query: &FindQuery, state: &AppState) -> Result<Box<Item>> {
        // Soft deleted items are not found anymore, which answers a 404
        let item = sqlx::query_as::<_, Item>(
            "SELECT * FROM items WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
        Ok(Box::new(item))
    }
    async fn list(query: &ListQuery, state: &AppState) -> Result<ListResult> {
        let (offset, limit) = pagination(query.offset, query.limit);
        let items = sqlx::query_as::<_, Item>(
            "SELECT * FROM items
             WHERE deleted_at IS NULL AND (?1 IS NULL OR project_id = ?1)
             ORDER BY id
             LIMIT ?2 OFFSET ?3",
        )
        .bind(query.project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;
        Ok(items)
    }
    async fn delete(self: Self, _query: &DeleteQuery, state: &AppState) -> Result<DeleteResult> {
        // Soft delete: the row is kept, flagged with its deletion date
        let item = sqlx::query_as::<_, Item>(
            "UPDATE items SET deleted_at = ? WHERE id = ? RETURNING *",
        )
        .bind(Utc::now())
        .bind(self.id)
        .fetch_one(&state.pool)
        .await?;
        Ok(item)
    }
}

/// The payload to create an item
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate)]
#[http_create(SaveQuery, AppState)]
struct NewItem {
    /// The project the item belongs to
    project_id: i64,
    content: String,
}
#[async_trait]
impl NewModel<Item, SaveQuery, AppState> for NewItem {
    async fn save(self: Self, _query: &SaveQuery, state: &AppState) -> Result<Item> {
        let project_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM projects WHERE id = ? AND deleted_at IS NULL)",
        )
        .bind(self.project_id)
        .fetch_one(&state.pool)
        .await?;
        anyhow::ensure!(project_exists, "PROJECT_NOT_FOUND");

        let now = Utc::now();
        let item = sqlx::query_as::<_, Item>(
            "INSERT INTO items (project_id, content, created_at, updated_at)
             VALUES (?, ?, ?, ?)
             RETURNING *",
        )
        .bind(self.project_id)
        .bind(self.content)
        .bind(now)
        .bind(now)
        .fetch_one(&state.pool)
        .await?;
        Ok(item)
    }
}

/// The payload to update an item
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate)]
#[http_update(Id, UpdateQuery, Item, FindQuery, AppState)]
struct UpdatableItem {
    id: Id,
    content: String,
    updated_at: Option<DateTime<Utc>>,
}
#[async_trait]
impl UpdatableModel<UpdatableItem, UpdateQuery, AppState> for UpdatableItem {
    async fn update(self: Self, _query: &UpdateQuery, state: &AppState) -> Result<UpdatableItem> {
        let item = sqlx::query_as::<_, UpdatableItem>(
            "UPDATE items SET content = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL
             RETURNING id, content, updated_at",
        )
        .bind(self.content)
        .bind(Utc::now())
        .bind(self.id)
        .fetch_one(&state.pool)
        .await?;
        Ok(item)
    }
}

// Registers the documented routes of the item endpoint
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_endpoint!(Item, NewItem, UpdatableItem)(cfg)
}
