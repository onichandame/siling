use event_adaptor::expand_event_adaptor_tests;
use proc_macro::TokenStream;
use storage_adaptor::expand_storage_tests;

mod event_adaptor;
mod storage_adaptor;

#[proc_macro]
pub fn test_event_adaptor(factory: TokenStream) -> TokenStream {
    expand_event_adaptor_tests(factory.into()).into()
}

#[proc_macro]
pub fn test_storage_adaptor(factory: TokenStream) -> TokenStream {
    expand_storage_tests(factory.into()).into()
}
