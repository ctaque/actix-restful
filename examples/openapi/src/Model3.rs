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
    };
    use chrono::{DateTime, Utc};
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_endpoint;

    #[derive(Default, Deserialize, JsonSchema, ApiComponent)]
    struct FindQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct ListQuery {
        /// Number of rows to skip
        offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        limit: Option<usize>,
    }
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct DeleteQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct SaveQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct UpdateQuery {}
    type Id = i64;

    #[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "postgres", timestamps, soft_delete)]
    #[octopux_info(path = "model3")]
    struct Model3 {
        id: Id,
        field_opt: String,
        created_at: Option<DateTime<Utc>>,
        updated_at: Option<DateTime<Utc>>,
        deleted_at: Option<DateTime<Utc>>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Model3", timestamps)]
    struct NewModel3 {
        field_opt: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Model3, FindQuery, AppState)]
    #[sqlx_model(database = "postgres", timestamps, soft_delete)]
    struct UpdatableModel3 {
        id: Id,
        field_opt: String,
        updated_at: Option<DateTime<Utc>>,
    }

    // Registers the documented routes of the model3 endpoint
    // (octopux `openapi` feature), to mount with `.configure(model3::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Model3, NewModel3, UpdatableModel3)(cfg)
    }

    
