
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
    type ListResult = Vec<Test>;
    type DeleteResult = Test;
    #[derive(Deserialize)]
    struct SaveQuery {}
    #[derive(Deserialize)]
    struct UpdateQuery {}
    type Id = i64;
    
    #[derive(Default, Serialize, Deserialize, HttpFindListDelete)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[actix_restful_info(scope = "/v1", path = "test")]
    struct Test {
        id: Id,
    }
    
    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Test {
        async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<Test>> {
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
    struct NewTest {

    }
    #[async_trait]
    impl NewModel<Test, SaveQuery, AppState> for NewTest {
        async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<Test> {
            // persist
        }
    }
    
    #[derive(Serialize, Deserialize, HttpUpdate)]
    #[http_update(Id, UpdateQuery, Test, FindQuery, AppState)]
    struct UpdatableTest {
        id: Id,
    }
    #[async_trait]
    impl UpdatableModel<UpdatableTest, UpdateQuery, AppState> for UpdatableTest {
        async fn update(mut self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<UpdatableTest> {
            // update in db
        }
    }
    