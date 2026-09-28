### Disclaimer

This project is still a WIP and not yet published on crates.io

### Motivation

Building a Json Api for actix can be a lot of boilerplace code to write.
This project aims to simplify code generation for fast implementation of Json apis for Actix.

#### Contents

This workspace contains :

- A CLI, to generate base models,
- Derive macros to implement on models,
- A function macro to configure routes on the actix server

### Note in code version

use version 0.1.x for actix-web v3

use version 0.6.x for actix web v4

#### Declare Models

``` rust
// src/models/Project.rs

// actix App State (https://actix.rs/docs/application/)
struct AppState {}


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
#[actix_restful_info(scope = "/v1", path = "project")]
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

// src/models/Project.rs

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

use actix_restful::gen_endpoint;
use models::{ Project, NewProject, UpdatableProject, AppState };
use actix_web::web;

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
actix-restful = { version = "0.2", features = ["openapi"] }
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
#[actix_restful_info(scope = "/v1", path = "project")]
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

This wil generate a base model of name Project.rs at path ./Project.rs

With `--openapi`, the model, its query structs and its creatable / updatable structs also derive `JsonSchema` and `ApiComponent`, ready for `gen_documented_endpoint!` (see [OpenAPI documentation with apistos](#openapi-documentation-with-apistos)):

``` bash

actix-restful generate-model --name Project --openapi

```

With `--fields`, the CLI asks interactively for the name and type of each field (the type defaults to `String`, an empty name ends the input). The fields are added to `Project`, `NewProject` and `UpdatableProject`, next to the `id: Id` field that is always generated:

``` bash

actix-restful generate-model --name Project --fields
Enter the model fields (empty name to finish), `id: Id` is already declared
Wrap a type in `Option<T>` (e.g. `Option<i32>`) to make the field optional, its column is then nullable
Field name: title
Type of `title` [String]:
Field name: stars
Type of `stars` [String]: i32
Field name:
Successfully generated model Project.rs

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

#### Examples :

Look into folder examples

The examples have routing configuration file for [hoppscotch](https://hoppscotch.io/)
