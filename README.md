This project is still a WIP and not yet published on crates.io

### Motivation

Building a Json Api for actix can be a lot of boilerplace code to write.
This project aims to simplify code generation for fast implementation of Json apis for Actix.

#### Contents

This workspace contains :

- A CLI, to generate base models,
- Derive macros to implement on models,
- A function macro to configure routes on the actix server

#### actix-restful-cli

generate base models :

``` bash
# generate a base model of name Project.rs at path ./Project.rs

actix-restful generate-model --name Project

```

#### Implement the traits methods on the models : 

``` rust

// models/Project.rs

#[async_trait]
impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Project {
    async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<Item>> {
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
impl NewModel<Project, SaveQuery, AppState> for NewProject {
    async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<Project> {
        // persist, and return Item entity
    }
}

#[async_trait]
impl UpdatableModel<UpdatableItem, UpdateQuery, AppState> for UpdatableProject {
    async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<UpdatableItem> {
        // update in db
    }
}
```

`

#### configures Restful routes with the function macro gen_endpoint!

``` rust

use actix_restful::gen_endpoint;
use models::{ Project, NewProject, UpdatableProject };
use actix_web::web;

struct AppState{};

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

this macro will generate 5 endpoints :

- GET /v1/project/{id}
- GET /v1/project
- PUT /v1/project/{id}
- DELETE /v1/project/{id}
- POST /v1/project


#### Examples :

Look into folder examples

The examples have a file called Insomnia.json which is a routing configuration file for [insomnia](https://insomnia.rest/)
