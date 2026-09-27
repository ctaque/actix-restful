use serde::{Serialize, Deserialize};
use actix_restful::{
    HttpCreate,
    HttpFindListDelete,
    HttpUpdate,
    Model,
    NewModel,
    UpdatableModel,
    gen_documented_endpoint,
    RestfulPathInfo
};
use actix_restful_derive::{HttpCreate, HttpFindListDelete, HttpUpdate, actix_restful_info};
use anyhow::Result;
use apistos::ApiComponent;
use async_trait::async_trait;
use schemars::JsonSchema;
use std::default::Default;
use actix_web;

use crate::shared::AppState;

#[derive(Default, Deserialize, JsonSchema, ApiComponent)]
struct FindQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct ListQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct DeleteQuery {}
type ListResult = Vec<Project>;
type DeleteResult = Project;
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct SaveQuery {}
#[derive(Deserialize, JsonSchema, ApiComponent)]
struct UpdateQuery {}
type Id = i64;

/// A project
#[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, HttpFindListDelete)]
#[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
#[actix_restful_info(path = "project")]
pub struct Project {
    id: Id,
    content: String,
}

#[async_trait]
impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Project {
    async fn find(id: Id, _query: &FindQuery, _state: &AppState) -> Result<Box<Project>> {
        Ok(Box::new(Project { id, content: String::from("Blah") }))
    }
    async fn list(_query: &ListQuery, _state: &AppState) -> Result<ListResult> {
        Ok(Vec::new())
    }
    async fn delete(self: Self, _query: &DeleteQuery, _state: &AppState) -> Result<DeleteResult> {
        Ok(self)
    }
}

/// The payload to create a project
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate)]
#[http_create(SaveQuery, AppState)]
struct NewProject {
    content: String,
}
#[async_trait]
impl NewModel<Project, SaveQuery, AppState> for NewProject {
    async fn save(self: Self, _query: &SaveQuery, _state: &AppState) -> Result<Project> {
        Ok(Project { id: 1, content: self.content })
    }
}

/// The payload to update a project
#[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpUpdate)]
#[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
struct UpdatableProject {
    id: Id,
    content: String,
}
#[async_trait]
impl UpdatableModel<UpdatableProject, UpdateQuery, AppState> for UpdatableProject {
    async fn update(self: Self, _query: &UpdateQuery, _state: &AppState) -> Result<UpdatableProject> {
        Ok(self)
    }
}

// Registers the documented routes of the project endpoint
pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
    gen_documented_endpoint!(Project, NewProject, UpdatableProject)(cfg)
}
