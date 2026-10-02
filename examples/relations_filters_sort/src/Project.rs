
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
        SqlxFilter
    };
    use chrono::{DateTime, Utc};
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_endpoint;

    #[derive(Default, Deserialize, JsonSchema, ApiComponent)]
    pub struct FindQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent, SqlxFilter)]
    #[sqlx_filter(database = "postgres")]
    pub struct ListQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
        #[sqlx_filter(sort = "created_at,updated_at, user_id")]
        pub sort: Option<String>,
        #[sqlx_filter(sort_direction)]
        pub direction: Option<String>,
        pub user_id: Option<i64>,
        /// Only the projects created at or after this date (RFC 3339)
        pub created_at_gte: Option<DateTime<Utc>>,
        pub created_at_lte: Option<DateTime<Utc>>,
        /// Only the projects created at exactly this date (RFC 3339)
        #[sqlx_filter(column = "created_at", op = "eq")]
        pub created_at_eq: Option<DateTime<Utc>>,
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
    #[sqlx_model(database = "postgres", timestamps, soft_delete, filter)]
    #[octopux_info(path = "project")]
    pub struct Project {
        pub id: Id,
        pub user_id: i64,
        pub optional_string_vec: Option<Vec<String>>,
        pub created_at: Option<DateTime<Utc>>,
        pub updated_at: Option<DateTime<Utc>>,
        pub deleted_at: Option<DateTime<Utc>>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Project", timestamps)]
    pub struct NewProject {
        pub optional_string_vec: Option<Vec<String>>,
        pub user_id: i64,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Project, FindQuery, AppState)]
    #[sqlx_model(database = "postgres", timestamps, soft_delete)]
    pub struct UpdatableProject {
        pub id: Id,
        pub user_id: i64,
        pub optional_string_vec: Option<Vec<String>>,
        pub updated_at: Option<DateTime<Utc>>,
    }

    // Registers the documented routes of the project endpoint
    // (octopux `openapi` feature), to mount with `.configure(project::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Project, NewProject, UpdatableProject)(cfg)
    }

    
