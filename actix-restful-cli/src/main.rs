use structopt::StructOpt;
use std::fs::File;
use std::io::{Write, Error};

#[derive(Debug, StructOpt)]
#[structopt(name = "actix-restful")]
pub enum Opt {
    #[structopt(name = "generate-model")]
    GenerateModel {
        #[structopt(short = "n", long = "name")]
        name: String,
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
        RestfulPathInfo
    };
    use actix_restful_derive::{HttpCreate, HttpFindListDelete, HttpUpdate, actix_restful_info};

    use anyhow::Result;
    use async_trait::async_trait;
    use std::default::Default;
    use actix_web;
    use serde_json;
    
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
    #[actix_restful_info(scope = "/v1", path = "{entity_lower_case}")]
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
        Opt::GenerateModel { name } => {
            let to_write = model_tpl.replace("{entity}", &name)
                .replace("{entity_lower_case}", &name.to_lowercase());
            let mut path = String::from("");
            path.push_str(&name);
            path.push_str(".rs");
            let mut output = File::create(path.clone())?;
            write!(output, "{}", to_write)?;
            println!("Successfully generated model {}", path);
            Ok(())
        }
    }
}
