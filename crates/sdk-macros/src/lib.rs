//! Proc-macros for the COG Rust dialect.
//!
//! `#[view]` exports the function as `__cog_view_<name>` so that the
//! host can discover all views in a module by iterating exports and
//! filtering on the prefix, and describes it in `__cog_meta_<name>`
//! (see `common::abi`).
//!
//! ```ignore
//! #[view]
//! fn whole(g: &mut Graph) { ... }
//!
//! #[view(for_each = Pattern::nodes_where("kind", "use"))]
//! fn per_use(g: &mut Graph, use_site: NodeId) { ... }
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{Expr, ItemFn, MetaNameValue, Token, parse_macro_input};

#[proc_macro_attribute]
pub fn view(args: TokenStream, input: TokenStream) -> TokenStream {
    let args =
        parse_macro_input!(args with Punctuated::<MetaNameValue, Token![,]>::parse_terminated);
    let func = parse_macro_input!(input as ItemFn);

    let mut for_each: Option<Expr> = None;
    for arg in args {
        if arg.path.is_ident("for_each") {
            for_each = Some(arg.value);
        } else {
            return syn::Error::new_spanned(arg.path, "expected `for_each = <Pattern>`")
                .to_compile_error()
                .into();
        }
    }

    let user_fn_ident = &func.sig.ident;
    let export_ident = format_ident!("__cog_view_{}", user_fn_ident);
    let meta_ident = format_ident!("__cog_meta_{}", user_fn_ident);

    let (key, call, pattern) = match &for_each {
        Some(pattern) => (
            format_ident!("key"),
            quote! {
                #user_fn_ident(&mut graph, key.expect("a `for_each` view runs with a key"))
            },
            quote! { ::core::option::Option::Some(#pattern) },
        ),
        None => (
            format_ident!("_key"),
            quote! { #user_fn_ident(&mut graph) },
            quote! { ::core::option::Option::None },
        ),
    };

    let expanded = quote! {
        #func

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "C" fn #export_ident(args_ptr: u32, args_len: u32) {
            let #key: ::core::option::Option<::sdk::EntityId> = ::sdk::__args(args_ptr, args_len);
            let mut graph = ::sdk::Graph::__new();
            #call;
        }

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "C" fn #meta_ident() -> u64 {
            ::sdk::__ret(&::sdk::ViewMeta { for_each: #pattern })
        }
    };

    expanded.into()
}
