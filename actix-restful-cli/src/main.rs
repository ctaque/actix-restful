use structopt::StructOpt;
use std::fs::File;
use std::io::{Write, Error};

#[derive(Debug, StructOpt)]
#[structopt(name = "actix-restful")]
pub enum Opt {
    #[structopt(name = "generate-model")]
    GenerateModel {
        #[structopt(short = "e", long = "entity")]
        entity: String,
    }
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    let model_tpl = r#"
    use serde::{Serialize, Deserialize};
    use actix_restful::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        Model,
        NewModel,
        UpdatableModel,
        gen_endpoint
    };
    use actix_restful_derive::{HttpCreate, HttpFindListDelete, HttpUpdate};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::default::Default;
    use actix_web;
    use serde_json;
    
    use chrono::prelude::*;
    
    #[derive(Default, Deserialize)]
    struct FindQuery {}
    #[derive(Deserialize)]
    struct ListQuery {}
    #[derive(Deserialize)]
    struct DeleteQuery {}
    type ListResult = Vec<{entity}>;
    type DeleteResult = {entity};
    #[derive(Deserialize)]
    struct SaveQuery {}
    #[derive(Deserialize)]
    struct UpdateQuery {}
    type Id = i64;
    
    #[derive(Default, Serialize, Deserialize, HttpFindListDelete)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    struct {entity} {
        id: Id,
    }
    
    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for {entity} {
        async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<{entity}>> {
            // fetch from somwhere with id
        }
        async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
            // list
        }
        async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
            // hard or soft delete
        }
    }
    
    #[derive(Serialize, Deserialize, HttpCreate)]
    #[http_create(SaveQuery, AppState)]
    struct New{entity} {
        content: String,
    }
    #[async_trait]
    impl NewModel<{entity}, SaveQuery, AppState> for New{entity} {
        async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<{entity}> {
            // persist
        }
    }
    
    #[derive(Serialize, Deserialize, HttpUpdate)]
    #[http_update(Id, UpdateQuery, {entity}, FindQuery, AppState)]
    struct Updatable{entity} {
        id: Id,
    }
    #[async_trait]
    impl UpdatableModel<Updatable{entity}, UpdateQuery, AppState> for Updatable{entity} {
        async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<Updatable{entity}> {
            // update in db
        }
    }
    "#;
    match opt {
        Opt::GenerateModel { entity } => {
            let to_write = model_tpl.replace("{entity}", &entity);
            let mut path = String::from("");
            path.push_str(&entity);
            path.push_str(".rs");
            let mut output = File::create(path.clone())?;
            write!(output, "{}", to_write)?;
            println!("Successfully generated model {}", path);
            Ok(())
        }
    }
}
