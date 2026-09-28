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

use version 0.2.x for actix web v4

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
            .service(web::scope(Project::scope()).configure(gen_endpoint!(Project, NewProject, UpdatableProject)))
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
            .service(apistos::web::scope(Project::scope()).configure(gen_documented_endpoint!(Project, NewProject, UpdatableProject)))
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

#### Examples :

Look into folder examples

The examples have a file called Insomnia.json which is a routing configuration file for [insomnia](https://insomnia.rest/)
