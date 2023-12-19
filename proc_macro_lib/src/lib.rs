extern crate proc_macro;
extern crate quote;
extern crate syn;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Leakable)]
pub fn derive_auto_release(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let indent = input.ident;
    quote!(
        impl Drop for #indent {
            fn drop(&mut self) {
                use crate::ffi::ffi_buffer::Releasable;
                unsafe {
                    self.release()
                }
            }
        }
    )
    .into()
}
