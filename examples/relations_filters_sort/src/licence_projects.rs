
    // The application state, declared (or re-exported) at the root of the crate
    use crate::shared::AppState;
    use crate::licence::{Licence, Id};
    use crate::project::Project;
    use serde::Deserialize;
    use octopux::{
        HasMany,
        anyhow::Result,
        async_trait,
        gen_documented_relation_endpoint
    };
    use apistos::ApiComponent;
    use schemars::JsonSchema;

    #[derive(Deserialize, JsonSchema, ApiComponent)]
    pub struct LicenceProjectsQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;

    /// The projects of a licence, served on `GET /licence/{id}/projects`
    pub struct LicenceProjects;

    #[async_trait]
    impl HasMany for LicenceProjects {
        type Parent = Licence;
        type Id = Id;
        type Query = LicenceProjectsQuery;
        type Result = Vec<Project>;
        type State = AppState;
        const RELATION: &'static str = "projects";

        async fn list_related(id: Id, query: &LicenceProjectsQuery, state: &AppState) -> Result<Option<Vec<Project>>> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, Project>(
                "SELECT project.* FROM project JOIN projectlicence ON projectlicence.project_id = project.id WHERE projectlicence.licence_id = $1 AND project.deleted_at IS NULL AND projectlicence.deleted_at IS NULL ORDER BY project.id LIMIT $2 OFFSET $3",
            )
            .bind(id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            // the parent is only looked up when the page is empty
            if models.is_empty() {
                let parent = sqlx::query_scalar::<_, Id>(
                    "SELECT id FROM licence WHERE id = $1 AND deleted_at IS NULL",
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

    // Registers the route of the projects of a licence, to mount with `.configure(licence_projects::configure)`
    // in the same scope as the licence routes
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_relation_endpoint!(LicenceProjects)(cfg)
    }
    
