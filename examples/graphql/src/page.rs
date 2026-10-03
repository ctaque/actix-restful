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
    use chrono::{DateTime, Utc};
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
    #[sqlx_model(database = "sqlite", timestamps, soft_delete)]
    #[octopux_info(path = "page")]
    #[graphql(complex)]
    pub struct Page {
        pub id: Id,
        pub page_number: i32,
        pub book_id: i64,
        pub contents: String,
        pub created_at: Option<DateTime<Utc>>,
        pub updated_at: Option<DateTime<Utc>>,
        pub deleted_at: Option<DateTime<Utc>>,
    }

    #[derive(Serialize, Deserialize, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "sqlite", model = "Page", timestamps)]
    pub struct NewPage {
        pub page_number: i32,
        pub book_id: i64,
        pub contents: String,
    }

    #[derive(Serialize, Deserialize, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Page, FindQuery, AppState)]
    #[sqlx_model(database = "sqlite", timestamps, soft_delete)]
    pub struct UpdatablePage {
        pub id: Id,
        pub page_number: i32,
        pub book_id: i64,
        pub contents: String,
        pub updated_at: Option<DateTime<Utc>>,
    }

    // Registers the routes of the page endpoint, to mount with `.configure(page::configure)`
    pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
        gen_endpoint!(Page, NewPage, UpdatablePage)(cfg)
    }


    // Fields of the GraphQL Page type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Page {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
    }

    // GraphQL queries of the page model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(page::PageQuery, ...);`
    #[derive(Default)]
    pub struct PageQuery;

    #[Object]
    impl PageQuery {
        async fn page(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Page> {
            find(id, app_state(ctx)?).await
        }

        async fn pages(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Page>> {
            Ok(Page::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the page model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(page::PageMutation, ...);`
    #[derive(Default)]
    pub struct PageMutation;

    #[Object]
    impl PageMutation {
        async fn create_page(&self, ctx: &Context<'_>, input: NewPage) -> async_graphql::Result<Page> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_page(&self, ctx: &Context<'_>, input: UpdatablePage) -> async_graphql::Result<Page> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_page(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Page> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the page up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Page> {
        match Page::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    