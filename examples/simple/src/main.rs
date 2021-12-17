use serde::{Serialize, Deserialize};
use actix_restful::{
    HttpCreate, HttpFind, HttpUpdate, HttpDelete, HttpAll, Model, NewModel, UpdatableModel
};
use actix_restful_derive::{HttpCreate, HttpFind, HttpUpdate, HttpDelete, HttpAll};
use anyhow::Result;
use chrono::prelude::*;
use async_trait::async_trait;
use std::default::Default;
use actix_web::{http, web, HttpResponse};
use serde_json;


#[derive(Default)]
struct FindQuery {}
struct AllQuery {}
struct DeleteQuery {}
type AllResult = Vec<Item>;
type DeleteResult = Item;
struct SaveQuery {}
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
impl NewModel<NewItem, SaveQuery> for NewItem {
    async fn save(self: Self, _query: &SaveQuery) -> Result<NewItem> {
        // persist,

        Ok(self)
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

fn main() {
    println!("Hello, world!");
}
