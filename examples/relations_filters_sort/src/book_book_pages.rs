
    // The application state, declared (or re-exported) at the root of the crate
    use crate::shared::AppState;
    use crate::book::{Book, Id};
    use crate::book_page::BookPage;
    use serde::Deserialize;
    use octopux::{
        HasMany,
        anyhow::Result,
        async_trait,
    };
    use apistos::ApiComponent;
    use schemars::JsonSchema;
    use octopux::gen_documented_relation_endpoint;

    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct BookBookPagesQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;

    /// The book_pages of a book, served on `GET /book/{id}/book_pages`
    pub struct BookBookPages;

    #[async_trait]
    impl HasMany for BookBookPages {
        type Parent = Book;
        type Id = Id;
        type Query = BookBookPagesQuery;
        type Result = Vec<BookPage>;
        type State = AppState;
        const RELATION: &'static str = "book_pages";

        async fn list_related(id: Id, query: &BookBookPagesQuery, state: &AppState) -> Result<Option<Vec<BookPage>>> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, BookPage>(
                "SELECT * FROM bookpage WHERE book_id = $1 AND deleted_at IS NULL ORDER BY id LIMIT $2 OFFSET $3",
            )
            .bind(id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            // the parent is only looked up when the page is empty
            if models.is_empty() {
                let parent = sqlx::query_scalar::<_, Id>(
                    "SELECT id FROM book WHERE id = $1 AND deleted_at IS NULL",
                )
                .bind(id)
                .fetch_optional(&state.pool)
                .await?;
                if parent.is_none() {
                    return Ok(None);
                }
            }
            Ok(Some(models))
        }
    }

    // Registers the route of the book_pages of a book, to mount with `.configure(book_book_pages::configure)`
    // in the same scope as the book routes
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_relation_endpoint!(BookBookPages)(cfg)
    }
    
