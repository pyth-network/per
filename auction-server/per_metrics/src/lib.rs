extern crate proc_macro;
use {
    proc_macro::TokenStream,
    proc_macro2::Span,
    quote::quote,
    syn::{
        parse::{
            Error,
            Parse,
            ParseStream,
            Result,
        },
        parse_macro_input,
        punctuated::Punctuated,
        Expr,
        ItemFn,
        Lit,
        Meta,
        ReturnType,
        Token,
        Type,
    },
};

// let args = parse_macro_input!(args as Args);

struct Args {
    category: String,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self> {
        match try_parse(input) {
            Ok(args) => Ok(args),
            _ => Err(error()),
        }
    }
}

fn try_parse(input: ParseStream) -> Result<Args> {
    let args = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
    let args: Option<Args> = args.iter().find_map(|arg| {
        if let Meta::NameValue(nv) = arg {
            if nv.path.is_ident("category") {
                if let Expr::Lit(ref s) = nv.value {
                    if let Lit::Str(ref s) = s.lit {
                        return Some(Args {
                            category: s.value(),
                        });
                    }
                }
            }
        }
        None
    });
    match args {
        Some(args) => Ok(args),
        _ => Err(error()),
    }
}

fn error() -> Error {
    let msg = "expected #[record_metrics(category = \"...\")]";
    Error::new(Span::call_site(), msg)
}

#[proc_macro_attribute]
pub fn record_metrics(args: TokenStream, item: TokenStream) -> TokenStream {
    let Args {
        category: __record_metrics_category,
    } = parse_macro_input!(args as Args);

    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = parse_macro_input!(item as ItemFn);

    let fn_name = sig.ident.to_string();
    let start = quote!(__measure_time_start_instant);

    // Check if the return type is Result
    let __record_metrics_is_result = match &sig.output {
        ReturnType::Type(_, ty) => match &**ty {
            Type::Path(type_path) => type_path
                .path
                .segments
                .last()
                .map_or(false, |segment| segment.ident == "Result"),
            _ => false,
        },
        ReturnType::Default => false,
    };

    // let __record_metrics_category = category;
    let expanded = quote! {
        #(#attrs)*
        #vis #sig {
            let #start = ::std::time::Instant::now();
            let ret = #block;

            let latency = #start.elapsed().as_secs_f64();

            let mut result = "success";
            if #__record_metrics_is_result {
                result = match &ret {
                    Ok(_) => "success",
                    Err(_) => "error",
                };
            }

            let labels = [
                ("function", #fn_name),
                ("result", result),
            ];

            metrics::counter!(format!("{}_total", #__record_metrics_category), &labels).increment(1);
            metrics::histogram!(format!("{}_duration_seconds", #__record_metrics_category), &labels).record(latency);

            ret
        }
    };

    TokenStream::from(expanded)
}
