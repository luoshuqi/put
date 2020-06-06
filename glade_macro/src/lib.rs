use proc_macro::TokenStream;
use quote::quote;
use syn;

#[proc_macro_attribute]
pub fn ui(attr: TokenStream, item: TokenStream) -> TokenStream {
    let st: syn::ItemStruct = syn::parse(item).expect("expect struct");
    let st_name = &st.ident;
    let mut fields = Vec::with_capacity(st.fields.len());
    for x in &st.fields {
        if let Some(f) = &x.ident {
            fields.push(f);
        }
    }
    let file: syn::LitStr = syn::parse(attr).expect("expect string literal");

    let gen = quote! {
        #st

        impl #st_name {
            pub fn new() -> Self {
                let str = include_str!(#file);
                let builder = gtk::Builder::new_from_string(str);
                #st_name {
                    #(
                        #fields: builder.get_object(stringify!(#fields)).expect(concat!("get object ", stringify!(#fields), " failed"))
                    ),*
                }
            }
        }
    };

    gen.into()
}
