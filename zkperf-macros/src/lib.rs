//! # zkperf-macros
//!
//! Proc macros that define security context boundaries for the zkPerf witness system.
//!
//! Each `#[witness_boundary]` annotation:
//! 1. Tags the function with a compile-time security signature (SHA-256 of path + constraints)
//! 2. Declares maximal complexity + perf counter bounds that constrain execution
//! 3. Wraps the body with timing, perf sampling, and witness recording
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Time-only constraint
//! #[witness_boundary(complexity = "O(n)", max_n = 1000, max_ms = 30000)]
//! async fn search(&self, q: &str) -> Result<Vec<Hit>> { ... }
//!
//! // Full perf constraint — enforce CPU cycles, instructions, cache behavior
//! #[witness_boundary(
//!     complexity = "O(n log n)",
//!     max_n = 10000,
//!     max_ms = 5000,
//!     max_cycles = 50_000_000,
//!     max_instructions = 20_000_000,
//!     max_cache_misses = 100_000,
//! )]
//! fn sort_results(data: &mut [Record]) { ... }
//! ```

use proc_macro::TokenStream;
use quote::quote;
use sha2::{Digest, Sha256};
use syn::{parse_macro_input, ItemFn, LitStr, LitInt};

/// Zero-config instrumentation. Just add `#[zkperf]` to any function.
/// Records timing, generates witness, posts to zkperf-service if running.
///
/// ```rust,ignore
/// #[zkperf]
/// fn my_function() -> Result<()> { ... }
///
/// #[zkperf]
/// async fn my_async_fn() -> String { ... }
/// ```
#[proc_macro_attribute]
pub fn zkperf(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let vis = &input.vis;
    let sig = &input.sig;
    let body = &input.block;
    let fn_attrs = &input.attrs;

    let mut hasher = Sha256::new();
    hasher.update(fn_name_str.as_bytes());
    let sig_hash = hex::encode(&hasher.finalize()[..16]);

    let record = quote! {
        let __ms = __t0.elapsed().as_millis() as u64;
        ::zkperf_witness::record(::zkperf_witness::Witness {
            context: #fn_name_str,
            signature: #sig_hash,
            complexity: "auto",
            max_n: 0,
            max_ms: 0,
            elapsed_ms: __ms,
            violated: false,
            timestamp: ::zkperf_witness::now_ms(),
            platform: ::std::env::consts::OS,
            perf: None,
            violations: None,
        });
    };

    let wrapped = if sig.asyncness.is_some() {
        quote! {
            let __t0 = ::std::time::Instant::now();
            let __r = (async move { #body }).await;
            #record
            __r
        }
    } else {
        quote! {
            let __t0 = ::std::time::Instant::now();
            let __r = (|| #body)();
            #record
            __r
        }
    };

    (quote! {
        #(#fn_attrs)*
        #vis #sig { #wrapped }
    }).into()
}

#[proc_macro_attribute]
pub fn witness_boundary(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let attrs = parse_macro_input!(attr as BoundaryAttrs);

    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let vis = &input.vis;
    let sig = &input.sig;
    let body = &input.block;
    let fn_attrs = &input.attrs;

    let complexity = &attrs.complexity;
    let max_n = attrs.max_n;
    let max_ms = attrs.max_ms;
    let context = attrs.context.unwrap_or_else(|| fn_name_str.clone());

    // Compile-time security signature
    let mut hasher = Sha256::new();
    hasher.update(context.as_bytes());
    hasher.update(b"|");
    hasher.update(complexity.as_bytes());
    hasher.update(b"|");
    hasher.update(max_n.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(max_ms.to_string().as_bytes());
    if let Some(c) = attrs.max_cycles { hasher.update(format!("|cyc:{c}").as_bytes()); }
    if let Some(i) = attrs.max_instructions { hasher.update(format!("|ins:{i}").as_bytes()); }
    if let Some(m) = attrs.max_cache_misses { hasher.update(format!("|cm:{m}").as_bytes()); }
    if let Some(b) = attrs.max_branch_misses { hasher.update(format!("|bm:{b}").as_bytes()); }
    let sig_hash = hex::encode(hasher.finalize());

    let has_perf = attrs.max_cycles.is_some()
        || attrs.max_instructions.is_some()
        || attrs.max_cache_misses.is_some()
        || attrs.max_branch_misses.is_some();

    let perf_constraints = if has_perf {
        let cyc = opt_some_u64(attrs.max_cycles);
        let ins = opt_some_u64(attrs.max_instructions);
        let cm = opt_some_u64(attrs.max_cache_misses);
        let bm = opt_some_u64(attrs.max_branch_misses);
        quote! {
            let __zkperf_constraints = ::zkperf_witness::PerfConstraints {
                max_cycles: #cyc,
                max_instructions: #ins,
                max_cache_misses: #cm,
                max_branch_misses: #bm,
                ..::std::default::Default::default()
            };
            let __zkperf_perf0 = ::zkperf_witness::PerfReadings::sample();
        }
    } else {
        quote! {}
    };

    let record_call = if has_perf {
        quote! {
            let __zkperf_perf1 = ::zkperf_witness::PerfReadings::sample();
            let __zkperf_delta = __zkperf_perf1.delta(&__zkperf_perf0);
            ::zkperf_witness::record_with_perf(
                #context,
                #sig_hash,
                #complexity,
                #max_n,
                #max_ms,
                __zkperf_elapsed.as_millis() as u64,
                &__zkperf_constraints,
                &__zkperf_delta,
            );
        }
    } else {
        quote! {
            ::zkperf_witness::record(::zkperf_witness::Witness {
                context: #context,
                signature: #sig_hash,
                complexity: #complexity,
                max_n: #max_n,
                max_ms: #max_ms,
                elapsed_ms: __zkperf_elapsed.as_millis() as u64,
                violated: __zkperf_elapsed.as_millis() as u64 > #max_ms,
                timestamp: ::zkperf_witness::now_ms(),
                platform: ::std::env::consts::OS,
                perf: None,
                violations: None,
            });
        }
    };

    let is_async = sig.asyncness.is_some();

    let witness_block = if is_async {
        quote! {
            #perf_constraints
            let __zkperf_t0 = ::std::time::Instant::now();
            let __zkperf_result = (async move { #body }).await;
            let __zkperf_elapsed = __zkperf_t0.elapsed();
            #record_call
            __zkperf_result
        }
    } else {
        quote! {
            #perf_constraints
            let __zkperf_t0 = ::std::time::Instant::now();
            let __zkperf_result = (|| #body)();
            let __zkperf_elapsed = __zkperf_t0.elapsed();
            #record_call
            __zkperf_result
        }
    };

    let output = quote! {
        #(#fn_attrs)*
        #vis #sig {
            #witness_block
        }
    };

    output.into()
}

fn opt_some_u64(v: Option<u64>) -> proc_macro2::TokenStream {
    match v {
        Some(n) => quote! { Some(#n) },
        None => quote! { None },
    }
}

// --- attribute parsing ---

struct BoundaryAttrs {
    complexity: String,
    max_n: u64,
    max_ms: u64,
    context: Option<String>,
    max_cycles: Option<u64>,
    max_instructions: Option<u64>,
    max_cache_misses: Option<u64>,
    max_branch_misses: Option<u64>,
}

impl syn::parse::Parse for BoundaryAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = BoundaryAttrs {
            complexity: "O(1)".into(),
            max_n: 0,
            max_ms: 60_000,
            context: None,
            max_cycles: None,
            max_instructions: None,
            max_cache_misses: None,
            max_branch_misses: None,
        };

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            let _: syn::Token![=] = input.parse()?;

            match key.to_string().as_str() {
                "complexity" => { let v: LitStr = input.parse()?; attrs.complexity = v.value(); }
                "max_n" => { let v: LitInt = input.parse()?; attrs.max_n = v.base10_parse()?; }
                "max_ms" => { let v: LitInt = input.parse()?; attrs.max_ms = v.base10_parse()?; }
                "context" => { let v: LitStr = input.parse()?; attrs.context = Some(v.value()); }
                "max_cycles" => { let v: LitInt = input.parse()?; attrs.max_cycles = Some(v.base10_parse()?); }
                "max_instructions" => { let v: LitInt = input.parse()?; attrs.max_instructions = Some(v.base10_parse()?); }
                "max_cache_misses" => { let v: LitInt = input.parse()?; attrs.max_cache_misses = Some(v.base10_parse()?); }
                "max_branch_misses" => { let v: LitInt = input.parse()?; attrs.max_branch_misses = Some(v.base10_parse()?); }
                other => {
                    return Err(syn::Error::new(key.span(),
                        format!("unknown attribute `{other}`")));
                }
            }

            if !input.is_empty() {
                let _: syn::Token![,] = input.parse()?;
            }
        }

        Ok(attrs)
    }
}
