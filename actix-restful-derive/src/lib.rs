extern crate proc_macro;
use darling::FromMeta;
use quote::{quote, ToTokens};
use syn::{ self, Result as SynResult, AttributeArgs, Token, parse_macro_input };

struct HttpCreateDeriveParams (syn::Ident, syn::Ident);
impl syn::parse::Parse for HttpCreateDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content;
        syn::parenthesized!(content in input);
        let query = content.parse()?;
        content.parse::<Token![,]>()?;
        let app_state = content.parse()?;
        Ok(HttpCreateDeriveParams(query, app_state))
    }
}
#[proc_macro_derive(HttpCreate, attributes(http_create))]
pub fn http_create(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_http_create_macro(&ast)
}

fn impl_http_create_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let attribute = ast.attrs.iter().filter(
        |a| a.path.segments.len() == 1 && a.path.segments[0].ident == "http_create"
    ).nth(0).expect("http_create attribute required for deriving HttpCreate!");

    let parameter: HttpCreateDeriveParams = syn::parse2(attribute.tokens.clone()).expect("Invalid http_create attribute!");
    let HttpCreateDeriveParams(query, app_state) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[async_trait]
        impl HttpCreate<#query, #app_state> for #name {
            async fn http_create(payload: actix_web::web::Json<Box<#name>>, query: actix_web::web::Query<#query>, state: actix_web::web::Data<#app_state>) -> Result<actix_web::HttpResponse, actix_web::HttpResponse>{
                let params = query.into_inner();
                let to_save = payload.into_inner();
                let result = to_save.save(&params, &state).await;
                match result {
                    Ok(res) => Ok(actix_web::HttpResponse::Ok().body(serde_json::json!(res))),
                    Err(err) => Err(actix_web::HttpResponse::InternalServerError().body(err.to_string()))
                }
            }
        }
    };
    gen.into()
}

struct HttpFindListDeleteDeriveParams (syn::Ident, syn::Ident, syn::Ident, syn::Ident, syn::Ident);
impl syn::parse::Parse for HttpFindListDeleteDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content;
        syn::parenthesized!(content in input);
        let id = content.parse()?;
        content.parse::<Token![,]>()?;
        let find_query = content.parse()?;
        content.parse::<Token![,]>()?;
        let list_query = content.parse()?;
        content.parse::<Token![,]>()?;
        let delete_query = content.parse()?;
        content.parse::<Token![,]>()?;
        let app_state = content.parse()?;
        Ok(HttpFindListDeleteDeriveParams(id, find_query, list_query, delete_query, app_state))
    }
}


#[derive(Debug, FromMeta)]
struct RestfulInfo {
    pub scope: String,
    pub path: String,
}

impl ToTokens for RestfulInfo {
    fn to_tokens(&self, _tokens: &mut proc_macro2::TokenStream) {
        ()
    }
}

#[proc_macro_attribute]
pub fn actix_restful_info(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let attrs_args = parse_macro_input!(args as AttributeArgs);
    let ast: syn::DeriveInput = syn::parse(input.clone()).unwrap();

    let args_tokens = match RestfulInfo::from_list(&attrs_args) {
        Ok(v) => v,
        Err(e) => { return proc_macro::TokenStream::from(e.write_errors()); }
    };
    let name  = ast.ident;
    let path =args_tokens.path;
    let scope = args_tokens.scope;
    let gen = quote! {
        impl RestfulPathInfo for #name {
            fn path() -> String  {
                let p = #path;
                let p = p.to_string();
                p
            }
            fn scope() -> &'static str {
                let p = #scope;
                p
            }
        }
    };
    let mut out:proc_macro::TokenStream = gen.into();
    out.extend::<proc_macro::TokenStream>(input);
    out
}

#[proc_macro_derive(HttpFindListDelete, attributes(http_find_list_delete))]
pub fn http_find_list_delete(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_http_find_list_delete_macro(&ast)
}

fn impl_http_find_list_delete_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let attribute = ast.attrs.iter().filter(
        |a| a.path.segments.len() == 1 && a.path.segments[0].ident == "http_find_list_delete"
    ).nth(0).expect("http_find_list_delete attribute required for deriving HttpFindListDelete!");

    let parameter: HttpFindListDeleteDeriveParams = syn::parse2(attribute.tokens.clone()).expect("Invalid http_find_list_delete attribute!");
    let HttpFindListDeleteDeriveParams(id, find_query, list_query, delete_query, app_state) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[derive(Deserialize)]
        struct ActixRestfulPath {
            id: #id
        }
        #[async_trait]
        impl HttpFindListDelete<ActixRestfulPath, #find_query, #list_query, #delete_query, #app_state> for #name {
            async fn http_list(
                query: actix_web::web::Query<#list_query>,
                state: actix_web::web::Data<#app_state>
            ) -> Result<actix_web::HttpResponse, actix_web::HttpResponse>{
                let params = query.into_inner();
                let result = #name::list(&params, &state).await;
                match result {
                    Ok(res) => Ok(actix_web::HttpResponse::Ok().body(serde_json::json!(res))),
                    Err(err) => Err(actix_web::HttpResponse::InternalServerError().body(err.to_string()))
                }
            }
            async fn http_find(
                info: actix_web::web::Path<ActixRestfulPath>,
                query: actix_web::web::Query<#find_query>,
                state: actix_web::web::Data<#app_state>
            ) -> Result<actix_web::HttpResponse, actix_web::HttpResponse> {
                let params = query.into_inner();
                let result = #name::find(info.id.into(), &params, &state).await;
                match result {
                    Ok(res) => Ok(actix_web::HttpResponse::Ok().body(serde_json::json!(res))),
                    Err(err) => Err(actix_web::HttpResponse::NotFound().body("ENTITY_NOT_FOUND"))
                }
            }
            async fn http_delete(
                info: actix_web::web::Path<ActixRestfulPath>,
                query: actix_web::web::Query<#delete_query>,
                state: actix_web::web::Data<#app_state>
            ) -> Result<actix_web::HttpResponse, actix_web::HttpResponse> {
                let params = query.into_inner();
                let find_params: #find_query = Default::default();
                let result = #name::find(info.id.into(), &find_params, &state).await;

                match result {
                    Ok(entity) => {
                        match entity.delete(&params, &state).await {
                            Ok(e) => Ok(actix_web::HttpResponse::Ok().body(serde_json::json!(e))),
                            Err(err) => Err(actix_web::HttpResponse::InternalServerError().body(err.to_string()))
                        }
                    }
                    Err(err) => Err(actix_web::HttpResponse::NotFound().body("ENTITY_NOT_FOUND"))
                }
            }
        }
    };
    gen.into()
}

struct HttpUpdateDeriveParams (syn::Ident, syn::Ident, syn::Ident, syn::Ident, syn::Ident);
impl syn::parse::Parse for HttpUpdateDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content;
        syn::parenthesized!(content in input);
        let id = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let query = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let output = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let find_query = content.parse()?;
        content.parse::<Token![,]>()?;
        let app_state = content.parse()?;
        Ok(HttpUpdateDeriveParams(id, query, output, find_query, app_state))
    }
}

#[proc_macro_derive(HttpUpdate, attributes(http_update))]
pub fn http_update(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_http_update_macro(&ast)
}

fn impl_http_update_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let attribute = ast.attrs.iter().filter(
        |a| a.path.segments.len() == 1 && a.path.segments[0].ident == "http_update"
    ).nth(0).expect("http_update attribute required for deriving HttpUpdate!");

    let parameter: HttpUpdateDeriveParams = syn::parse2(attribute.tokens.clone()).expect("Invalid http_update attribute!");
    let HttpUpdateDeriveParams(id, query, output, find_query, app_state) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[derive(Deserialize)]
        struct ActixRestfulUpdatePath {
            id: #id
        }
        #[async_trait]
        impl HttpUpdate<ActixRestfulUpdatePath, #query, #app_state> for #name {
            async fn http_update(
                info: actix_web::web::Path<ActixRestfulUpdatePath>,
                payload: actix_web::web::Json<Box<#name>>,
                query: actix_web::web::Query<#query>,
                state: actix_web::web::Data<#app_state>
            ) -> Result<actix_web::HttpResponse, actix_web::HttpResponse> {
                let to_update = payload.into_inner();
                let params = query.into_inner();
                let find_params: #find_query = Default::default();
                let result = #output::find(info.id.into(), &find_params, &state).await;

                match result {
                    Ok(entity) => {
                        match to_update.update(&params, &state).await {
                            Ok(e) => Ok(actix_web::HttpResponse::Ok().body(serde_json::json!(e))),
                            Err(err) => Err(actix_web::HttpResponse::InternalServerError().body(err.to_string()))
                        }
                    }
                    Err(err) => Err(actix_web::HttpResponse::NotFound().body("ENTITY_NOT_FOUND"))
                }
            }
        }
    };
    gen.into()
}
