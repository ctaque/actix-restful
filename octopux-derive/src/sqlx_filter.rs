// `SqlxFilter`: the `SqlxFilter` implementation of a list query, pushing a condition on a column
// for each of its `Option` fields that is set. The column and the operator are read from the field name,
// `price_gte` filters `price >= ...`, or from `#[sqlx_filter(column = "...", op = "...")]`.
// The field marked `#[sqlx_filter(sort = "...")]` gives the `ORDER BY` clause, among the columns it lists,
// and the one marked `#[sqlx_filter(sort_direction)]` the direction of its columns, `asc` or `desc`.
use darling::{FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use quote::quote;

use crate::sqlx_model::{column, is_column_name, Database};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(sqlx_filter), supports(struct_named))]
struct FilterArgs {
    ident: syn::Ident,
    data: darling::ast::Data<(), FilterField>,
    /// `sqlite`, `postgres` or `mysql`
    database: String,
}

#[derive(Debug, FromField)]
#[darling(attributes(sqlx_filter))]
struct FilterField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    /// Defaults to the field name without its operator suffix
    #[darling(default)]
    column: Option<String>,
    /// Defaults to the operator suffix of the field name, `eq` without one
    #[darling(default)]
    op: Option<String>,
    /// The field is not a filter
    #[darling(default)]
    skip: bool,
    /// The field orders the rows, by the columns of this list separated by commas
    #[darling(default)]
    sort: Option<String>,
    /// The field gives the direction of the sort, `asc` or `desc`
    #[darling(default)]
    sort_direction: bool,
}

// Operators of the filters, by their name and the suffix of the field name
const OPERATORS: [(&str, &str); 7] = [
    ("eq", "="),
    ("ne", "<>"),
    ("gt", ">"),
    ("gte", ">="),
    ("lt", "<"),
    ("lte", "<="),
    ("like", "LIKE"),
];

// Pagination fields of the list query, read by `SqlxModel`
const PAGINATION: [&str; 2] = ["offset", "limit"];

fn operator(name: &str) -> Option<&'static str> {
    OPERATORS.iter().find(|(n, _)| *n == name).map(|(_, sql)| *sql)
}

// `price_gte` gives the column `price` and the operator `gte`, `name` the column `name` and `eq`
fn split_suffix(field: &str) -> (&str, &str) {
    OPERATORS
        .iter()
        .filter(|(name, _)| *name != "eq")
        .find_map(|(name, _)| {
            field
                .strip_suffix(name)
                .and_then(|rest| rest.strip_suffix('_'))
                .filter(|column| !column.is_empty())
                .map(|column| (column, *name))
        })
        .unwrap_or((field, "eq"))
}

fn is_option(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path.qself.is_none() && path.path.segments.last().map_or(false, |s| s.ident == "Option"),
        _ => false,
    }
}

pub fn impl_sqlx_filter(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let args = FilterArgs::from_derive_input(ast).map_err(|e| syn::Error::new_spanned(&ast.ident, e.to_string()))?;
    let database = Database::parse(&args.database).ok_or_else(|| {
        syn::Error::new_spanned(&ast.ident, format!("unknown database `{}`, use sqlite, postgres or mysql", args.database))
    })?;
    let db = database.sqlx_type();
    let fields = match args.data {
        darling::ast::Data::Struct(fields) => fields.fields,
        darling::ast::Data::Enum(_) => unreachable!("darling only accepts structs with named fields"),
    };

    let mut filters = Vec::new();
    let mut sort = None;
    let mut direction = None;
    for field in &fields {
        let ident = field.ident.as_ref().expect("darling only accepts structs with named fields");
        let name = column(ident);
        if field.sort.is_some() || field.sort_direction {
            let (slot, attr) = if field.sort_direction { (&mut direction, "sort_direction") } else { (&mut sort, "sort = \"...\"") };
            if slot.is_some() {
                return Err(syn::Error::new_spanned(ident, format!("only one field can be marked #[sqlx_filter({})]", attr)));
            }
            if field.skip || field.column.is_some() || field.op.is_some() || (field.sort.is_some() && field.sort_direction) {
                return Err(syn::Error::new_spanned(ident, "a sort field takes neither `skip`, `column`, `op` nor both `sort` and `sort_direction`"));
            }
            if !is_option(&field.ty) {
                return Err(syn::Error::new_spanned(ident, "a sort field must be an `Option<String>`"));
            }
            *slot = Some(field);
            continue;
        }
        if field.skip || (PAGINATION.contains(&name.as_str()) && field.column.is_none() && field.op.is_none()) {
            continue;
        }
        if !is_option(&field.ty) {
            return Err(syn::Error::new_spanned(
                ident,
                format!("the filter `{}` must be an `Option`, set when the filter applies, or be marked #[sqlx_filter(skip)]", name),
            ));
        }
        let (suffix_column, suffix_op) = split_suffix(&name);
        // an explicit operator keeps the whole field name as the column
        let filter_column = field.column.clone().unwrap_or_else(|| {
            if field.op.is_some() { name.clone() } else { suffix_column.to_string() }
        });
        let op_name = field.op.clone().unwrap_or_else(|| suffix_op.to_string());
        let op = operator(&op_name).ok_or_else(|| {
            let names: Vec<&str> = OPERATORS.iter().map(|(n, _)| *n).collect();
            syn::Error::new_spanned(ident, format!("unknown operator `{}`, use {}", op_name, names.join(", ")))
        })?;
        if !is_column_name(&filter_column) {
            return Err(syn::Error::new_spanned(
                ident,
                format!("`{}` is not a valid column name, use only latin letters (a-z, A-Z), digits and `_`", filter_column),
            ));
        }
        let condition = format!("{} {} ", filter_column, op);
        filters.push(quote! {
            if let ::std::option::Option::Some(value) = &self.#ident {
                qb.push(if *has_where { " AND " } else { " WHERE " });
                *has_where = true;
                qb.push(#condition).push_bind(value);
            }
        });
    }

    let order_by = match (sort, direction) {
        (Some(sort), direction) => Some(sort_body(sort, direction)?),
        (None, Some(direction)) => {
            return Err(syn::Error::new_spanned(&direction.ident, "the sort direction requires a field marked #[sqlx_filter(sort = \"...\")]"));
        }
        (None, None) => None,
    };
    let name = &args.ident;
    let order_by = order_by.map(|body| {
        quote! {
            fn push_order_by(&self, qb: &mut ::octopux::__private::sqlx::QueryBuilder<#db>) -> ::octopux::anyhow::Result<bool> {
                #body
            }
        }
    });
    Ok(quote! {
        impl ::octopux::SqlxFilter<#db> for #name {
            fn push_filters(&self, qb: &mut ::octopux::__private::sqlx::QueryBuilder<#db>, has_where: &mut bool) {
                #(#filters)*
            }
            #order_by
        }
    })
}

// Body of `push_order_by`, the columns of `sort` being checked here and the values of the fields at runtime
fn sort_body(sort: &FilterField, direction: Option<&FilterField>) -> syn::Result<TokenStream> {
    let ident = sort.ident.as_ref().expect("darling only accepts structs with named fields");
    let columns: Vec<&str> = sort.sort.as_deref().unwrap_or_default().split(',').map(str::trim).collect();
    if let Some(invalid) = columns.iter().find(|c| c.is_empty() || !is_column_name(c)) {
        return Err(syn::Error::new_spanned(
            ident,
            format!("`{}` is not a valid sort column, use only latin letters (a-z, A-Z), digits and `_`", invalid),
        ));
    }
    let direction = match direction.and_then(|d| d.ident.as_ref()) {
        Some(direction) => quote! { ::std::option::Option::as_deref(&self.#direction) },
        None => quote! { ::std::option::Option::None },
    };
    Ok(quote! {
        ::octopux::__private::push_order_by(qb, ::std::option::Option::as_deref(&self.#ident), #direction, &[#(#columns),*])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(ast: syn::DeriveInput) -> String {
        impl_sqlx_filter(&ast).map(|t| t.to_string()).unwrap_or_else(|e| format!("error: {}", e))
    }

    #[test]
    fn splits_the_operator_suffix() {
        assert_eq!(split_suffix("price_gte"), ("price", "gte"));
        assert_eq!(split_suffix("price_gt"), ("price", "gt"));
        assert_eq!(split_suffix("name"), ("name", "eq"));
        assert_eq!(split_suffix("name_like"), ("name", "like"));
        // a suffix alone is a column
        assert_eq!(split_suffix("_lt"), ("_lt", "eq"));
        assert_eq!(split_suffix("salt"), ("salt", "eq"));
    }

    #[test]
    fn filters_the_set_fields_by_their_suffix() {
        let out = expand(syn::parse_quote! {
            #[sqlx_filter(database = "postgres")]
            struct ListQuery {
                offset: Option<usize>,
                limit: Option<usize>,
                name: Option<String>,
                price_gte: Option<i32>,
                r#type_ne: Option<i32>,
            }
        });
        assert!(out.contains("impl :: octopux :: SqlxFilter < :: octopux :: __private :: sqlx :: Postgres > for ListQuery"), "{}", out);
        assert!(out.contains("\"name = \""), "{}", out);
        assert!(out.contains("\"price >= \""), "{}", out);
        assert!(out.contains("\"type <> \""), "{}", out);
        assert!(!out.contains("offset"), "{}", out);
        assert!(!out.contains("limit"), "{}", out);
    }

    #[test]
    fn attribute_sets_the_column_and_the_operator() {
        let out = expand(syn::parse_quote! {
            #[sqlx_filter(database = "mysql")]
            struct ListQuery {
                #[sqlx_filter(column = "name", op = "like")]
                q: Option<String>,
                #[sqlx_filter(op = "eq")]
                created_gt: Option<i32>,
                #[sqlx_filter(skip)]
                expand: bool,
            }
        });
        assert!(out.contains(":: sqlx :: MySql"), "{}", out);
        assert!(out.contains("\"name LIKE \""), "{}", out);
        assert!(out.contains("\"created_gt = \""), "{}", out);
        assert!(!out.contains("expand"), "{}", out);
    }

    #[test]
    fn sort_field_lists_the_allowed_columns() {
        let out = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery {
                name: Option<String>,
                #[sqlx_filter(sort = "name, created_at")]
                sort: Option<String>,
            }
        });
        assert!(out.contains("fn push_order_by"), "{}", out);
        assert!(
            out.contains("push_order_by (qb , :: std :: option :: Option :: as_deref (& self . sort) , :: std :: option :: Option :: None , & [\"name\" , \"created_at\"])"),
            "{}",
            out
        );
        // the sort field is not a filter
        assert!(!out.contains("\"sort = \""), "{}", out);
        let without = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { name: Option<String> }
        });
        assert!(!without.contains("push_order_by"), "{}", without);
    }

    #[test]
    fn sort_direction_field_orders_the_columns() {
        let out = expand(syn::parse_quote! {
            #[sqlx_filter(database = "postgres")]
            struct ListQuery {
                #[sqlx_filter(sort_direction)]
                order: Option<String>,
                #[sqlx_filter(sort = "name")]
                sort: Option<String>,
            }
        });
        assert!(
            out.contains("as_deref (& self . sort) , :: std :: option :: Option :: as_deref (& self . order) , & [\"name\"]"),
            "{}",
            out
        );
        // the direction field is not a filter
        assert!(!out.contains("\"order = \""), "{}", out);
    }

    #[test]
    fn rejects_invalid_sort_fields() {
        let bad_column = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { #[sqlx_filter(sort = "name, id DESC")] sort: Option<String> }
        });
        assert!(bad_column.starts_with("error: `id DESC` is not a valid sort column"), "{}", bad_column);
        let empty = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { #[sqlx_filter(sort = "name,")] sort: Option<String> }
        });
        assert!(empty.starts_with("error: `` is not a valid sort column"), "{}", empty);
        let two = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery {
                #[sqlx_filter(sort = "name")] sort: Option<String>,
                #[sqlx_filter(sort = "id")] order: Option<String>,
            }
        });
        assert!(two.starts_with("error: only one field can be marked #[sqlx_filter(sort"), "{}", two);
        let two_directions = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery {
                #[sqlx_filter(sort = "name")] sort: Option<String>,
                #[sqlx_filter(sort_direction)] order: Option<String>,
                #[sqlx_filter(sort_direction)] dir: Option<String>,
            }
        });
        assert!(two_directions.starts_with("error: only one field can be marked #[sqlx_filter(sort_direction)]"), "{}", two_directions);
        let alone = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { #[sqlx_filter(sort_direction)] order: Option<String> }
        });
        assert!(alone.starts_with("error: the sort direction requires a field"), "{}", alone);
        let both = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { #[sqlx_filter(sort = "name", sort_direction)] sort: Option<String> }
        });
        assert!(both.starts_with("error: a sort field takes neither"), "{}", both);
        let not_option = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { #[sqlx_filter(sort = "name")] sort: String }
        });
        assert!(not_option.starts_with("error: a sort field must be an `Option<String>`"), "{}", not_option);
    }

    #[test]
    fn rejects_invalid_filters() {
        let not_option = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { name: String }
        });
        assert!(not_option.starts_with("error: the filter `name` must be an `Option`"), "{}", not_option);
        let unknown_op = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { #[sqlx_filter(op = "between")] name: Option<String> }
        });
        assert!(unknown_op.starts_with("error: unknown operator `between`"), "{}", unknown_op);
        let bad_column = expand(syn::parse_quote! {
            #[sqlx_filter(database = "sqlite")]
            struct ListQuery { #[sqlx_filter(column = "name; DROP TABLE x")] name: Option<String> }
        });
        assert!(bad_column.starts_with("error: `name; DROP TABLE x` is not a valid column name"), "{}", bad_column);
        let unknown_db = expand(syn::parse_quote! {
            #[sqlx_filter(database = "oracle")]
            struct ListQuery { name: Option<String> }
        });
        assert!(unknown_db.contains("unknown database `oracle`"), "{}", unknown_db);
    }
}
