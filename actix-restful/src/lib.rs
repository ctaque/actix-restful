use async_trait::async_trait;
use actix_web::{ HttpResponse, web };
use anyhow::Result;

#[async_trait]
pub trait Model<ID, FQ, LQ, LR, DQ, DR> {
    async fn find(id: ID, query: &FQ) -> Result<Box<Self>>;
    async fn list(query: &LQ) -> Result<LR>;
    async fn delete(self: Self, query: &DQ) -> Result<DR>;
}

#[async_trait]
pub trait NewModel<T, Q> {
    async fn save(self: Self, query: &Q) -> Result<T>;
}

#[async_trait]
pub trait UpdatableModel<T, Q> {
    async fn update(self: Self, query: &Q) -> Result<T>;
}

#[async_trait]
pub trait HttpCreate<Q> {
    async fn http_create(payload: web::Json<Box<Self>>, query: web::Query<Q>) -> Result<HttpResponse, HttpResponse>;
}

#[async_trait]
pub trait HttpFindListDelete<P, FQ, LQ, DQ> {
    async fn http_find(info: web::Path<P>, query: web::Query<FQ>) -> Result<HttpResponse, HttpResponse>;
    async fn http_list(query: web::Query<LQ>) -> Result<HttpResponse, HttpResponse>;
    async fn http_delete(info: web::Path<P>, query: web::Query<DQ>) -> Result<HttpResponse, HttpResponse>;
}

#[async_trait]
pub trait HttpUpdate<P, Q> {
    async fn http_update(info: web::Path<P>, payload: web::Json<Box<Self>>, query: web::Query<Q>) -> Result<HttpResponse, HttpResponse>;
}
