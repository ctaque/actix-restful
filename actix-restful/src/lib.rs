use async_trait::async_trait;
use actix_web::{ HttpResponse, web };
use anyhow::Result;

#[async_trait]
pub trait Model<ID, FQ, LQ, LR, DQ, DR, AppState> {
    async fn find(id: ID, query: &FQ, state: &AppState) -> Result<Box<Self>>;
    async fn list(query: &LQ, state: &AppState) -> Result<LR>;
    async fn delete(self: Self, query: &DQ, state: &AppState) -> Result<DR>;
}

#[async_trait]
pub trait NewModel<T, Q, AppState> {
    async fn save(self: Self, query: &Q, state: &AppState) -> Result<T>;
}

#[async_trait]
pub trait UpdatableModel<T, Q, AppState> {
    async fn update(self: Self, query: &Q, state: &AppState) -> Result<T>;
}

#[async_trait]
pub trait HttpCreate<Q, AppState> {
    async fn http_create(payload: web::Json<Box<Self>>, query: web::Query<Q>, app_state: web::Data<AppState>) -> Result<HttpResponse, HttpResponse>;
}

#[async_trait]
pub trait HttpFindListDelete<P, FQ, LQ, DQ, AppState> {
    async fn http_find(info: web::Path<P>, query: web::Query<FQ>, app_state: web::Data<AppState>) -> Result<HttpResponse, HttpResponse>;
    async fn http_list(query: web::Query<LQ>, app_state: web::Data<AppState>) -> Result<HttpResponse, HttpResponse>;
    async fn http_delete(info: web::Path<P>, query: web::Query<DQ>, app_state: web::Data<AppState>) -> Result<HttpResponse, HttpResponse>;
}

#[async_trait]
pub trait HttpUpdate<P, Q, AppState> {
    async fn http_update(info: web::Path<P>, payload: web::Json<Box<Self>>, query: web::Query<Q>, app_state: web::Data<AppState>) -> Result<HttpResponse, HttpResponse>;
}



#[macro_export]
macro_rules! gen_endpoint {
    ($entity:expr, $model:ident, $new_model:ident, $updatable_model:ident) => {
        {
            use actix_web::web;
            move | cfg: &mut web::ServiceConfig | {
                cfg.route("/{entity}/{id}".replace("{entity}", $entity).as_str(), web::get().to($model::http_find))
                    .route("/{entity}/{id}".replace("{entity}", $entity).as_str(), web::delete().to($model::http_delete))
                    .route("/{entity}/{id}".replace("{entity}", $entity).as_str(), web::put().to($updatable_model::http_update))
                    .route("/{entity}".replace("{entity}", $entity).as_str(), web::get().to($model::http_list))
                    .route("/{entity}".replace("{entity}", $entity).as_str(), web::post().to($new_model::http_create));
            }
        }
    };
}
