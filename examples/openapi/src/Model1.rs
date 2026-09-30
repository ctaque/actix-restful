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
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
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
    #[sqlx_model(database = "postgres", timestamps)]
    #[octopux_info(path = "model1")]
    struct Model1 {
        id: Id,
        string: String,
        i32: i32,
        i64: i64,
        f64: f64,
        bool: bool,
        opt_str: Option<String>,
        dt: DateTime<Utc>,
        ndt: DateTime<Utc>,
        ndt2: NaiveDateTime,
        nd: NaiveDate,
        nt: NaiveTime,
        vecu8: Vec<u8>,
        created_at: Option<DateTime<Utc>>,
        updated_at: Option<DateTime<Utc>>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "postgres", model = "Model1", timestamps)]
    struct NewModel1 {
        string: String,
        i32: i32,
        i64: i64,
        f64: f64,
        bool: bool,
        opt_str: Option<String>,
        dt: DateTime<Utc>,
        ndt: DateTime<Utc>,
        ndt2: NaiveDateTime,
        nd: NaiveDate,
        nt: NaiveTime,
        vecu8: Vec<u8>,
    }

    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Model1, FindQuery, AppState)]
    #[sqlx_model(database = "postgres", timestamps)]
    struct UpdatableModel1 {
        id: Id,
        string: String,
        i32: i32,
        i64: i64,
        f64: f64,
        bool: bool,
        opt_str: Option<String>,
        dt: DateTime<Utc>,
        ndt: DateTime<Utc>,
        ndt2: NaiveDateTime,
        nd: NaiveDate,
        nt: NaiveTime,
        vecu8: Vec<u8>,
        updated_at: Option<DateTime<Utc>>,
    }

    // Registers the documented routes of the model1 endpoint
    // (octopux `openapi` feature), to mount with `.configure(model1::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Model1, NewModel1, UpdatableModel1)(cfg)
    }

    
