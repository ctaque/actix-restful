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
    #[octopux_info(path = "book")]
    #[graphql(complex)]
    pub struct Book {
        pub id: Id,
        pub title: String,
        pub author_id: i64,
    }

    #[derive(Serialize, Deserialize, InputObject, HttpCreate, SqlxNewModel)]
    #[http_create(SaveQuery, AppState)]
    #[sqlx_model(database = "sqlite", model = "Book")]
    pub struct NewBook {
        pub title: String,
        pub author_id: i64,
    }

    #[derive(Serialize, Deserialize, InputObject, sqlx::FromRow, HttpUpdate, SqlxUpdatableModel)]
    #[http_update(Id, UpdateQuery, Book, FindQuery, AppState)]
    #[sqlx_model(database = "sqlite")]
    pub struct UpdatableBook {
        pub id: Id,
        pub title: String,
        pub author_id: i64,
    }

    // Registers the routes of the book endpoint, to mount with `.configure(book::configure)`
    pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
        gen_endpoint!(Book, NewBook, UpdatableBook)(cfg)
    }


    // Fields of the GraphQL Book type resolved by functions, its has-many relations
    #[ComplexObject]
    impl Book {
        // Relations generated with `octopux generate-relation --graphql`, inserted below
    }

    // GraphQL queries of the book model (async-graphql), to merge into the query root of the schema:
    // `#[derive(MergedObject, Default)] struct Query(book::BookQuery, ...);`
    #[derive(Default)]
    pub struct BookQuery;

    #[Object]
    impl BookQuery {
        async fn book(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Book> {
            find(id, app_state(ctx)?).await
        }

        async fn books(&self, ctx: &Context<'_>, offset: Option<usize>, limit: Option<usize>) -> async_graphql::Result<Vec<Book>> {
            Ok(Book::list(&ListQuery { offset, limit }, app_state(ctx)?).await?)
        }
    }

    // GraphQL mutations of the book model, to merge into the mutation root of the schema:
    // `#[derive(MergedObject, Default)] struct Mutation(book::BookMutation, ...);`
    #[derive(Default)]
    pub struct BookMutation;

    #[Object]
    impl BookMutation {
        async fn create_book(&self, ctx: &Context<'_>, input: NewBook) -> async_graphql::Result<Book> {
            Ok(input.save(&SaveQuery {}, app_state(ctx)?).await?)
        }

        async fn update_book(&self, ctx: &Context<'_>, input: UpdatableBook) -> async_graphql::Result<Book> {
            let state = app_state(ctx)?;
            let id = input.id;
            find(id, state).await?;
            input.update(&UpdateQuery {}, state).await?;
            find(id, state).await
        }

        async fn delete_book(&self, ctx: &Context<'_>, id: Id) -> async_graphql::Result<Book> {
            let state = app_state(ctx)?;
            let model = find(id, state).await?;
            Ok(model.delete(&DeleteQuery {}, state).await?)
        }
    }

    // Looks the book up, any error being ENTITY_NOT_FOUND as for the REST routes
    async fn find(id: Id, state: &AppState) -> async_graphql::Result<Book> {
        match Book::find(id, &FindQuery::default(), state).await {
            Ok(model) => Ok(*model),
            Err(_) => Err(async_graphql::Error::new("ENTITY_NOT_FOUND")),
        }
    }

    // The state shared with the REST routes, given to the schema with `Schema::build(...).data(state.clone())`
    fn app_state<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a AppState> {
        Ok(ctx.data::<actix_web::web::Data<AppState>>()?.get_ref())
    }

    