//                         -----
//                     -------------
//                   -----  ----------
//                  ---  --------------
//                 ---  ----------------
//                 --- -----------------
//                 --- -----------------
//                 --- -----------------
//                 ---------------------
//       -----      -------------------       -----
//      -------      --  ---------- --      -------
//          ----      ---------------      -----
//           ---      ---------------      ----
//          ----     -----------------     ----
//        ------   ----------------------   ------
//    --------  ---------------------------  --------
//   ------   -------------------------- ----   -------
//  ----    -----  ---------- --- ------- -----    -----
// ----  ------  -------- --- --- ---- ---  ------  ----
// ----        ---- ----  --- ---- ---- -----       ----
//  ----   ------  ----  ---- ----  ----   ------  -----
//  ------      ------   ---- -----  ------      ------
//    ---------------    ----  ----    --------------
//      ----------       ----  ----      ----------
//                       ----  ----
//                 ---   ---- -----   --
//               ------  ---- ----- -------
//              -------  ---- ----- --------
//              ----    ----   -----    ----
//              -----------     -----------
//               ---------        --------

    // The application state, declared (or re-exported) at the root of the crate
    use crate::AppState;
    use serde::{Serialize, Deserialize};
    use octopux::{
        HttpCreate,
        HttpFindListDelete,
        HttpUpdate,
        SqlxModel,
        SqlxNewModel,
        SqlxUpdatableModel,
        octopux_info,
        Model,
        NewModel,
        UpdatableModel,
    };
    use octopux::gen_endpoint;
    use async_graphql::{ComplexObject, Context, InputObject, Object, SimpleObject};

    #[derive(Default, Deserialize)]
    pub struct FindQuery {}
    #[derive(Deserialize)]
    pub struct ListQuery {
        /// Number of rows to skip
        pub offset: Option<usize>,
        /// Maximum number of rows to return (20 by default, 100 at most)
        pub limit: Option<usize>,
    }
    #[derive(Deserialize)]
    pub struct DeleteQuery {}
    #[derive(Deserialize)]
    pub struct SaveQuery {}
    #[derive(Deserialize)]
    pub struct UpdateQuery {}
    pub type Id = i64;

    #[derive(Default, Serialize, Deserialize, SimpleObject, sqlx::FromRow, HttpFindListDelete, SqlxModel)]
    #[http_find_list_delete(Id, FindQuery, ListQuery, DeleteQuery, AppState)]
    #[sqlx_model(database = "sqlite")]
    #[octopux_info(path = "author")]
    #[graphql(complex)]
    pub struct Author {
        pub id: Id,
        pub name: String,
    }

    #[derive(Serialize, Deserialize, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "sqlite", model = "Author")]
    pub struct NewAuthor {
        pub name: String,
    }

    #[derive(Serialize, Deserialize, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Author, FindQuery, AppState)]
    #[sqlx_model(database = "sqlite")]
    pub struct UpdatableAuthor {
        pub id: Id,
        pub name: String,
    }

    // Registers the routes of the author endpoint, to mount with `.configure(author::configure)`
    pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
        gen_endpoint!(Author, NewAuthor, UpdatableAuthor)(cfg)
    }


    // Fields of the GraphQL Author type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Author {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
        /// The books of the author, paginated
        async fn books(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<crate::book::Book>> {
            crate::author_books::resolve(ctx, self.id, offset, limit).await
        }
    }

    // GraphQL queries of the author model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(author::AuthorQuery, ...);`
    #[derive(Default)]
    pub struct AuthorQuery;

    #[Object]
    impl AuthorQuery {
        async fn author(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Author> {
            find(id, app_state(ctx)?).await
        }

        async fn authors(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Author>> {
            Ok(Author::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the author model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(author::AuthorMutation, ...);`
    #[derive(Default)]
    pub struct AuthorMutation;

    #[Object]
    impl AuthorMutation {
        async fn create_author(&self, ctx: &Context<'_>, input: NewAuthor) -> async_graphql::Result<Author> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_author(&self, ctx: &Context<'_>, input: UpdatableAuthor) -> async_graphql::Result<Author> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_author(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Author> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the author up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Author> {
        match Author::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    