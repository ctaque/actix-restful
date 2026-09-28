### Motivation

Building a Json Api for actix can be a lot of boilerplace code to write.
This project aims to simplify code generation for fast implementation of Json CRUD operations with Actix.

#### Contents

This workspace contains :

- `actix-restful`: the traits, the derive macros to implement on models (re-exported from `actix-restful-derive`), and the function macros to configure routes on the actix server,
- `actix-restful-cli`: a CLI, to generate base models.

### Installation

`actix-restful` is the only crate to depend on: it re-exports the derive macros, as well as `async_trait` and `anyhow` which the traits are declared with.

``` toml
[dependencies]
actix-restful = "0.6"
actix-web = "4"
serde = { version = "1", features = ["derive"] }
```

The model generator is installed once, as the `actix-restful` binary:

``` bash
cargo install actix-restful-cli
```

### Note in code version

use version 0.1.x for actix-web v3

use version 0.6.x for actix web v4

#### Declare Models

``` rust
// src/project.rs

use actix_restful::{
    HttpCreate, HttpFindListDelete, HttpUpdate, Model, NewModel, UpdatableModel,
    actix_restful_info, anyhow::Result, async_trait,
};
use serde::{Deserialize, Serialize};

// actix App State (https://actix.rs/docs/application/)
pub struct AppState {}


#[derive(Default, Deserialize)]
struct FindQuery {}
#[derive(Deserialize)]
struct ListQuery {}
#[derive(Deserialize)]
struct DeleteQuery {}
type ListResult = Vec<Project>;
type DeleteResult = Project;
type Id = i64;

#[derive(Default, Serialize, Deserialize, HttpFindListDelete)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[actix_restful_info(path = "project")]
struct Project {
    ...
}

#[derive(Deserialize)]
struct SaveQuery {}

#[derive(Serialize, Deserialize, HttpCreate)]
#[http_create(SaveQuery, AppState)]
struct NewProject {
    ...
}

#[derive(Deserialize)]
struct UpdateQuery {}

#[derive(Serialize, Deserialize, HttpUpdate)]
#[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
struct UpdatableProject {
    ...
}
```

#### Implement the traits methods on the models : 

``` rust

// src/project.rs

#[async_trait]
impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Project {
    async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<Project>> {
        // fetch from somwhere with id and return result
    }
    async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
        // list
    }
    async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
        // hard or soft delete
    }
}


#[async_trait]
// below, the Project type variable Project is the inner return type of the save function
impl NewModel<Project, SaveQuery, AppState> for NewProject {
    async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<Project> {
        // persist, and return Project entity
    }
}

#[async_trait]
// below, the type variable UpdatableProject is the inner return type of the update function.
impl UpdatableModel<UpdatableProject, UpdateQuery, AppState> for UpdatableProject {
    async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<UpdatableProject> {
        // update in db
    }
}
```

`

#### configures Restful routes with the function macro gen_endpoint!

``` rust
// src/main.rs

mod project;

use actix_restful::gen_endpoint;
use project::{ Project, NewProject, UpdatableProject, AppState };
use actix_web::web;

#[actix_web::main]
async fn main() -> std::io::Result<()>{
    actix_web::HttpServer::new(|| {
        actix_web::App::new()
            .service(actix_web::web::scope("/v1").configure(gen_endpoint!(Project, NewProject, UpdatableProject)))
            .app_data(web::Data::new(AppState{}))
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}

```

The macro gen_endpoint! will generate 5 routes :

- GET /v1/project/{id}
- GET /v1/project
- PUT /v1/project/{id}
- DELETE /v1/project/{id}
- POST /v1/project

If the updatable struct has an `id` field, `PUT /v1/project/{id}` answers `400 ID_MISMATCH` when the payload `id` differs from the path `{id}`.

#### OpenAPI documentation with apistos

With the `openapi` feature, the macro `gen_documented_endpoint!` generates the same 5 routes on an [apistos](https://docs.rs/apistos) app, which adds them to its OpenAPI 3.0 document.
Operations are tagged with the model path (`project`) and identified by the action and the path (`find_project`, `list_project`, `create_project`, `update_project`, `delete_project`).

``` toml
[dependencies]
actix-restful = { version = "0.6", features = ["openapi"] }
apistos = { version = "0.9", features = ["chrono", "swagger-ui"] }
# apistos relies on its fork of schemars
schemars = { package = "apistos-schemars", version = "0.8" }
```

The models, the query structs and the list / delete results must implement `apistos::ApiComponent`:

``` rust
use apistos::ApiComponent;
use schemars::JsonSchema;

#[derive(Deserialize, JsonSchema, ApiComponent)]
struct ListQuery {
    offset: Option<usize>,
}

/// Doc comments are used as schema descriptions
#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, HttpFindListDelete)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[actix_restful_info(path = "project")]
struct Project {
    ...
}
```

``` rust
use actix_restful::gen_documented_endpoint;
use apistos::app::{BuildConfig, OpenApiWrapper};
use apistos::spec::Spec;
use apistos::SwaggerUIConfig;

async fn main() -> std::io::Result<()>{
    actix_web::HttpServer::new(|| {
        actix_web::App::new()
            .document(Spec::default())
            .service(apistos::web::scope("v1").configure(gen_documented_endpoint!(Project, NewProject, UpdatableProject)))
            .app_data(web::Data::new(AppState{}))
            // serves the document on /openapi.json and Swagger UI on /swagger
            .build_with("/openapi.json", BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")))
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}
```

apistos parses route paths with a regex syntax introduced in `regex` 1.9, but accepts older versions: if the app panics with `path name regex`, run `cargo update -p regex`.

#### actix-restful-cli

Alternatively, if you want to avoid writing a lot of boilerplate code, you can use the model generator :

``` bash

actix-restful generate-model --name Project

```

This will generate a base model in `project.rs`, in the working directory (usually `src`), to declare with `mod project;`. Its structs are public, and it imports `AppState` from the root of the crate (`use crate::AppState;`): declare your application state in `main.rs` or `lib.rs`, or change this import. The generated file only relies on `actix-restful` and `serde`, plus `sqlx` and `chrono` with the options below.

With `--openapi`, the model, its query structs and its creatable / updatable structs also derive `JsonSchema` and `ApiComponent`, ready for `gen_documented_endpoint!` (see [OpenAPI documentation with apistos](#openapi-documentation-with-apistos)):

``` bash

actix-restful generate-model --name Project --openapi

```

With `--fields`, the CLI asks interactively for the name and type of each field (the type is picked by its number in the menu, or typed as any Rust type, and defaults to `String`; an empty name ends the input). The fields are added to `Project`, `NewProject` and `UpdatableProject`, next to the `id: Id` field that is always generated:

``` bash

actix-restful generate-model --name Project --fields
Enter the model fields (empty name to finish), `id: Id` is already declared
Wrap a type in `Option<T>` (e.g. `Option<i32>`) to make the field optional, its column is then nullable
Field name: title
  1) String  2) i32  3) i64  4) f64  5) bool  6) Option<String>  7) DateTime<Utc>  8) NaiveDateTime  9) NaiveDate  10) NaiveTime  11) Vec<u8>
Type of `title` (number or custom type) [String]:
Field name: stars
  1) String  2) i32  3) i64  4) f64  5) bool  6) Option<String>  7) DateTime<Utc>  8) NaiveDateTime  9) NaiveDate  10) NaiveTime  11) Vec<u8>
Type of `stars` (number or custom type) [String]: 2
Field name:
Successfully generated model project.rs, declare it with `mod project;`

```

With `--sqlx` (which requires `--fields`, and at least one field), the `find`, `list`, `delete`, `save` and `update` functions are filled with [sqlx](https://docs.rs/sqlx) queries on the `project` table, run on the `pool` field of your `AppState`. `Project` and `UpdatableProject` also derive `sqlx::FromRow`. The queries target SQLite by default, `--postgres` or `--mysql` target PostgreSQL or MySQL (`--sqlite` makes the default explicit). SQLite and PostgreSQL queries use `$N` placeholders and `RETURNING`, MySQL ones use `?` placeholders and select the row after the insert (with `last_insert_id()`) and the update, and before the delete:

``` bash

actix-restful generate-model --name Project --fields --sqlx

```

With `--migration` (which also requires `--fields`), the CLI creates the migration of the `project` table for the targeted database in `<timestamp>_create_project.sql` of the `migrations` folder next to `src` (the parent `migrations` folder when run from inside `src`), ready for `sqlx migrate run` or `sqlx::migrate!()`. Each field type is mapped to the column type sqlx declares for it (`<T as sqlx::Type<DB>>::type_info().name()`), so the generated columns always decode into the model fields, and `Option<T>` fields are nullable:

| Rust type | SQLite | PostgreSQL | MySQL |
|---|---|---|---|
| `String` | `TEXT` | `TEXT` | `VARCHAR(255)` |
| `i8` | `INTEGER` | | `TINYINT` |
| `i16` | `INTEGER` | `INT2` | `SMALLINT` |
| `i32` | `INTEGER` | `INT4` | `INT` |
| `i64` | `INTEGER` | `INT8` | `BIGINT` |
| `u8`, `u16`, `u32` | `INTEGER` | | `TINYINT UNSIGNED`, `SMALLINT UNSIGNED`, `INT UNSIGNED` |
| `u64` | | | `BIGINT UNSIGNED` |
| `f32` | `REAL` | `FLOAT4` | `FLOAT` |
| `f64` | `REAL` | `FLOAT8` | `DOUBLE` |
| `bool` | `BOOLEAN` | `BOOL` | `BOOLEAN` |
| `DateTime<Utc>` | `DATETIME` | `TIMESTAMPTZ` | `DATETIME(6)` |
| `NaiveDateTime` | `DATETIME` | `TIMESTAMP` | `DATETIME(6)` |
| `NaiveDate` | `DATE` | `DATE` | `DATE` |
| `NaiveTime` | `TIME` | `TIME` | `TIME(6)` |
| `Vec<u8>` | `BLOB` | `BYTEA` | `BLOB` |
| `Vec<String>`, `Vec<i16>`, `Vec<i32>`, `Vec<i64>`, `Vec<f64>`, `Vec<bool>` | | `TEXT[]`, `INT2[]`, `INT4[]`, `INT8[]`, `FLOAT8[]`, `BOOL[]` | |

The PostgreSQL names are the ones of sqlx, aliases of the standard types: `INT2`, `INT4` and `INT8` are `SMALLINT`, `INTEGER` and `BIGINT`, `FLOAT4` and `FLOAT8` are `REAL` and `DOUBLE PRECISION`, `BOOL` is `BOOLEAN`. On MySQL, four types differ from sqlx: `String` is a `VARCHAR(255)` (sqlx names a `VARCHAR` without a length, which is not a valid column type), `DateTime<Utc>` is a `DATETIME(6)` rather than a `TIMESTAMP` (sqlx writes the UTC date and time, stored as is by `DATETIME` where `TIMESTAMP` would convert it from the session time zone), and `NaiveDateTime` and `NaiveTime` keep their microseconds with `DATETIME(6)` and `TIME(6)`.

As the column types come from sqlx, the CLI is built with the SQLite, PostgreSQL and MySQL drivers of sqlx (it never connects to a database): its first build is longer, and needs a C compiler for the bundled SQLite.

The `id` column is `INTEGER PRIMARY KEY AUTOINCREMENT` on SQLite, `BIGSERIAL PRIMARY KEY` on PostgreSQL and `BIGINT AUTO_INCREMENT PRIMARY KEY` on MySQL. The chrono import is added to the model for the date types. With `--migration`, a type without a column type in the targeted database (like `Vec<String>` on SQLite) is refused when prompting for the fields:

``` bash

actix-restful generate-model --name Project --fields --sqlx --migration --postgres

```

With `--timestamps`, `Project` gets `created_at`, `updated_at` and `deleted_at` fields and `UpdatableProject` an `updated_at` field, all typed `Option<DateTime<Utc>>` from [chrono](https://docs.rs/chrono) (add `chrono` with its `serde` feature to your dependencies, and the `chrono` feature of sqlx or apistos when you use `--sqlx` or `--openapi`). With `--sqlx`, `save` sets `created_at` and `updated_at` to `Utc::now()`, `update` refreshes `updated_at`, and `delete` is a soft delete: it sets `deleted_at` to `Utc::now()` instead of removing the row, and `find`, `list` and `update` skip the rows whose `deleted_at` is set (a deleted row answers like a missing one). With `--migration`, the table gets nullable `created_at`, `updated_at` and `deleted_at` columns of the `DateTime<Utc>` column type:

``` bash

actix-restful generate-model --name Project --fields --sqlx --migration --timestamps

```

# Quick start :

Create the project and add the dependencies
```
cargo new my-api && cd my-api
cargo add actix-restful actix-restful-derive
cargo add actix-web anyhow async-trait
cargo add serde --features derive
cargo add sqlx --features runtime-tokio,sqlite,macros,migrate   # if persisting via sqlx
cargo add chrono --features serde                                # if --timestamps or date types
```

The code generated by the derives contains hard-coded paths, so actix_web, async_trait, anyhow and serde must be direct dependencies of the project, even if the user doesn't use them directly.

For OpenAPI, you also need:

```
cargo add actix-restful --features openapi
cargo add apistos --features chrono,swagger-ui
cargo add schemars --rename schemars --package apistos-schemars   # or by hand in Cargo.toml
```
Install the CLI (once per machine)
```
cargo install actix-restful-cli     # installs the `actix-restful` binary
cargo install sqlx-cli               # optional, for `sqlx migrate run`
```
Generate a model
```
cd src
actix-restful generate-model --name Project --fields --sqlx --migration --timestamps
Field name: title        Type [String]:
Field name: stars        Type [String]: i32
Field name: optionnal    Type [String]: Option<String>
```

This command produces:

src/Project.rs: the Project, NewProject and UpdatableProject structs, the query structs, and the Model, NewModel and UpdatableModel implementations filled in with sqlx queries;
```migrations/<timestamp>_create_project.sql.```

Wire the model into main.rs

```rust
#[path = "Project.rs"]
mod project;
#[path = "helpers.rs"]
mod shared; // shared module must export the AppState struct which is in turn imported into the generated models files
use actix_restful::{gen_endpoint, RestfulPathInfo};
use actix_web::{web, App, HttpServer};
use project::{Project, NewProject, UpdatableProject};
use sqlx::SqlitePool;
 


pub struct AppState {
    pub pool: SqlitePool,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = SqlitePool::connect("sqlite://data.db?mode=rwc").await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let state = web::Data::new(shared::AppState { pool });

    actix_web::HttpServer::new(move || {
        let spec = Spec {
            info: Info {
                title: "MyApp REST API".to_string(),
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
                  .configure(project::configure) // Where the magic operates
            )
            .app_data(state.clone())
            .build_with(
                "/openapi.json",
                BuildConfig::default().with(SwaggerUIConfig::new(&"/swagger")),
            )
    })
    .bind(("127.0.0.1", 8085))?
    .run()
    .await
}
```

#### Examples :

Look into folder examples

The examples have routing configuration file for [hoppscotch](https://hoppscotch.io/)
