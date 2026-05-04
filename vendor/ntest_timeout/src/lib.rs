//! Part of the ntest library. Add timeout attribute to the rust test framework.

extern crate proc_macro;
extern crate syn;

use proc_macro::TokenStream;
use quote::quote;

use syn::parse_macro_input;

/// The timeout attribute can be used for tests to let them fail if they exceed a certain execution time.
/// With the `#[timeout]` attribute a timeout in milliseconds is added to a test.
///
/// The function input must be of type `int`. For example `#[timeout(10)]` will fail if the test takes longer than 10 milliseconds.
///
/// # Examples
///
/// This example will not panic
///
/// ```
/// #[test]
/// #[timeout(100)]
/// fn no_timeout() {
///     let fifty_millis = time::Duration::from_millis(50);
///     thread::sleep(fifty_millis);
/// }
/// ```
///
/// This example will panic and break the infinite loop after 10 milliseconds.
///
/// ```
/// #[test]
/// #[timeout(10)]
/// #[should_panic]
/// fn timeout() {
///     loop {};
/// }
/// ```
///
/// Also works with test functions using a Result:
///
/// ```
/// #[test]
/// #[timeout(100)]
/// fn timeout_with_result() -> Result<(), String> {
///     let ten_millis = time::Duration::from_millis(10);
///     thread::sleep(ten_millis);
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn timeout(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let time_ms = get_timeout(&parse_macro_input!(attr as syn::AttributeArgs));
    let vis = &input.vis;
    let sig = &input.sig;
    let output = &sig.output;
    let body = &input.block;
    let attrs = &input.attrs;
    check_other_attributes(&input);
    let result = quote! {
        #(#attrs)*
        #vis #sig {
            fn ntest_callback() #output
            #body
            let ntest_timeout_now = std::time::Instant::now();
            
            type NtestPanicPayload = std::boxed::Box<dyn std::any::Any + std::marker::Send + 'static>;
            // Channel sends Result: Ok for success, Err for panic payload
            let (sender, receiver) = std::sync::mpsc::channel::<std::result::Result<_, NtestPanicPayload>>();
            std::thread::spawn(move || {
                let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ntest_callback()
                }));
                // Send will fail if receiver has already timed out or dropped - this is expected
                let _ = sender.send(panic_result);
            });
            match receiver.recv_timeout(std::time::Duration::from_millis(#time_ms)) {
                std::result::Result::Ok(std::result::Result::Ok(t)) => return t,
                std::result::Result::Ok(std::result::Result::Err(panic_payload)) => {
                    // Resume the panic with the original payload to preserve panic message
                    std::panic::resume_unwind(panic_payload);
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!("timeout: the function call took {} ms. Max time {} ms", ntest_timeout_now.elapsed().as_millis(), #time_ms),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => panic!("Thread disconnected unexpectedly"),
            }
        }
    };
    result.into()
}

fn check_other_attributes(input: &syn::ItemFn) {
    for attribute in &input.attrs {
        let meta = attribute.parse_meta();
        match meta {
            std::result::Result::Ok(m) => match m {
                syn::Meta::Path(p) => {
                    if p.segments.iter().any(|ps| ps.ident == "timeout") {
                        panic!("Timeout attribute is only allowed once");
                    }
                }
                syn::Meta::List(ml) => {
                    if ml.path.segments.iter().any(|ps| ps.ident == "timeout") {
                        panic!("Timeout attribute is only allowed once");
                    }
                }
                syn::Meta::NameValue(nv) => {
                    if nv.path.segments.iter().any(|ps| ps.ident == "timeout") {
                        panic!("Timeout attribute is only allowed once");
                    }
                }
            },
            Err(e) => panic!("Could not determine meta data. Error {}.", e),
        }
    }
}

fn get_timeout(attribute_args: &syn::AttributeArgs) -> u64 {
    if attribute_args.len() > 1 {
        panic!("Only one integer expected. Example: #[timeout(10)]");
    }
    match &attribute_args[0] {
        syn::NestedMeta::Meta(_) => {
            panic!("Integer expected. Example: #[timeout(10)]");
        }
        syn::NestedMeta::Lit(lit) => match lit {
            syn::Lit::Int(int) => int.base10_parse::<u64>().expect("Integer expected"),
            _ => {
                panic!("Integer as timeout in ms expected. Example: #[timeout(10)]");
            }
        },
    }
}
