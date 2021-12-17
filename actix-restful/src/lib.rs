use async_trait::async_trait;
use actix_web::{ HttpResponse, web };
use anyhow::Result;

#[async_trait]
pub trait Model<FO, FP, FQ, AQ, AR, DQ, DR> {
    async fn find(id: FP, query: &FQ) -> Result<FO>;
    async fn all(query: &AQ) -> Result<AR>;
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
pub trait HttpAll<Q> {
    async fn http_all(query: web::Query<Q>) -> Result<HttpResponse, HttpResponse>;
}

#[async_trait]
pub trait HttpFind<P, Q> {
    async fn http_find(info: web::Path<P>, query: web::Query<Q>) -> Result<HttpResponse, HttpResponse>;
}


#[async_trait]
pub trait HttpDelete<P, Q> {
    async fn http_delete(info: web::Path<P>, query: web::Query<Q>) -> Result<HttpResponse, HttpResponse>;
}

#[async_trait]
pub trait HttpUpdate<P, Q> {
    async fn http_update(info: web::Path<P>, payload: web::Json<Box<Self>>, query: web::Query<Q>) -> Result<HttpResponse, HttpResponse>;
}
