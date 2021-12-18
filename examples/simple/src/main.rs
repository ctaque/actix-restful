use serde::{Serialize, Deserialize};
use actix_restful::{
    HttpCreate, HttpFindListDelete, HttpUpdate, Model, NewModel, UpdatableModel
};
use actix_restful_derive::{HttpCreate, HttpFindListDelete, HttpUpdate};
use anyhow::Result;
use chrono::prelude::*;
use async_trait::async_trait;
use std::default::Default;
use actix_web::{App, web, HttpResponse, HttpServer};
use serde_json;

struct AppState {}
#[derive(Default, Deserialize)]
struct FindQuery {}
#[derive(Deserialize)]
struct ListQuery {}
#[derive(Deserialize)]
struct DeleteQuery {}
type ListResult = Vec<Item>;
type DeleteResult = Item;
#[derive(Deserialize)]
struct SaveQuery {}
#[derive(Deserialize)]
struct UpdateQuery {}
type Id = i64;

#[derive(Default, Serialize, Deserialize, HttpFindListDelete)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
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
    async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
        // list
        let mut res = Vec::new();
        for i in 0..2{
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
#[http_update(i64, UpdateQuery, Item, FindQuery, AppState)]
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
    HttpServer::new(|| {
        App::new()
            .route("/item/{id}", web::get().to(Item::http_find))
            .route("/item", web::get().to(Item::http_list))
            .route("/item", web::post().to(NewItem::http_create))
            .route("/item/{id}", web::delete().to(Item::http_delete))
            .route("/item/{id}", web::put().to(UpdatableItem::http_update))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
