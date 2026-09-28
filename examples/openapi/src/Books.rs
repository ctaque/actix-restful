
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
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use actix_restful::gen_documented_endpoint;

    #[derive(Default, Deserialize, JsonSchema, ApiComponent)]
    struct FindQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct ListQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct DeleteQuery {}
    type ListResult = Vec<Books>;
    type DeleteResult = Books;
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct SaveQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct UpdateQuery {}
    type Id = i64;

    #[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, HttpFindListDelete)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[actix_restful_info(path = "books")]
    struct Books {
        id: Id,
    }
    
    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Books {
        async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<Books>> {
            // fetch from somwhere with id
        }
        async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
            // list
        }
        async fn delete(mut self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
            // hard or soft delete
        }
    }
    
    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate)]
    #[http_create(SaveQuery, AppState)]
    struct NewBooks {

    }
    #[async_trait]
    impl NewModel<Books, SaveQuery, AppState> for NewBooks {
        async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<Books> {
            // persist
        }
    }
    
    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpUpdate)]
    #[http_update(Id, UpdateQuery, Books, FindQuery, AppState)]
    struct UpdatableBooks {
        id: Id,
    }
    #[async_trait]
    impl UpdatableModel<UpdatableBooks, UpdateQuery, AppState> for UpdatableBooks {
        async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<UpdatableBooks> {
            // update in db
        }
    }

    // Registers the documented routes of the books endpoint
    // (actix-restful `openapi` feature), to mount with `.configure(books::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Books, NewBooks, UpdatableBooks)(cfg)
    }

    
