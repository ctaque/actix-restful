use actix_web::web;
use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;
use sqlx::postgres::PgPoolOptions;
#[path = "helpers.rs"]
mod shared;
#[path = "Project.rs"]
mod project;
#[path = "Category.rs"]
mod category;
#[path = "ProjectCategory.rs"]
mod projectcategory;
#[path = "Book.rs"]
mod book;
mod project_books;
mod project_project_categories;
mod book_book_pages;
mod book_page;
mod user;
mod tags;
mod project_tags;
mod project_project_tags;
mod project_licences;
mod project_licence;
mod licence;
mod licence_projects;
mod user_projects;




#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok(); // loads .env if present, never overrides existing vars
    //

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| std::io::Error::other("DATABASE_URL must be set, e.g. postgres://user:password@localhost:5432/db"))?;
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .map_err(std::io::Error::other)?;
    sqlx::migrate!().run(&pool).await.map_err(std::io::Error::other)?;

    // One pool shared by every worker
    let state = web::Data::new(shared::AppState { pool });

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .document(Spec::default())
            .service(apistos::web::scope("v1")
                .configure(project::configure)
                .configure(category::configure)
                .configure(projectcategory::configure)
                .configure(book::configure)
                .configure(project_books::configure)
                .configure(project_project_categories::configure)
                .configure(book_book_pages::configure)
                .configure(book_page::configure)
                .configure(user::configure)
                .configure(tags::configure)
                .configure(project_tags::configure)
                .configure(project_project_tags::configure)
                .configure(licence::configure)
                .configure(project_licences::configure)
                .configure(project_licence::configure)
                .configure(licence_projects::configure)
                .configure(user_projects::configure)

            )
            .app_data(state.clone())
            // serves the document on /openapi.json and Swagger UI on /swagger
            .build_with("/openapi.json", BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")))
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}
