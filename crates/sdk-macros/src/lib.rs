//! Proc-macros for the COG Rust dialect.
//!
//! `#[view]` exports the function as `__cog_view_<name>` so that the
//! host can discover all views in a module by iterating exports and
//! filtering on the prefix.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn view(_args: TokenStream, input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    let user_fn_ident = &func.sig.ident;
    let export_ident = format_ident!("__cog_view_{}", user_fn_ident);

    let expanded = quote! {
        #func

        #[unsafe(no_mangle)]
        pub extern "C" fn #export_ident() {
            let mut graph = ::sdk::Graph::__new();
            #user_fn_ident(&mut graph);
        }
    };

    expanded.into()
}