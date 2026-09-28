    use crate::shared::AppState;
    use serde::{Serialize, Deserialize};
    use actix_restful::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        Model,
        NewModel,
        UpdatableModel,
        actix_restful_info,
        anyhow::Result,
        async_trait,
    };
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use actix_restful::gen_documented_endpoint;

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
    type ListResult = Vec<Model1>;
    type DeleteResult = Model1;
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct SaveQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    struct UpdateQuery {}
    type Id = i64;
    /// Number of rows returned by list when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of rows returned by list
    const MAX_LIMIT: i64 = 100;

    #[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpFindListDelete)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[actix_restful_info(path = "model1")]
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
    
    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Model1 {
        async fn find(id: Id, _query: &FindQuery, state: &AppState) -> Result<Box<Model1>> {
            let model = sqlx::query_as::<_, Model1>(
                "SELECT * FROM model1 WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
            Ok(Box::new(model))
        }
        async fn list(query: &ListQuery, state: &AppState) -> Result<ListResult> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, Model1>(
                "SELECT * FROM model1 ORDER BY id LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            Ok(models)
        }
        async fn delete(self: Self, _query: &DeleteQuery, state: &AppState) -> Result<DeleteResult> {
            let model = sqlx::query_as::<_, Model1>(
                "DELETE FROM model1 WHERE id = $1 RETURNING *",
            )
            .bind(self.id)
            .fetch_one(&state.pool)
            .await?;
            Ok(model)
        }
    }
    
    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate)]
    #[http_create(SaveQuery, AppState)]
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
    #[async_trait]
    impl NewModel<Model1, SaveQuery, AppState> for NewModel1 {
        async fn save(self: Self, _query: &SaveQuery, state: &AppState) -> Result<Model1> {
            let now = Utc::now();
            let model = sqlx::query_as::<_, Model1>(
                "INSERT INTO model1 (string, i32, i64, f64, bool, opt_str, dt, ndt, ndt2, nd, nt, vecu8, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) RETURNING *",
            )
            .bind(self.string)
            .bind(self.i32)
            .bind(self.i64)
            .bind(self.f64)
            .bind(self.bool)
            .bind(self.opt_str)
            .bind(self.dt)
            .bind(self.ndt)
            .bind(self.ndt2)
            .bind(self.nd)
            .bind(self.nt)
            .bind(self.vecu8)
            .bind(now)
            .bind(now)
            .fetch_one(&state.pool)
            .await?;
            Ok(model)
        }
    }
    
    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate)]
    #[http_update(Id, UpdateQuery, Model1, FindQuery, AppState)]
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
    #[async_trait]
    impl UpdatableModel<UpdatableModel1, UpdateQuery, AppState> for UpdatableModel1 {
        async fn update(self: Self, _query: &UpdateQuery, state: &AppState) -> Result<UpdatableModel1> {
            let model = sqlx::query_as::<_, UpdatableModel1>(
                "UPDATE model1 SET string = $1, i32 = $2, i64 = $3, f64 = $4, bool = $5, opt_str = $6, dt = $7, ndt = $8, ndt2 = $9, nd = $10, nt = $11, vecu8 = $12, updated_at = $13 WHERE id = $14 RETURNING id, string, i32, i64, f64, bool, opt_str, dt, ndt, ndt2, nd, nt, vecu8, updated_at",
            )
            .bind(self.string)
            .bind(self.i32)
            .bind(self.i64)
            .bind(self.f64)
            .bind(self.bool)
            .bind(self.opt_str)
            .bind(self.dt)
            .bind(self.ndt)
            .bind(self.ndt2)
            .bind(self.nd)
            .bind(self.nt)
            .bind(self.vecu8)
            .bind(Utc::now())
            .bind(self.id)
            .fetch_one(&state.pool)
            .await?;
            Ok(model)
        }
    }

    // Registers the documented routes of the model1 endpoint
    // (actix-restful `openapi` feature), to mount with `.configure(model1::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Model1, NewModel1, UpdatableModel1)(cfg)
    }

    
