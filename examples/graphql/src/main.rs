//                         -----
//                     -------------
//                   -----  ----------
//                  ---  --------------
//                 ---  ----------------
//                 --- -----------------
//                 --- -----------------
//                 --- -----------------
//                 ---------------------
//       -----      -------------------       -----
//      -------      --  ---------- --      -------
//          ----      ---------------      -----
//           ---      ---------------      ----
//          ----     -----------------     ----
//        ------   ----------------------   ------
//    --------  ---------------------------  --------
//   ------   -------------------------- ----   -------
//  ----    -----  ---------- --- ------- -----    -----
// ----  ------  -------- --- --- ---- ---  ------  ----
// ----        ---- ----  --- ---- ---- -----       ----
//  ----   ------  ----  ---- ----  ----   ------  -----
//  ------      ------   ---- -----  ------      ------
//    ---------------    ----  ----    --------------
//      ----------       ----  ----      ----------
//                       ----  ----
//                 ---   ---- -----   --
//               ------  ---- ----- -------
//              -------  ---- ----- --------
//              ----    ----   -----    ----
//              -----------     -----------
//               ---------        --------

#![recursion_limit = "512"]

mod page;
mod author;
mod author_books;
mod book;
mod helpers;
use actix_web::web;
use helpers::AppState;
use sqlx::SqlitePool;
use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptySubscription, MergedObject, Object, Schema};
use async_graphql_actix_web::GraphQL;

// The version of the API, so that the query root has a field before the first model is merged
#[derive(Default)]
struct ApiQuery;

#[Object]
impl ApiQuery {
    async fn api_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

// The GraphQL queries, `octopux generate-model --graphql` merges the query root of the model here:
// `struct Query(ApiQuery, <model_name>::<Model>Query, ...);`
#[derive(MergedObject, Default)]
struct Query(ApiQuery, author::AuthorQuery, book::BookQuery, page::PageQuery);

// The GraphQL mutations, GraphQL refusing a mutation root without fields, `octopux generate-model --graphql` replaces it with:
// `#[derive(MergedObject, Default)] struct Mutation(<model_name>::<Model>Mutation, ...);`
#[derive(MergedObject, Default)]
struct Mutation(author::AuthorMutation, book::BookMutation, page::PageMutation);

async fn graphiql() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/graphql").finish())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = SqlitePool::connect("sqlite://data.db?mode=rwc").await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let state = web::Data::new(AppState { pool });
    // The resolvers of the models read the state from the data of the schema
    let schema = Schema::build(Query::default(), Mutation::default(), EmptySubscription)
        .data(state.clone())
        .finish();

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            // Actix does not fall through between scopes sharing a prefix,
            // so resources living under the same scope must be registered together
            .service(
                web::scope("v1") // Where the magic operates
                    .configure(author::configure)
                    .configure(author_books::configure)
                    .configure(book::configure),
            )
            .app_data(state.clone())
            // GraphQL queries on POST /graphql, GraphiQL on GET /graphql
            .service(
                web::resource("/graphql")
                    .route(web::post().to(GraphQL::new(schema.clone())))
                    .route(web::get().to(graphiql)),
            )
    })
    .bind(("127.0.0.1", 8085))?
    .run()
    .await
}

// Exercises the code generated with --graphql: the schema of --bootstrap, the resolvers of
// generate-model and the field of generate-relation, through POST /graphql as a client would
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;
    use serde_json::{json, Value};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn app() -> impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error> {
        // a single connection, each connection to `sqlite::memory:` opening its own database
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = web::Data::new(AppState { pool });
        let schema = Schema::build(Query::default(), Mutation::default(), EmptySubscription)
            .data(state.clone())
            .finish();
        test::init_service(
            actix_web::App::new()
                .service(
                    web::scope("v1")
                        .configure(author::configure)
                        .configure(author_books::configure)
                        .configure(book::configure)
                        .configure(page::configure),
                )
                .app_data(state.clone())
                .service(
                    web::resource("/graphql")
                        .route(web::post().to(GraphQL::new(schema.clone())))
                        .route(web::get().to(graphiql)),
                ),
        )
        .await
    }

    async fn graphql(app: &impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>, query: &str) -> Value {
        let req = test::TestRequest::post().uri("/graphql").set_json(json!({ "query": query })).to_request();
        test::call_and_read_body_json(app, req).await
    }

    #[actix_web::test]
    async fn the_schema_serves_the_api_version_and_graphiql() {
        let app = app().await;
        assert_eq!(graphql(&app, "{ apiVersion }").await, json!({ "data": { "apiVersion": "0.0.0" } }));
        let res = test::call_service(&app, test::TestRequest::get().uri("/graphql").to_request()).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn models_are_created_found_listed_updated_and_deleted() {
        let app = app().await;
        let created = graphql(&app, r#"mutation { createAuthor(input: { name: "Ursula" }) { id name } }"#).await;
        assert_eq!(created, json!({ "data": { "createAuthor": { "id": 1, "name": "Ursula" } } }));
        graphql(&app, r#"mutation { createAuthor(input: { name: "Frank" }) { id } }"#).await;

        assert_eq!(graphql(&app, "{ author(id: 1) { name } }").await, json!({ "data": { "author": { "name": "Ursula" } } }));
        assert_eq!(
            graphql(&app, "{ authors(offset: 1, limit: 1) { name } }").await,
            json!({ "data": { "authors": [{ "name": "Frank" }] } })
        );

        let updated = graphql(&app, r#"mutation { updateAuthor(input: { id: 1, name: "Ursula K. Le Guin" }) { id name } }"#).await;
        assert_eq!(updated, json!({ "data": { "updateAuthor": { "id": 1, "name": "Ursula K. Le Guin" } } }));

        let deleted = graphql(&app, "mutation { deleteAuthor(id: 2) { name } }").await;
        assert_eq!(deleted, json!({ "data": { "deleteAuthor": { "name": "Frank" } } }));
        assert_eq!(graphql(&app, "{ authors { id } }").await, json!({ "data": { "authors": [{ "id": 1 }] } }));
    }

    #[actix_web::test]
    async fn missing_models_are_not_found() {
        let app = app().await;
        for query in [
            "{ author(id: 42) { id } }",
            r#"mutation { updateAuthor(input: { id: 42, name: "Nobody" }) { id } }"#,
            "mutation { deleteAuthor(id: 42) { id } }",
        ] {
            let res = graphql(&app, query).await;
            assert_eq!(res["errors"][0]["message"], "ENTITY_NOT_FOUND", "{}", query);
        }
    }

    #[actix_web::test]
    async fn the_relation_resolves_the_paginated_children_of_the_parent() {
        let app = app().await;
        graphql(&app, r#"mutation { createAuthor(input: { name: "Ursula" }) { id } }"#).await;
        graphql(&app, r#"mutation { createAuthor(input: { name: "Frank" }) { id } }"#).await;
        for (title, author_id) in [("The Dispossessed", 1), ("Dune", 2), ("The Lathe of Heaven", 1), ("Earthsea", 1)] {
            graphql(&app, &format!(r#"mutation {{ createBook(input: {{ title: "{}", authorId: {} }}) {{ id }} }}"#, title, author_id)).await;
        }

        assert_eq!(
            graphql(&app, "{ author(id: 1) { name books { title } } }").await,
            json!({ "data": { "author": { "name": "Ursula", "books": [
                { "title": "The Dispossessed" }, { "title": "The Lathe of Heaven" }, { "title": "Earthsea" }
            ] } } })
        );
        assert_eq!(
            graphql(&app, "{ author(id: 1) { books(offset: 1, limit: 1) { title } } }").await,
            json!({ "data": { "author": { "books": [{ "title": "The Lathe of Heaven" }] } } })
        );
        assert_eq!(
            graphql(&app, "{ authors { name books { title } } }").await["data"]["authors"][1],
            json!({ "name": "Frank", "books": [{ "title": "Dune" }] })
        );

        // the REST routes are still served next to the schema, on the same state
        let req = test::TestRequest::get().uri("/v1/author/1/books").to_request();
        let books: Value = test::call_and_read_body_json(&app, req).await;
        assert_eq!(books.as_array().unwrap().len(), 3);
    }
}
