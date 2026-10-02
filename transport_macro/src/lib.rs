use proc_macro::TokenStream as CompilerTokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input,
    spanned::Spanned,
    Data,
    DeriveInput,
    Error,
    Fields,
};

#[proc_macro_derive(TripleBuffered)]
/// Derives TripleBuffered for a struct with named fields.
///
/// Usage:
/// #[derive(TripleBuffered)]
/// struct TestData {
/// x: f32
/// }
///
/// let (send, recv) = TestData::channel(TestData{x: 0.0});
///
pub fn derive_triple_buffered(input: CompilerTokenStream) -> CompilerTokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_triple_buffered(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_triple_buffered(
    input: &DeriveInput,
) -> syn::Result<TokenStream2> {
    let state_name = &input.ident;
    let sender_name = format_ident!("{state_name}Sender");
    let receiver_name = format_ident!("{state_name}Receiver");

    match &input.data {
        Data::Struct(data) => {
            if !matches!(&data.fields, Fields::Named(_)) {
                return Err(Error::new(
                    data.fields.span(),
                    "TripleBuffered requires a struct with named fields",
                ));
            }
        }
        _ => {
            return Err(Error::new(
                input.span(),
                "TripleBuffered can only be derived for structs",
            ));
        }
    }

    Ok(quote! {
        pub struct #sender_name {
            inner: ::triple_buffer::Input<#state_name>,
        }

        pub struct #receiver_name {
            inner: ::triple_buffer::Output<#state_name>,
        }

        impl #state_name {
            pub fn channel(
                initial: #state_name,
            ) -> (#sender_name, #receiver_name) {
                let (input, output) =
                    ::triple_buffer::triple_buffer(&initial);

                (
                    #sender_name { inner: input },
                    #receiver_name { inner: output },
                )
            }
        }

        impl #sender_name {
            pub fn send(&mut self, value: #state_name) {
                self.inner.write(value);
            }
        }

        impl #receiver_name {
            pub fn receive(&mut self) -> &#state_name {
                self.inner.read()
            }
        }
    })
}

