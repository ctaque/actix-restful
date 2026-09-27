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

use crate::shared::AppState;

// Query structs are documented as query parameters
#[derive(Default, Deserialize, JsonSchema, ApiComponent)]
struct FindQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct ListQuery {
    /// Number of items to skip
    offset: Option<usize>,
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

/// An item of the store
#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, HttpFindListDelete)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[actix_restful_info(path = "item")]
pub struct Item {
    id: Id,
    content: String,
    deleted_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

#[async_trait]
impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Item {
    async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<Item>> {
        // fetch from somwhere with id and return result
        Ok(
            Box::new(
                Item {
                    id,
                    content: String::from("test"),
                    deleted_at: None,
                    updated_at: None,
                    created_at: None,
                }
            )
        )
    }
    async fn list(query: &ListQuery, _state: &AppState) -> Result<ListResult> {
        // list
        let mut res = Vec::new();
        let offset = query.offset.unwrap_or(0) as i64;
        for i in offset..offset + 2 {
            res.push(Item {
                id: i,
                content: String::from("test"),
                deleted_at: None,
                updated_at: None,
                created_at: None,
            });
        }
        Ok(res)
    }
    async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
        // hard or soft delete
        let utc: DateTime<Utc> = Utc::now();
        self.deleted_at = Some(utc);
        Ok(self)
    }
}

/// The payload to create an item
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate)]
#[http_create(SaveQuery, AppState)]
struct NewItem {
    content: String,
}
#[async_trait]
impl NewModel<Item, SaveQuery, AppState> for NewItem {
    async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<Item> {
        // persist, and return Item entity
        let utc: DateTime<Utc> = Utc::now();
        Ok(Item{
            id: 1,
            content: self.content,
            created_at: Some(utc),
            deleted_at: None,
            updated_at: None,
        })
    }
}

/// The payload to update an item
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpUpdate)]
#[http_update(Id, UpdateQuery, Item, FindQuery, AppState)]
struct UpdatableItem {
    id: Id,
    content: String,
    updated_at: Option<DateTime<Utc>>,
}
#[async_trait]
impl UpdatableModel<UpdatableItem, UpdateQuery, AppState> for UpdatableItem {
    async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<UpdatableItem> {
        // update in db
        let utc: DateTime<Utc> = Utc::now();
        self.updated_at = Some(utc);
        Ok(self)
    }
}

// Registers the documented routes of the item endpoint
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_endpoint!(Item, NewItem, UpdatableItem)(cfg)
}
