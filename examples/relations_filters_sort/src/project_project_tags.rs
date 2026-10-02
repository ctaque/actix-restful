
    // The application state, declared (or re-exported) at the root of the crate
    use crate::shared::AppState;
    use crate::project::{Project, Id};
    use crate::project_tags::ProjectTags;
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
    pub struct ProjectProjectTagsQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    /// Number of children returned when no limit is given
    const DEFAULT_LIMIT: i64 = 20;
    /// Maximum number of children returned
    const MAX_LIMIT: i64 = 100;

    /// The project_tags of a project, served on `GET /project/{id}/project_tags`
    pub struct ProjectProjectTags;

    #[async_trait]
    impl HasMany for ProjectProjectTags {
        type Parent = Project;
        type Id = Id;
        type Query = ProjectProjectTagsQuery;
        type Result = Vec<ProjectTags>;
        type State = AppState;
        const RELATION: &'static str = "project_tags";

        async fn list_related(id: Id, query: &ProjectProjectTagsQuery, state: &AppState) -> Result<Option<Vec<ProjectTags>>> {
            let offset = query.offset.unwrap_or(0) as i64;
            let limit = query.limit.map_or(DEFAULT_LIMIT, |l| (l as i64).min(MAX_LIMIT));
            let models = sqlx::query_as::<_, ProjectTags>(
                "SELECT projecttag.* FROM projecttag JOIN projecttags ON projecttags.project_tag_id = projecttag.id WHERE projecttags.project_id = $1 AND projecttag.deleted_at IS NULL AND projecttags.deleted_at IS NULL ORDER BY projecttag.id LIMIT $2 OFFSET $3",
            )
            .bind(id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
            // the parent is only looked up when the page is empty
            if models.is_empty() {
                let parent = sqlx::query_scalar::<_, Id>(
                    "SELECT id FROM project WHERE id = $1 AND deleted_at IS NULL",
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

    // Registers the route of the project_tags of a project, to mount with `.configure(project_project_tags::configure)`
    // in the same scope as the project routes
    pub fn configure(cfg: &mut apistos::web::ServiceConfig) {
        gen_documented_relation_endpoint!(ProjectProjectTags)(cfg)
    }
    
