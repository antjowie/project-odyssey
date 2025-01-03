extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn;

/// Implement Display using the Debug trait
#[proc_macro_derive(DisplayDebug)]
pub fn display_debug_derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let ast = syn::parse(input).unwrap();

    // Build the impl
    impl_display_debug(&ast)
}

fn impl_display_debug(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self, f)
            }
        }
    };
    gen.into()
}
