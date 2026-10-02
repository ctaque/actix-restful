
    // The application state, declared (or re-exported) at the root of the crate
    use crate::shared::AppState;
    use crate::user::{User, Id};
    use crate::project::Project;
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
    pub struct UserProjectsQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;

    /// The projects of a user, served on `GET /user/{id}/projects`
    pub struct UserProjects;

    #[async_trait]
    impl HasMany for UserProjects {
        type Parent = User;
        type Id = Id;
        type Query = UserProjectsQuery;
        type Result = Vec<Project>;
        type State = AppState;
        const RELATION: &'static str = "projects";

        async fn list_related(id: Id, query: &UserProjectsQuery, state: &AppState) -> Result<Option<Vec<Project>>> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, Project>(
                "SELECT * FROM project WHERE user_id = $1 AND deleted_at IS NULL ORDER BY id LIMIT $2 OFFSET $3",
            )
            .bind(id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            // the parent is only looked up when the page is empty
            if models.is_empty() {
                let parent = sqlx::query_scalar::<_, Id>(
                    "SELECT id FROM user WHERE id = $1 AND deleted_at IS NULL",
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

    // Registers the route of the projects of a user, to mount with `.configure(user_projects::configure)`
    // in the same scope as the user routes
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_relation_endpoint!(UserProjects)(cfg)
    }
    
