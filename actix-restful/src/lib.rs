//!
//! # Example of use :
//! ```
//!use serde::{Serialize, Deserialize};
//!use actix_restful::{
//!     HttpCreate,
//!     HttpFindListDelete,
//!     HttpUpdate,
//!     Model,
//!     NewModel,
//!     UpdatableModel,
//!     gen_endpoint,
//!     RestfulPathInfo
//!};
//!use actix_restful_derive::{HttpCreate, HttpFindListDelete, HttpUpdate, actix_restful_info};
//!use anyhow::Result;
//!use async_trait::async_trait;
//!use std::default::Default;
//!use actix_web;
//!use serde_json;
//!use chrono::prelude::*;
//!
//!struct AppState {}
//!#[derive(Default, Deserialize)]
//!struct FindQuery {}
//!#[derive(Deserialize)]
//!struct ListQuery {}
//!#[derive(Deserialize)]
//!struct DeleteQuery {}
//!type ListResult = Vec<Item>;
//!type DeleteResult = Item;
//!#[derive(Deserialize)]
//!struct SaveQuery {}
//!#[derive(Deserialize)]
//!struct UpdateQuery {}
//!type Id = i64;
//!
//!#[derive(Default, Serialize, Deserialize, HttpFindListDelete)]
//!#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
//!#[actix_restful_info(scope = "/v1", path = "item")]
//!struct Item {
//!    id: Id,
//!    content: String,
//!    deleted_at: Option<DateTime<Utc>>,
//!    updated_at: Option<DateTime<Utc>>,
//!    created_at: Option<DateTime<Utc>>,
//!}
//!
//!#[async_trait]
//!impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Item {
//!    async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<Item>> {
//!        // fetch from somwhere with id and return result
//!        Ok(
//!            Box::new(
//!                Item {
//!                    id,
//!                    content: String::from("test"),
//!                    deleted_at: None,
//!                    updated_at: None,
//!                    created_at: None,
//!                }
//!            )
//!        )
//!    }
//!    async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
//!        // list
//!        let mut res = Vec::new();
//!        for i in 0..2{
//!            res.push(Item {
//!                id: i,
//!                content: String::from("test"),
//!                deleted_at: None,
//!                updated_at: None,
//!                created_at: None,
//!            });
//!        }
//!        Ok(res)
//!    }
//!    async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
//!        // hard or soft delete
//!        let utc: DateTime<Utc> = Utc::now();
//!        self.deleted_at = Some(utc);
//!        Ok(self)
//!    }
//!}
//!
//!#[derive(Serialize, Deserialize, HttpCreate)]
//!#[http_create(SaveQuery, AppState)]
//!struct NewItem {
//!     content: String,
//!}
//!#[async_trait]
//!impl NewModel<Item, SaveQuery, AppState> for NewItem {
//!     async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<Item> {
//!         // persist, and return Item entity
//!         let utc: DateTime<Utc> = Utc::now();
//!         Ok(Item{
//!             id: 1,
//!             content: self.content,
//!             created_at: Some(utc),
//!             deleted_at: None,
//!             updated_at: None,
//!         })
//!     }
//!}
//!
//!#[derive(Serialize, Deserialize, HttpUpdate)]
//!#[http_update(Id, UpdateQuery, Item, FindQuery, AppState)]
//!struct UpdatableItem {
//!     id: Id,
//!     content: String,
//!     updated_at: Option<DateTime<Utc>>,
//!}
//!#[async_trait]
//!impl UpdatableModel<UpdatableItem, UpdateQuery, AppState> for UpdatableItem {
//!     async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<UpdatableItem> {
//!         // update in db
//!         let utc: DateTime<Utc> = Utc::now();
//!         self.updated_at = Some(utc);
//!         Ok(self)
//!     }
//!}
//!
//!#[actix_web::main]
//!async fn main() -> std::io::Result<()>{
//!     actix_web::HttpServer::new(|| {
//!         actix_web::App::new()
//!         .service(actix_web::web::scope(Item::scope()).configure(gen_endpoint!(Item, NewItem, UpdatableItem)))
//!         .data(AppState{})
//!     })
//!     .bind(("127.0.0.1", 8085))?
//!     .run()
//!     .await
//!}
//! ```

use actix_web::{web, HttpResponse};
use anyhow::Result;
use async_trait::async_trait;

/// A trait to implement on your main struct entity via the HttpFindListDelete derive macro :
///
/// ```
///
/// use actix_restful::{
///  HttpFindListDelete,
///  Model,
///  RestfulPathInfo
/// };
/// use actix_restful_derive::{HttpFindListDelete, actix_restful_info};
/// use anyhow::Result;
/// use async_trait::async_trait;
///
/// #[derive(HttpFindListDelete)]
/// #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
/// #[actix_restful_info(scope = "/v1", path = "item")]
/// struct Item {
/// }
///
/// #[async_trait]
/// impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Item {
///    async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<Item>> {
///    }
///    async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
///    }
///    async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
///    }
/// }
/// ```
#[async_trait]
pub trait Model<ID, FQ, LQ, LR, DQ, DR, AppState> {
    async fn find(id: ID, query: &FQ, state: &AppState) -> Result<Box<Self>>;
    async fn list(query: &LQ, state: &AppState) -> Result<LR>;
    async fn delete(self: Self, query: &DQ, state: &AppState) -> Result<DR>;
}

/// A trait to implement on your creatable entity entity
///
/// ```
///
/// use actix_restful::{
///  HttpCreate,
///  NewModel,
/// };
/// use actix_restful_derive::HttpCreate;
/// use anyhow::Result;
/// use async_trait::async_trait;
///
/// #[derive(HttpCreate)]
/// #[http_create(SaveQuery, AppState)]
/// struct NewItem {
/// }
///
/// #[async_trait]
/// impl NewModel<Item, SaveQuery, AppState> for NewItem {
///    async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<Item> {
///    }
/// }
/// ```
#[async_trait]
pub trait NewModel<T, Q, AppState> {
    async fn save(self: Self, query: &Q, state: &AppState) -> Result<T>;
}

/// A trait to implement on your Updatable entity
///
/// ```
///
/// use actix_restful::{
///  HttpUpdate,
///  UpdatableModel,
/// };
/// use actix_restful_derive::HttpUpdate;
/// use anyhow::Result;
/// use async_trait::async_trait;
///
/// #[derive(HttpUpdate)]
/// #[http_update(Id, UpdateQuery, Item, FindQuery, AppState)]
/// struct UpdatableItem {
/// }
///
/// #[async_trait]
/// impl UpdatableModel<UpdatableItem, UpdateQuery, AppState> for UpdatableItem {
///     async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<UpdatableItem> {
///     }
/// }
/// ```
#[async_trait]
pub trait UpdatableModel<T, Q, AppState> {
    async fn update(self: Self, query: &Q, state: &AppState) -> Result<T>;
}

/// This Trait is automatically implemented with the `actix_restful_derive::HttpCreate` derive macro

#[async_trait]
pub trait HttpCreate<Q, AppState> {
    /// This method is automaticaly implemented with the `actix_restful_derive::HttpCreate` derive macro
    async fn http_create(
        payload: web::Json<Box<Self>>,
        query: web::Query<Q>,
        app_state: web::Data<AppState>,
    ) -> Result<HttpResponse, HttpResponse>;
}

/// This Trait is automatically implemented with the `actix_restful_derive::HttpFindListDelete` derive macro

#[async_trait]
pub trait HttpFindListDelete<P, FQ, LQ, DQ, AppState> {
    /// This method is automatically implemented with the `actix_restful_derive::HttpFindListDelete` derive macro
    async fn http_find(
        info: web::Path<P>,
        query: web::Query<FQ>,
        app_state: web::Data<AppState>,
    ) -> Result<HttpResponse, HttpResponse>;
    /// This method is automatically implemented with the `actix_restful_derive::HttpFindListDelete` derive macro
    async fn http_list(
        query: web::Query<LQ>,
        app_state: web::Data<AppState>,
    ) -> Result<HttpResponse, HttpResponse>;
    /// This method is automatically implemented with the `actix_restful_derive::HttpFindListDelete` derive macro
    async fn http_delete(
        info: web::Path<P>,
        query: web::Query<DQ>,
        app_state: web::Data<AppState>,
    ) -> Result<HttpResponse, HttpResponse>;
}

/// This Trait is automaticaly implemented with the `actix_restful_derive::HttpUpdate` derive macro

#[async_trait]
pub trait HttpUpdate<P, Q, AppState> {
    /// This method is automaticaly implemented with the `actix_restful_derive::HttpUpdate` derive macro
    async fn http_update(
        info: web::Path<P>,
        payload: web::Json<Box<Self>>,
        query: web::Query<Q>,
        app_state: web::Data<AppState>,
    ) -> Result<HttpResponse, HttpResponse>;
}

pub trait RestfulPathInfo {
    fn path() -> String;
    fn scope() -> &'static str;
}

/// A macro to generate the http routes on the Actix app :
///
/// ```
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()>{
///    actix_web::HttpServer::new(|| {
///        actix_web::App::new()
///            .service(actix_web::web::scope(Item::scope()).configure(gen_endpoint!(Item, NewItem, UpdatableItem)))
///            .data(AppState{})
///    })
///        .bind(("127.0.0.1", 8085))?
///        .run()
///        .await
/// }
/// ```
///
/// If the attribute macro `actix_restful_info` is used with these parameters :
///
/// #[actix_restful_info(scope = "/v1", path = "item")]
///
///
/// The macro gen_endpoint! will generate 5 routes on the actix App :
///
/// - GET /v1/item/{id}
/// - GET /v1/item
/// - POST /v1/item
/// - PUT /v1/item/{id}
/// - DELETE /v1/item/{id}

#[macro_export]
macro_rules! gen_endpoint {
    ($model:ident, $new_model:ident, $updatable_model:ident) => {{
        let path = $model::path();
        use actix_web::web;
        move |cfg: &mut web::ServiceConfig| {
            cfg.route(
                "/{path}/{id}".replace("{path}", &path).as_str(),
                web::get().to($model::http_find),
            )
            .route(
                "/{path}/{id}".replace("{path}", &path).as_str(),
                web::delete().to($model::http_delete),
            )
            .route(
                "/{path}/{id}".replace("{path}", &path).as_str(),
                web::put().to($updatable_model::http_update),
            )
            .route(
                "/{path}".replace("{path}", &path).as_str(),
                web::get().to($model::http_list),
            )
            .route(
                "/{path}".replace("{path}", &path).as_str(),
                web::post().to($new_model::http_create),
            );
        }
    }};
}
