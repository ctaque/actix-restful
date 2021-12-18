extern crate proc_macro;
use quote::quote;
use syn::{ self, Result as SynResult, Token};

struct HttpCreateDeriveParams (syn::Ident);
impl syn::parse::Parse for HttpCreateDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content;
        syn::parenthesized!(content in input);
        let query = content.parse()?;
        Ok(HttpCreateDeriveParams(query))
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
    let HttpCreateDeriveParams(query) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[async_trait]
        impl HttpCreate<#query> for #name {
            async fn http_create(payload: web::Json<Box<#name>>, query: web::Query<#query>) -> Result<HttpResponse, HttpResponse>{
                let params = query.into_inner();
                let to_save = payload.into_inner();
                let result = to_save.save(&params).await;
                match result {
                    Ok(res) => Ok(HttpResponse::Ok().body(serde_json::json!(res))),
                    Err(err) => Err(HttpResponse::InternalServerError().body(err.to_string()))
                }
            }
        }
    };
    gen.into()
}

struct HttpFindListDeleteDeriveParams (syn::Ident, syn::Ident, syn::Ident, syn::Ident);
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
        Ok(HttpFindListDeleteDeriveParams(id, find_query, list_query, delete_query))
    }
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
    let HttpFindListDeleteDeriveParams(id, find_query, list_query, delete_query) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[derive(Deserialize)]
        struct ActixRestfulPath {
            id: #id
        }
        #[async_trait]
        impl HttpFindListDelete<ActixRestfulPath, #find_query, #list_query, #delete_query> for #name {
            async fn http_list(query: web::Query<#list_query>) -> Result<HttpResponse, HttpResponse>{
                let params = query.into_inner();
                let result = #name::list(&params).await;
                match result {
                    Ok(res) => Ok(HttpResponse::Ok().body(serde_json::json!(res))),
                    Err(err) => Err(HttpResponse::InternalServerError().body(err.to_string()))
                }
            }
            async fn http_find(info: web::Path<ActixRestfulPath>, query: web::Query<#find_query>) -> Result<HttpResponse, HttpResponse> {
                let params = query.into_inner();
                let result = #name::find(info.id.into(), &params).await;
                match result {
                    Ok(res) => Ok(HttpResponse::Ok().body(serde_json::json!(res))),
                    Err(err) => Err(HttpResponse::NotFound().body("ENTITY_NOT_FOUND"))
                }
            }
            async fn http_delete(info: web::Path<ActixRestfulPath>, query: web::Query<#delete_query>) -> Result<HttpResponse, HttpResponse> {
                let params = query.into_inner();
                let find_params: #find_query = Default::default();
                let result = #name::find(info.id.into(), &find_params).await;

                match result {
                    Ok(entity) => {
                        match entity.delete(&params).await {
                            Ok(e) => Ok(HttpResponse::Ok().body(serde_json::json!(e))),
                            Err(err) => Err(HttpResponse::InternalServerError().body(err.to_string()))
                        }
                    }
                    Err(err) => Err(HttpResponse::NotFound().body("ENTITY_NOT_FOUND"))
                }
            }
        }
    };
    gen.into()
}

struct HttpUpdateDeriveParams (syn::Ident, syn::Ident, syn::Ident, syn::Ident);
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
        Ok(HttpUpdateDeriveParams(id, query, output, find_query))
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
    let HttpUpdateDeriveParams(id, query, output, find_query) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[derive(Deserialize)]
        struct ActixRestfulUpdatePath {
            id: #id
        }
        #[async_trait]
        impl HttpUpdate<ActixRestfulUpdatePath, #query> for #name {
            async fn http_update(info: web::Path<ActixRestfulUpdatePath>, payload: web::Json<Box<#name>>, query: web::Query<#query>) -> Result<HttpResponse, HttpResponse> {
                let to_update = payload.into_inner();
                let params = query.into_inner();
                let find_params: #find_query = Default::default();
                let result = #output::find(info.id.into(), &find_params).await;

                match result {
                    Ok(entity) => {
                        match to_update.update(&params).await {
                            Ok(e) => Ok(HttpResponse::Ok().body(serde_json::json!(e))),
                            Err(err) => Err(HttpResponse::InternalServerError().body(err.to_string()))
                        }
                    }
                    Err(err) => Err(HttpResponse::NotFound().body("ENTITY_NOT_FOUND"))
                }
            }
        }
    };
    gen.into()
}
