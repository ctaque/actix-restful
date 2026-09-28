#[path = "Model1.rs"]
mod model1;
#[path = "Model2.rs"]
mod model2;
#[path = "Model3.rs"]
mod model3;
#[path = "helpers.rs"]
mod shared;

use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::info::Info;
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use sqlx::postgres::PgPoolOptions;
use std::default::Default;
use actix_web;


// The OpenAPI document is served on /openapi.json, and browsable on /swagger
#[actix_web::main]
async fn main() -> anyhow::Result<()>{
    // The PostgreSQL database is migrated on launch
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set, e.g. postgres://user:password@localhost:5432/db"))?;
    let pool = PgPoolOptions::new().connect(&database_url).await?;
    sqlx::migrate!().run(&pool).await?;

    // One pool shared by every worker
    let state = actix_web::web::Data::new(shared::AppState { pool });

    actix_web::HttpServer::new(move || {
        let spec = Spec {
            info: Info {
                title: "MyApp API".to_string(),
                version: "1.0.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        actix_web::App::new()
            .document(spec)
            // Actix does not fall through between scopes sharing a prefix,
            // so resources living under the same scope must be registered together
            .service(
                apistos::web::scope("v1")
                  .configure(model1::configure)
                  .configure(model2::configure)
                  .configure(model3::configure)
            )
            .app_data(state.clone())
            .build_with(
                "/openapi.json",
                BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")),
            )
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await?;
    Ok(())
}
