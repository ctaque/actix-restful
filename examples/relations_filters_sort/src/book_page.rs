
    // The application state, declared (or re-exported) at the root of the crate
    use crate::shared::AppState;
    use serde::{Serialize, Deserialize};
    use octopux::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        Model,
        NewModel,
        UpdatableModel,
        octopux_info,
        anyhow::Result,
        async_trait,
    };
    use chrono::{DateTime, Utc};
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_endpoint;

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
    pub type ListResult = Vec<BookPage>;
    pub type DeleteResult = BookPage;
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct SaveQuery {}
    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct UpdateQuery {}
    pub type Id = i64;
    /// Number of rows returned by list when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of rows returned by list
    const MAX_LIMIT: i64 = 100;

    #[derive(Default, Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpFindListDelete)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[octopux_info(path = "bookpage")]
    pub struct BookPage {
        pub id: Id,
        pub book_id: i64,
        pub page_content: String,
        pub created_at: Option<DateTime<Utc>>,
        pub updated_at: Option<DateTime<Utc>>,
        pub deleted_at: Option<DateTime<Utc>>,
    }
    
    #[async_trait]
    impl Model<Id, FindQuery, ListQuery, ListResult, DeleteQuery, DeleteResult, AppState> for BookPage {
        async fn find(id: Id, _query: &FindQuery, state: &AppState) -> Result<Box<BookPage>> {
            let model = sqlx::query_as::<_, BookPage>(
                "SELECT * FROM bookpage WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
            Ok(Box::new(model))
        }
        async fn list(query: &ListQuery, state: &AppState) -> Result<ListResult> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, BookPage>(
                "SELECT * FROM bookpage WHERE deleted_at IS NULL ORDER BY id LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            Ok(models)
        }
        async fn delete(self: Self, _query: &DeleteQuery, state: &AppState) -> Result<DeleteResult> {
            let now = Utc::now();
            let model = sqlx::query_as::<_, BookPage>(
                "UPDATE bookpage SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL RETURNING *",
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
    pub struct NewBookPage {
        pub book_id: i64,
        pub page_content: String,
    }
    #[async_trait]
    impl NewModel<BookPage, SaveQuery, AppState> for NewBookPage {
        async fn save(self: Self, _query: &SaveQuery, state: &AppState) -> Result<BookPage> {
            let now = Utc::now();
            let model = sqlx::query_as::<_, BookPage>(
                "INSERT INTO bookpage (book_id, page_content, created_at, updated_at) VALUES ($1, $2, $3, $4) RETURNING *",
            )
            .bind(self.book_id)
            .bind(self.page_content)
            .bind(now)
            .bind(now)
            .fetch_one(&state.pool)
            .await?;
            Ok(model)
        }
    }
    
    #[derive(Serialize, Deserialize, JsonSchema, ApiComponent, sqlx::FromRow, HttpUpdate)]
    #[http_update(Id, UpdateQuery, BookPage, FindQuery, AppState)]
    pub struct UpdatableBookPage {
        pub id: Id,
        pub book_id: i64,
        pub page_content: String,
        pub updated_at: Option<DateTime<Utc>>,
    }
    #[async_trait]
    impl UpdatableModel<UpdatableBookPage, UpdateQuery, AppState> for UpdatableBookPage {
        async fn update(self: Self, _query: &UpdateQuery, state: &AppState) -> Result<UpdatableBookPage> {
            let model = sqlx::query_as::<_, UpdatableBookPage>(
                "UPDATE bookpage SET book_id = $1, page_content = $2, updated_at = $3 WHERE id = $4 AND deleted_at IS NULL RETURNING id, book_id, page_content, updated_at",
            )
            .bind(self.book_id)
            .bind(self.page_content)
            .bind(Utc::now())
            .bind(self.id)
            .fetch_one(&state.pool)
            .await?;
            Ok(model)
        }
    }

    // Registers the documented routes of the bookpage endpoint
    // (octopux `openapi` feature), to mount with `.configure(bookpage::configure)`
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_endpoint!(BookPage, NewBookPage, UpdatableBookPage)(cfg)
    }

    
