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
    use chrono::{DateTime, Utc};
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
    type ListResult = Vec<Model3>;
    type DeleteResult = Model3;
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
    #[actix_restful_info(path = "model3")]
    struct Model3 {
        id: Id,
        field_opt: String,
        created_at: Option<DateTime<Utc>>,
        updated_at: Option<DateTime<Utc>>,
        deleted_at: Option<DateTime<Utc>>,
    }
    
    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for Model3 {
        async fn find(id: Id, _query: &FindQuery, state: &AppState) -> Result<Box<Model3>> {
            let model = sqlx::query_as::<_, Model3>(
                "SELECT * FROM model3 WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
            Ok(Box::new(model))
        }
        async fn list(query: &ListQuery, state: &AppState) -> Result<ListResult> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, Model3>(
                "SELECT * FROM model3 WHERE deleted_at IS NULL ORDER BY id LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            Ok(models)
        }
        async fn delete(self: Self, _query: &DeleteQuery, state: &AppState) -> Result<DeleteResult> {
            let now = Utc::now();
            let model = sqlx::query_as::<_, Model3>(
                "UPDATE model3 SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL RETURNING *",
            )
            .bind(now)
            .bind(self.id)
            .fetch_one(&state.pool)
            .await?;
            Ok(model)
        }
    }
    
    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, HttpCreate)]
    #[http_create(SaveQuery, AppState)]
    struct NewModel3 {
        field_opt: String,
    }
    #[async_trait]
    impl NewModel<Model3, SaveQuery, AppState> for NewModel3 {
        async fn save(self: Self, _query: &SaveQuery, state: &AppState) -> Result<Model3> {
            let now = Utc::now();
            let model = sqlx::query_as::<_, Model3>(
                "INSERT INTO model3 (field_opt, created_at, updated_at) VALUES ($1, $2, $3) RETURNING *",
            )
            .bind(self.field_opt)
            .bind(now)
            .bind(now)
            .fetch_one(&state.pool)
            .await?;
            Ok(model)
        }
    }
    
    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate)]
    #[http_update(Id, UpdateQuery, Model3, FindQuery, AppState)]
    struct UpdatableModel3 {
        id: Id,
        field_opt: String,
        updated_at: Option<DateTime<Utc>>,
    }
    #[async_trait]
    impl UpdatableModel<UpdatableModel3, UpdateQuery, AppState> for UpdatableModel3 {
        async fn update(self: Self, _query: &UpdateQuery, state: &AppState) -> Result<UpdatableModel3> {
            let model = sqlx::query_as::<_, UpdatableModel3>(
                "UPDATE model3 SET field_opt = $1, updated_at = $2 WHERE id = $3 AND deleted_at IS NULL RETURNING id, field_opt, updated_at",
            )
            .bind(self.field_opt)
            .bind(Utc::now())
            .bind(self.id)
            .fetch_one(&state.pool)
            .await?;
            Ok(model)
        }
    }

    // Registers the documented routes of the model3 endpoint
    // (actix-restful `openapi` feature), to mount with `.configure(model3::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(Model3, NewModel3, UpdatableModel3)(cfg)
    }

    
