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

struct HttpAllDeriveParams (syn::Ident);
impl syn::parse::Parse for HttpAllDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content;
        syn::parenthesized!(content in input);
        let query = content.parse()?;
        Ok(HttpAllDeriveParams(query))
    }
}
#[proc_macro_derive(HttpAll, attributes(http_all))]
pub fn http_all(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_http_all_macro(&ast)
}

fn impl_http_all_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let attribute = ast.attrs.iter().filter(
        |a| a.path.segments.len() == 1 && a.path.segments[0].ident == "http_all"
    ).nth(0).expect("http_all attribute required for deriving HttpAll!");

    let parameter: HttpAllDeriveParams = syn::parse2(attribute.tokens.clone()).expect("Invalid http_all attribute!");
    let HttpAllDeriveParams(query) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[async_trait]
        impl HttpAll<#query> for #name {
            async fn http_all(query: web::Query<#query>) -> Result<HttpResponse, HttpResponse>{
                let params = query.into_inner();
                let result = #name::all(&params).await;
                match result {
                    Ok(res) => Ok(HttpResponse::Ok().body(serde_json::json!(res))),
                    Err(err) => Err(HttpResponse::InternalServerError().body(err.to_string()))
                }
            }
        }
    };
    gen.into()
}

struct HttpFindDeriveParams (syn::Ident, syn::Ident);
impl syn::parse::Parse for HttpFindDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content;
        syn::parenthesized!(content in input);
        let path = content.parse()?;
        content.parse::<Token![,]>()?;
        let query = content.parse()?;
        Ok(HttpFindDeriveParams(path, query))
    }
}

#[proc_macro_derive(HttpFind, attributes(http_find))]
pub fn http_find(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_http_find_macro(&ast)
}

fn impl_http_find_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let attribute = ast.attrs.iter().filter(
        |a| a.path.segments.len() == 1 && a.path.segments[0].ident == "http_find"
    ).nth(0).expect("http_find attribute required for deriving HttpFind!");

    let parameter: HttpFindDeriveParams = syn::parse2(attribute.tokens.clone()).expect("Invalid http_find attribute!");
    let HttpFindDeriveParams(path, query) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[async_trait]
        impl HttpFind<#path, #query> for #name {
            async fn http_find(info: web::Path<#path>, query: web::Query<#query>) -> Result<HttpResponse, HttpResponse> {
                let params = query.into_inner();
                let result = #name::find(info.id.into(), &params).await;
                match result {
                    Ok(res) => Ok(HttpResponse::Ok().body(serde_json::json!(res))),
                    Err(err) => Err(HttpResponse::NotFound().body("ENTITY_NOT_FOUND"))
                }
            }
        }
    };
    gen.into()
}

struct HttpDeleteDeriveParams (syn::Ident, syn::Ident, syn::Ident);
impl syn::parse::Parse for HttpDeleteDeriveParams {
    fn parse(input: syn::parse::ParseStream) -> SynResult<Self> {
        let content;
        syn::parenthesized!(content in input);
        let path = content.parse()?;
        content.parse::<Token![,]>()?;
        let query = content.parse()?;
        content.parse::<Token![,]>()?;
        let find_query = content.parse()?;
        Ok(HttpDeleteDeriveParams(path, query, find_query))
    }
}

#[proc_macro_derive(HttpDelete, attributes(http_delete))]
pub fn http_delete(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_http_delete_macro(&ast)
}

fn impl_http_delete_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let attribute = ast.attrs.iter().filter(
        |a| a.path.segments.len() == 1 && a.path.segments[0].ident == "http_delete"
    ).nth(0).expect("http_delete attribute required for deriving HttpDelete!");

    let parameter: HttpDeleteDeriveParams = syn::parse2(attribute.tokens.clone()).expect("Invalid http_delete attribute!");
    let HttpDeleteDeriveParams(path, query, find_query) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[async_trait]
        impl HttpDelete<#path, #query> for #name {
            async fn http_delete(info: web::Path<#path>, query: web::Query<#query>) -> Result<HttpResponse, HttpResponse> {
                let params = query.into_inner();
                let find_params = #find_query { ..Default::default() };
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
        let path = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let query = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let output = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let find_query = content.parse()?;
        Ok(HttpUpdateDeriveParams(path, query, output, find_query))
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
    let HttpUpdateDeriveParams(path, query, output, find_query) = parameter;

    let name = &ast.ident;
    let gen = quote! {
        #[async_trait]
        impl HttpUpdate<#path, #query> for #name {
            async fn http_update(info: web::Path<#path>, payload: web::Json<Box<#name>>, query: web::Query<#query>) -> Result<HttpResponse, HttpResponse> {
                let to_update = payload.into_inner();
                let params = query.into_inner();
                let find_params = #find_query { ..Default::default() };
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
