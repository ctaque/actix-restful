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
        /// Derives JsonSchema and ApiComponent on the model types, for `gen_documented_endpoint!`
        #[structopt(long = "openapi")]
        openapi: bool,
    }
}

const OPENAPI_IMPORTS: &str = r#"
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use actix_restful::gen_documented_endpoint;"#;

const OPENAPI_CONFIGURE: &str = r#"

    // Registers the documented routes of the {entity_lower_case} endpoint
    // (actix-restful `openapi` feature), to mount with `.configure({entity_lower_case}::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!({entity}, New{entity}, Updatable{entity})(cfg)
    }
"#;

const OPENAPI_DERIVES: &str = ", JsonSchema, ApiComponent";

fn render_model(name: &str, openapi: bool) -> String {
    let (imports, derives, configure) = if openapi {
        (OPENAPI_IMPORTS, OPENAPI_DERIVES, OPENAPI_CONFIGURE)
    } else {
        ("", "", "")
    };
    MODEL_TPL
        .replace("{openapi_imports}", imports)
        .replace("{openapi_derives}", derives)
        .replace("{openapi_configure}", configure)
        .replace("{entity}", name)
        .replace("{entity_lower_case}", &name.to_lowercase())
}

const MODEL_TPL: &str = r#"
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
    use serde_json;{openapi_imports}

    #[derive(Default, Deserialize{openapi_derives})]
    struct FindQuery {}
    #[derive(Deserialize{openapi_derives})]
    struct ListQuery {}
    #[derive(Deserialize{openapi_derives})]
    struct DeleteQuery {}
    type ListResult = Vec<{entity}>;
    type DeleteResult = {entity};
    #[derive(Deserialize{openapi_derives})]
    struct SaveQuery {}
    #[derive(Deserialize{openapi_derives})]
    struct UpdateQuery {}
    type Id = i64;

    #[derive(Default, Serialize, Deserialize{openapi_derives}, HttpFindListDelete)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[actix_restful_info(path = "{entity_lower_case}")]
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
    
    #[derive(Serialize, Deserialize{openapi_derives}, HttpCreate)]
    #[http_create(SaveQuery, AppState)]
    struct New{entity} {

    }
    #[async_trait]
    impl NewModel<{entity}, SaveQuery, AppState> for New{entity} {
        async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<{entity}> {
            // persist
        }
    }
    
    #[derive(Serialize, Deserialize{openapi_derives}, HttpUpdate)]
    #[http_update(Id, UpdateQuery, {entity}, FindQuery, AppState)]
    struct Updatable{entity} {
        id: Id,
    }
    #[async_trait]
    impl UpdatableModel<Updatable{entity}, UpdateQuery, AppState> for Updatable{entity} {
        async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<Updatable{entity}> {
            // update in db
        }
    }{openapi_configure}
    "#;

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    match opt {
        Opt::GenerateModel { name, openapi } => {
            let to_write = render_model(&name, openapi);
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

#[cfg(test)]
mod tests {
    use super::render_model;

    #[test]
    fn default_model_has_no_openapi_derives() {
        let model = render_model("Project", false);
        assert!(!model.contains("JsonSchema"));
        assert!(!model.contains("ApiComponent"));
        assert!(!model.contains("fn configure"));
        assert!(!model.contains("{openapi"));
        assert!(model.contains("#[actix_restful_info(path = \"project\")]"));
    }

    #[test]
    fn openapi_model_derives_schemas_on_every_route_type() {
        let model = render_model("Project", true);
        assert!(model.contains("use apistos::ApiComponent;"));
        assert!(model.contains("use schemars::JsonSchema;"));
        assert!(model.contains("use actix_restful::gen_documented_endpoint;"));
        assert!(model.contains("pub fn configure(cfg: &mut apistos::web::ServiceConfig) {"));
        assert!(model.contains("gen_documented_endpoint!(Project, NewProject, UpdatableProject)(cfg)"));
        // 5 query structs + Project, NewProject and UpdatableProject
        assert_eq!(model.matches(", JsonSchema, ApiComponent").count(), 8);
        assert!(!model.contains("{openapi"));
        assert!(!model.contains("{entity"));
    }
}
