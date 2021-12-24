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

actix-restful generate-model --entity Project

```

#### configures Restful routes with the function macro gen_endpoint!

``` rust

use actix_restful::gen_endpoint;
use models::{ Project, NewProject, UpdatableProject };

struct AppState{};

async fn main() -> std::io::Result<()>{
    actix_web::HttpServer::new(|| {
        actix_web::App::new()
            .configure(gen_endpoint!("project", Project, NewProject, UpdatableProject))
            .data(AppState{})
    })
        .bind(("127.0.0.1", 8085))?
        .run()
        .await
}

```

this macro will generate 5 endpoints :

- GET /project/{id}
- GET /project
- PUT /project/{id}
- DELETE /project/{id}
- POST /project


#### Examples :

Look into folder examples
The examples have a file called Insomnia.json which is a routing configuration file for [insomnia](https://insomnia.rest/)