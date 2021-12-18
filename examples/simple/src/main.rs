use serde::{Serialize, Deserialize};
use actix_restful::{
    HttpCreate, HttpFind, HttpUpdate, HttpDelete, HttpAll, Model, NewModel, UpdatableModel
};
use actix_restful_derive::{HttpCreate, HttpFind, HttpUpdate, HttpDelete, HttpAll};
use anyhow::Result;
use chrono::prelude::*;
use async_trait::async_trait;
use std::default::Default;
use actix_web::{App, web, HttpResponse, HttpServer};
use serde_json;


#[derive(Default, Deserialize)]
struct FindQuery {}
#[derive(Deserialize)]
struct AllQuery {}
#[derive(Deserialize)]
struct DeleteQuery {}
type AllResult = Vec<Item>;
type DeleteResult = Item;
#[derive(Deserialize)]
struct SaveQuery {}
#[derive(Deserialize)]
struct UpdateQuery {}


#[derive(Default, Serialize, Deserialize, HttpFind, HttpAll, HttpDelete)]
#[http_find(FindPathParams, FindQuery)]
#[http_all(AllQuery)]
#[http_delete(FindPathParams, DeleteQuery, FindQuery)]
struct Item {
    id: i64,
    content: String,
    deleted_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, HttpCreate)]
#[http_create(SaveQuery)]
struct NewItem {
    content: String,
}

#[derive(Serialize, Deserialize, HttpUpdate)]
#[http_update(FindPathParams,UpdateQuery,Item,FindQuery)]
struct UpdatableItem {
    id: i64,
    content: String,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct FindPathParams {
    id: i64,
}

#[async_trait]
impl Model<Item, i64, FindQuery, AllQuery, AllResult, DeleteQuery, DeleteResult> for Item {
    async fn find(id: i64, _query: &FindQuery) -> Result<Item> {
        // fetch from somwhere with id and return result
        Ok(
            Item {
                id,
                content: String::from("test"),
                deleted_at: None,
                updated_at: None,
                created_at: None,
            }
        )
    }
    async fn all(_query: &AllQuery) -> Result<AllResult> {
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
    async fn delete(mut self: Self, _query: &DeleteQuery) -> Result<DeleteResult> {
        // hard or soft delete
        let utc: DateTime<Utc> = Utc::now();
        self.deleted_at = Some(utc);
        Ok(self)
    }
}


#[async_trait]
impl NewModel<Item, SaveQuery> for NewItem {
    async fn save(self: Self, _query: &SaveQuery) -> Result<Item> {
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


#[async_trait]
impl UpdatableModel<UpdatableItem, UpdateQuery> for UpdatableItem {
    async fn update(mut self: Self, _query: &UpdateQuery) -> Result<UpdatableItem> {
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
            .route("/item", web::get().to(Item::http_all))
            .route("/item", web::post().to(NewItem::http_create))
            .route("/item/{id}", web::delete().to(Item::http_delete))
            .route("/item/{id}", web::put().to(UpdatableItem::http_update))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
