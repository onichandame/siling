use proc_macro2::TokenStream;
use quote::quote;

pub fn expand_event_adaptor_tests(_factory: TokenStream) -> TokenStream {
    quote! {
        #[cfg(test)]
        mod tests {
            #[tokio::test]
            async fn event_adaptor(){}
        }
    }
}
