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
            .data(AppState{})
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

#### actix-restful-cli

Alternatively, if you want to avoid writing a lot of boilerplate code, you can use the model generator :

``` bash

actix-restful generate-model --name Project

```

This wil generate a base model of name Project.rs at path ./Project.rs

#### Examples :

Look into folder examples

The examples have a file called Insomnia.json which is a routing configuration file for [insomnia](https://insomnia.rest/)
