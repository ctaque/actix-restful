use serde::{Serialize, Deserialize};
use actix_restful::{
    HttpCreate,
    HttpFindListDelete,
    HttpUpdate,
    Model,
    NewModel,
    UpdatableModel,
    gen_endpoint,
    RestfulPathInfo
};
use actix_restful_derive::{HttpCreate, HttpFindListDelete, HttpUpdate, actix_restful_info};
use anyhow::Result;
use async_trait::async_trait;
use std::default::Default;
use actix_web;
use serde_json;
use chrono::prelude::*;

struct AppState {}
#[derive(Default, Deserialize)]
struct FindQuery {}
#[derive(Deserialize)]
struct ListQuery {
    offset: usize,
    limit: usize,
}
#[derive(Deserialize)]
struct DeleteQuery {}
#[derive(Serialize)]
struct ListResult{
    limit: usize,
    offset: usize,
    results: Vec<Item>
}
type DeleteResult = Item;
#[derive(Deserialize)]
struct SaveQuery {}
#[derive(Deserialize)]
struct UpdateQuery {}
type Id = i64;

#[derive(Default, Serialize, Deserialize, HttpFindListDelete)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[actix_restful_info(scope = "/v1", path = "item")]
struct Item {
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
        for i in 1..500{
            res.push(Item {
                id: i,
                content: String::from("test"),
                deleted_at: None,
                updated_at: None,
                created_at: None,
            });
        }
        let paginated: Vec<Item> = res.into_iter().skip(query.offset).take(query.limit).collect();
        let output = ListResult {
            limit: query.limit,
            offset: query.offset,
            results: paginated
        };
        Ok(output)
    }
    async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
        // hard or soft delete
        let utc: DateTime<Utc> = Utc::now();
        self.deleted_at = Some(utc);
        Ok(self)
    }
}

#[derive(Serialize, Deserialize, HttpCreate)]
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

#[derive(Serialize, Deserialize, HttpUpdate)]
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

#[actix_web::main]
async fn main() -> std::io::Result<()>{
    actix_web::HttpServer::new(|| {
        actix_web::App::new()
            .service(actix_web::web::scope(Item::scope()).configure(gen_endpoint!(Item, NewItem, UpdatableItem)))
            .data(AppState{})
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}
