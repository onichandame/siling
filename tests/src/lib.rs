use event_adaptor::expand_event_adaptor_tests;
use proc_macro::TokenStream;

mod event_adaptor;

#[proc_macro]
pub fn test_event_adaptor(factory: TokenStream) -> TokenStream {
    let input = factory.into();
    expand_event_adaptor_tests(input).into()
}
