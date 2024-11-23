use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, Attribute, FnArg, Ident, ImplItem, ImplItemMethod, ItemImpl, Meta,
    NestedMeta, PatType, ReturnType, Type, TypePath,
};

#[proc_macro_attribute]
pub fn subsystem(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input `impl` block
    let input = parse_macro_input!(item as ItemImpl);

    // Collect the generated items
    let mut generated_items = Vec::new();
    let mut impl_items = Vec::new();

    // Determine the path to titan_core
    let titan_core_path = match crate_name("titan_core") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            quote!(#ident)
        }
        Err(_) => quote!(titan_core),
    };

    let titan_path = match crate_name("titan") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            quote!(#ident)
        }
        Err(_) => quote!(titan_core),
    };

    for item in input.items.iter() {
        if let ImplItem::Method(method) = item {
            // Check if the method has the `#[task(...)]` attribute
            let task_attr = method
                .attrs
                .iter()
                .find(|attr| attr.path.is_ident("task"));

            if let Some(attr) = task_attr {
                // Process the method to generate the Task struct and impl
                let gen = generate_task(
                    &input.self_ty,
                    method.clone(),
                    attr.clone(),
                    &titan_core_path,
                    &titan_path,
                );
                generated_items.push(gen);

                // Remove the `#[task(...)]` attribute from the method
                let mut method_clone = method.clone();
                method_clone
                    .attrs
                    .retain(|a| !a.path.is_ident("task"));

                impl_items.push(ImplItem::Method(method_clone));
            } else {
                // Keep the method as is
                impl_items.push(item.clone());
            }
        } else {
            // Keep other impl items as is
            impl_items.push(item.clone());
        }
    }

    // Reconstruct the original impl block without the `#[task]` attributes
    let original_impl = ItemImpl {
        attrs: input.attrs.clone(),
        defaultness: input.defaultness,
        unsafety: input.unsafety,
        impl_token: input.impl_token,
        generics: input.generics.clone(),
        trait_: None,
        self_ty: input.self_ty.clone(),
        brace_token: input.brace_token,
        items: impl_items,
    };

    let self_ty = &input.self_ty;
    let subsystem_trait_path = quote! { #titan_core_path::Subsystem };

    // Generate the final token stream
    let expanded = quote! {
        #original_impl

        impl #subsystem_trait_path for #self_ty {}

        #(#generated_items)*
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn task(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Return the input tokens unmodified
    item
}

fn generate_task(
    self_ty: &Type,
    method: ImplItemMethod,
    attr: Attribute,
    titan_core_path: &proc_macro2::TokenStream,
    titan_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let fn_name = &method.sig.ident;
    let fn_name_str = fn_name.to_string();
    let fn_args = &method.sig.inputs;
    let fn_asyncness = &method.sig.asyncness;
    let fn_output = &method.sig.output;

    // Generate the Task struct name
    let struct_name_str = format!("{}{}", &fn_name_str[..1].to_uppercase(), &fn_name_str[1..]);
    let struct_name = Ident::new(&struct_name_str, fn_name.span());

    let original_subsystem_name = self_ty
        .clone()
        .to_token_stream()
        .to_string();
    let subsystem_name = original_subsystem_name.replace("Subsystem", "");
    let channel_name = subsystem_name.to_lowercase();
    let channel_ident = Ident::new(&channel_name, proc_macro2::Span::call_site());

    let script_task_name_str = format!("{}{}", subsystem_name, struct_name_str);
    let script_task_name = Ident::new(&script_task_name_str, fn_name.span());
    let script_task_name_fn = Ident::new(&script_task_name_str.to_lowercase(), fn_name.span());

    let mut subsystem_ty = self_ty.clone();
    let mut is_benchmark = false;

    let args = match attr.parse_meta() {
        Ok(Meta::List(meta_list)) => meta_list
            .nested
            .into_iter()
            .collect::<Vec<NestedMeta>>(),
        Ok(_) => Vec::new(),
        Err(err) => {
            eprintln!("Error parsing attribute arguments: {}", err);
            Vec::new()
        }
    };

    for arg in args {
        match arg {
            NestedMeta::Meta(Meta::Path(path)) => {
                if path.is_ident("benchmark") {
                    is_benchmark = true;
                }
            }
            NestedMeta::Meta(Meta::NameValue(nv)) => {
                if nv.path.is_ident("subsystem") {
                    if let syn::Lit::Str(lit_str) = nv.lit {
                        subsystem_ty = syn::parse_str::<Type>(&lit_str.value()).unwrap();
                    }
                }
            }
            _ => {}
        }
    }

    let mut struct_fields = Vec::new();
    let mut call_args = Vec::new();
    let mut param_names = Vec::new();

    for arg in fn_args.iter() {
        if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
            // Exclude `&self` or `self`
            if let syn::Pat::Ident(ident) = &**pat {
                if ident.ident == "self" {
                    continue;
                }
            }

            // Collect struct fields (pattern: type)
            struct_fields.push(quote! { #pat: #ty });

            // Collect parameter names for struct initialization
            param_names.push(quote! { #pat });

            // Collect function call arguments (self.pattern)
            call_args.push(quote! { self.#pat.clone() });
        }
    }

    let (output_type, returns_result) = extract_output_type(fn_output);

    let task_struct = if struct_fields.is_empty() {
        quote! {
            pub struct #struct_name;
        }
    } else {
        quote! {
            pub struct #struct_name {
                #(pub #struct_fields),*
            }
        }
    };

    let benchmark_fn = if is_benchmark {
        quote! {
            fn benchmark() -> bool {
                true
            }
        }
    } else {
        quote! {}
    };

    let task_trait_path = quote! { #titan_core_path::Task };

    let execute_fn = if fn_asyncness.is_some() {
        if returns_result {
            quote! {
                async fn execute(self, subsystem: &#subsystem_ty) -> titan_core::Result<Self::Output> {
                    subsystem.#fn_name(#(#call_args),*).await
                }
            }
        } else {
            quote! {
                async fn execute(self, subsystem: &#subsystem_ty) -> titan_core::Result<Self::Output> {
                    let res = subsystem.#fn_name(#(#call_args),*).await?;
                    Ok(res)
                }
            }
        }
    } else if returns_result {
        quote! {
            async fn execute(self, subsystem: &#subsystem_ty) -> titan_core::Result<Self::Output> {
                let res = subsystem.#fn_name(#(#call_args),*)?;
                Ok(res)
            }
        }
    } else {
        quote! {
            async fn execute(self, subsystem: &#subsystem_ty) -> titan_core::Result<Self::Output> {
                Ok(subsystem.#fn_name(#(#call_args),*))
            }
        }
    };

    let task_impl = quote! {
        #[titan_core::async_trait]
        impl #task_trait_path<#subsystem_ty> for #struct_name {
            type Output = #output_type;
            #benchmark_fn
            #execute_fn
        }
    };

    let script_task_impl = quote! {
            #[allow(clippy::needless_borrow)]
            #[ad_astra::export]
            pub fn #script_task_name_fn(#(#struct_fields),*) ->#output_type {
                let task_instance = #struct_name {
                    #(#param_names),*
                };

                let channels = #titan_path::channels::channels();

                let future = channels.#channel_ident.send(task_instance);
                let res = titan_core::futures::executor::block_on(future);
                res.unwrap()
            }
    };

    quote! {
        #task_struct
        #task_impl

        #script_task_impl
    }
}

fn extract_output_type(fn_output: &ReturnType) -> (proc_macro2::TokenStream, bool) {
    match fn_output {
        syn::ReturnType::Type(_, ty) => {
            if let Type::Path(TypePath { path, .. }) = &**ty {
                // Check if the return type is Result<T>
                if let Some(segment) = path.segments.last() {
                    if segment.ident == "Result" {
                        // It's a Result<T>
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                return (quote! { #inner_ty }, true);
                            }
                        }
                    }
                }
            }
            // It's some other type T
            (quote! { #ty }, false)
        }
        syn::ReturnType::Default => {
            // No return type specified, so it's `()`
            (quote! { () }, false)
        }
    }
}
