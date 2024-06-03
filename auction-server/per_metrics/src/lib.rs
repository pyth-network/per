extern crate proc_macro;
use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{
        parse_macro_input,
        punctuated::Punctuated,
        ItemFn,
        Meta,
    },
};

#[proc_macro_attribute]
pub fn record_metrics(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let __per_metrics_category = args.iter().find_map(|arg| {
        if let Meta::NameValue(nv) = arg {
            if nv.path.is_ident("category") {
                if let syn::Expr::Lit(ref s) = nv.value {
                    if let syn::Lit::Str(ref s) = s.lit {
                        return Some(s.value());
                    }
                }
            }
        }
        None
    });

    match __per_metrics_category {
        Some(__per_metrics_category) => {
            let ItemFn {
                attrs,
                vis,
                sig,
                block,
            } = parse_macro_input!(item as ItemFn);

            let fn_name = sig.ident.to_string();
            let start = quote!(__measure_time_start_instant);

            let expanded = quote! {
                #(#attrs)*
                #vis #sig {
                    let #start = ::std::time::Instant::now();
                    let ret = #block;
                    let latency = ::std::time::Instant::now().duration_since(#start);
                    let labels = [
                        ("function", #fn_name),
                    ];

                    metrics::counter!(format!("{}_requests_total", #__per_metrics_category), &labels).increment(1);
                    metrics::histogram!(format!("{}_duration_seconds", #__per_metrics_category), &labels).record(latency);

                    ret
                }
            };

            TokenStream::from(expanded)
        }
        None => TokenStream::from(quote! {
            compile_error!("Missing category argument");
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
    }
}
