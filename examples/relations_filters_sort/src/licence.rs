
    // The application state, declared (or re-exported) at the root of the crate
    use crate::shared::AppState;
    use serde::{Serialize, Deserialize};
    use octopux::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        SqlxModel,
        SqlxNewModel,
        SqlxUpdatableModel,
        octopux_info,
        gen_documented_endpoint
    };
    use chrono::{DateTime, Utc};
    use schemars::JsonSchema;
    use apistos::ApiComponent;


    #[derive(Default, Deserialize, JsonSchema, ApiComponent)]
    pub struct FindQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct ListQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct DeleteQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct SaveQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct UpdateQuery {}
    pub type Id = i64;

    #[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "postgres", timestamps, soft_delete)]
    #[octopux_info(path = "licence")]
    pub struct Licence {
        pub id: Id,
        pub licence_name: String,
        pub created_at: Option<DateTime<Utc>>,
        pub updated_at: Option<DateTime<Utc>>,
        pub deleted_at: Option<DateTime<Utc>>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Licence", timestamps)]
    pub struct NewLicence {
        pub licence_name: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Licence, FindQuery, AppState)]
    #[sqlx_model(database = "postgres", timestamps, soft_delete)]
    pub struct UpdatableLicence {
        pub id: Id,
        pub licence_name: String,
        pub updated_at: Option<DateTime<Utc>>,
    }

    // Registers the documented routes of the licence endpoint
    // (octopux `openapi` feature), to mount with `.configure(licence::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Licence, NewLicence, UpdatableLicence)(cfg)
    }

    
