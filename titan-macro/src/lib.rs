use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::quote;
use syn::token::{Comma, Paren};
use syn::{
    Attribute, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, PatType, ReturnType, Type, TypePath,
    punctuated::Punctuated,
};
use syn::{parse_macro_input, LitStr, Meta, TypeTuple};

fn is_task_attribute(attr: &syn::Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .map_or(false, |segment| segment.ident == "task")
}

#[proc_macro_attribute]
pub fn subsystem(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    let path = match &*input.self_ty {
        Type::Path(type_path) => {
            let path_str = type_path.path.segments.iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            
            path_str  
        },
        _ => panic!("Expected Type::Path in impl!")
    };

    // Separate items that have a #[task] attribute from those that don't.
    let (task_functions, other_items): (Vec<_>, Vec<_>) =
        input.items.into_iter().partition(|item| match item {
            ImplItem::Fn(method) => method.attrs.iter().any(is_task_attribute),

            _ => false,
        });

    // Process each task-annotated method, generating the associated tasks and
    // stripping off the #[task] attribute from the method.
    let mut generated_tasks = Vec::new();
    let mut updated_functions = Vec::new();

    for item in task_functions {
        if let ImplItem::Fn(mut function) = item {
            // Extract the task attribute
            let task_attr = function
                .attrs
                .iter()
                .find(|attr| is_task_attribute(attr))
                .cloned()
                .expect("Expected a #[task] attribute");

            // Generate the task code
            let generated_task = generate_task(&input.self_ty, function.clone(), task_attr, path.clone());
            generated_tasks.push(generated_task);

            // Remove the #[task] attribute from the original method
            function.attrs.retain(|attr| !attr.path().is_ident("task"));
            updated_functions.push(ImplItem::Fn(function));
        }
    }

    // Combine the updated methods (task methods without the attribute)
    // and the other items (which had no #[task]) to form a new impl block.
    let mut impl_items = other_items;
    impl_items.extend(updated_functions);

    let updated_impl = ItemImpl {
        items: impl_items,
        ..input
    };

    let titan_core_path = get_crate_path("titan_core")
        .expect("Failed to find titan_core");
    
    let self_ty = &updated_impl.self_ty;

    let expanded = quote! {
        #updated_impl

        impl #titan_core_path::Subsystem for #self_ty {}

        #(#generated_tasks)*
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
    function: ImplItemFn,
    macro_attributes: Attribute,
    module_path: String,
) -> proc_macro2::TokenStream {
    let task_data = extract_task_function_data(function, macro_attributes);

    build_task(self_ty, &task_data, module_path)
}

// Supported: #[task(benchmark, io)]
#[derive(Debug)]
struct TaskMacroAttributes {
    pub benchmark: bool,
    pub io: bool,
}

fn extract_macro_attributes(macro_attributes: &Attribute) -> TaskMacroAttributes {
    eprintln!("extract macro attributes");

    let last_path_segment = macro_attributes.path()
        .segments
        .last()
        .unwrap();

    let nested_meta = if last_path_segment.ident == "task" {
        match &macro_attributes.meta {
            Meta::List(nested_meta) => {
                eprintln!("Nested meta found");
                Some(nested_meta.clone())
            },
            _ => None
        }
    } else {
        None
    };

    match nested_meta {
        Some(_) => {
            let mut benchmark = false;
            let mut io = false;

            // If `#[task]` has no parentheses, `parse_nested_meta` won't call the closure.
            // If `#[task(...)]` has arguments, the closure is called for each nested meta item.
            let _ = macro_attributes.parse_nested_meta(|meta| {
                 
                if meta.path.is_ident("benchmark") {
                    benchmark = true;
                    Ok(())
                } else if meta.path.is_ident("io") {
                    io = true;
                    Ok(())
                } else {
                    eprintln!("Error parsing nested meta for task attribute");
                    Err(meta.error("unsupported argument in #[task] attribute"))
                }
            });
            
            TaskMacroAttributes { benchmark, io }
        },
        None => {
            eprintln!("No nested meta found");
            TaskMacroAttributes { benchmark: false, io: false }
        },
    }
}

#[derive(Debug)]
struct TaskFunctionData {
    pub name: syn::Ident,
    pub input_types: Vec<syn::Type>,
    pub input_names: Vec<syn::Pat>,
    pub is_async: bool,
    pub returns_result: bool,
    pub output_type: syn::Type,
    pub macro_attributes: TaskMacroAttributes,
    pub generics: syn::Generics,
    pub is_mut: bool,
}

fn extract_task_function_data(method: ImplItemFn, macro_attributes: Attribute) -> TaskFunctionData {
    
    let task_name = method.sig.ident;
    let task_async = method.sig.asyncness.is_some();
    let task_input = method.sig.inputs;
    let task_output = method.sig.output;
    let task_generics = method.sig.generics;

    eprintln!();
    eprintln!("{}", task_name);
    eprintln!("extract task data");

    let (task_input_types, task_input_names, task_mutability) = extract_params(task_input);
    let (task_output_type, task_returns_result) = extract_output(task_output);

    let macro_attributes = extract_macro_attributes(&macro_attributes);

    TaskFunctionData {
        name: task_name,
        input_types: task_input_types,
        input_names: task_input_names,
        is_async: task_async,
        returns_result: task_returns_result,
        output_type: task_output_type,
        macro_attributes,
        generics: task_generics,
        is_mut: task_mutability,
    }
}

fn extract_params(task_params: Punctuated<FnArg, Comma>) -> (Vec<syn::Type>, Vec<syn::Pat>, bool) {
    eprintln!("extract params");
    
    let mut task_call_param_types = Vec::new();
    let mut task_call_param_names = Vec::new();
    let mut task_is_mut = false;

    for param in task_params.iter() {
        match param {
            FnArg::Receiver(receiver) => {
                if receiver.mutability.is_some() {
                    task_is_mut = true;
                }
                continue;
            },
            FnArg::Typed(PatType { pat, ty, .. }) => {
                // Exclude `&self` or `self`
                if let syn::Pat::Ident(ident) = &(**pat) {
                    if ident.ident == "self" {                    
                        continue;
                    }
                }

                // Collect parameter names
                task_call_param_names.push((**pat).clone());

                // Collect parameter types
                task_call_param_types.push((**ty).clone());
            }
        }        
    }

    (task_call_param_types, task_call_param_names, task_is_mut)
}

fn extract_output(task_output: ReturnType) -> (syn::Type, bool) {
    eprintln!("extract output");
    
    match task_output {
        syn::ReturnType::Type(_, ty) => {
            if let Type::Path(TypePath { path, .. }) = &(*ty) {
                // Check if the return type is Result<T>
                if let Some(segment) = path.segments.last() {
                    if segment.ident == "Result" {
                        // It's a Result<T>
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                return (inner_ty.clone(), true);
                            }
                        }
                    }
                }
            }
            // It's some other type T
            ((*ty).clone(), false)
        }
        syn::ReturnType::Default => {
            // No return type specified, so it's `()`
            (
                syn::Type::Tuple(TypeTuple {
                    paren_token: Paren::default(),
                    elems: Punctuated::new(),
                }),
                false,
            )
        }
    }
}

fn build_task(
    subsystem_type: &syn::Type,
    task_data: &TaskFunctionData,
    module_path: String,
) -> proc_macro2::TokenStream {
    eprintln!("build task");
    
    let task_struct = build_task_struct(task_data);    
    let task_impl = build_task_impl(subsystem_type, task_data, module_path);

    quote! {
        #task_struct
        #task_impl
    }
}

fn build_task_struct(task_data: &TaskFunctionData) -> proc_macro2::TokenStream {
    eprintln!("build task struct");

    let task_name = get_task_name(&task_data.name.to_string());
    let generics = &task_data.generics;
    let where_clause = &task_data.generics.where_clause;

    // Build fields if we have parameters
    let task_fields = task_data
        .input_types
        .iter()
        .zip(&task_data.input_names)
        .map(|(ty, name)| {
            // Each field is "pub name: type"
            quote! { pub #name: #ty }
        });

    // Generate the struct implementation
    match task_data.input_types.is_empty() {
        true => {
            quote! {
                pub struct #task_name;
            }
        }
        false => {
            quote! {
                pub struct #task_name #generics
                #where_clause {
                    #(#task_fields),*
                }
            }
        }
    }
}

fn build_task_impl(
    subsystem_type: &syn::Type,
    task_data: &TaskFunctionData,
    module_path: String,
) -> proc_macro2::TokenStream {
    eprintln!("build task impl");
     
    let titan_core_path = get_crate_path("titan_core")
        .expect("Failed to find titan_core!");

    let task_name = get_task_name(&task_data.name.to_string());
    let output_type = &task_data.output_type;
    let id_fn = build_id_functions(task_data, module_path);
    let benchmark_fn = build_task_benchmark_function(task_data);
    let io_fn = build_task_io_function(task_data);
    let execute_fn = build_task_execute_function(subsystem_type, task_data);
    let generics = &task_data.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        #[#titan_core_path::async_trait]
        impl #impl_generics #titan_core_path::Task<#subsystem_type> for #task_name #ty_generics
        #where_clause {
            type Output = #output_type;
            #id_fn
            #io_fn
            #benchmark_fn
            #execute_fn
        }
    }
}

fn build_id_functions(task_data: &TaskFunctionData, module_path: String) -> proc_macro2::TokenStream {
    
    let module_name = module_path.split("::")
        .next()
        .unwrap_or("unknown");

    let task_name = get_task_name(&task_data.name.to_string());
    let task_name = format!("{}::{}", &module_name, task_name);
    let task_name = LitStr::new(&task_name.to_string(), Span::call_site());
    let task_is_mut = task_data.is_mut;

    quote!{
        fn name() -> &'static str {
            #task_name
        }

        fn is_mut() -> bool {
            #task_is_mut
        }
    }
}

fn build_task_benchmark_function(task_data: &TaskFunctionData) -> proc_macro2::TokenStream {
    eprintln!("build task benchmark fn");
 
    match task_data.macro_attributes.benchmark {
        true => quote! {
            fn benchmark() -> bool {
                true
            }
        },
        false => quote! {},
    }
}

fn build_task_io_function(task_data: &TaskFunctionData) -> proc_macro2::TokenStream { 
    eprintln!("build task io fn");
    
    match task_data.macro_attributes.io {
        true => quote! {
            fn io() -> bool {
                true
            }
        },
        false => quote! {},
    }
}

fn build_task_execute_function(
    subsystem_type: &syn::Type,
    task_data: &TaskFunctionData,
) -> proc_macro2::TokenStream {
    eprintln!("build task execute fn");
    
    let titan_core_path = get_crate_path("titan_core")
        .expect("Failed to find titan_core!");
    
    let task_name = &task_data.name;
    
    let task_args = task_data.input_names.iter()
        .map(|name| {
            quote! { self.#name }
        });
    
    let execute_call = quote! { subsystem.#task_name(#(#task_args),*) };
    
    // Determine if `.await` should be appended
    let await_execute = if task_data.is_async {
        quote! { .await }
    } else {
        quote! {}
    };
        
    // Determine if `?` should be used for error handling
    let result_execute = if task_data.returns_result {
        quote! { ? }
    } else {
        quote! {}
    };
        
    // Conditionally generate the `execute_mut` function if `is_mut` is true
    if task_data.is_mut {
        quote! {
            async fn execute_mut(self, subsystem: &mut #subsystem_type) -> #titan_core_path::Result<Self::Output> {
                Ok(#execute_call #await_execute #result_execute)
            }
        }
    } else {
        quote! {
            async fn execute(self, subsystem: &#subsystem_type) -> #titan_core_path::Result<Self::Output> {
                Ok(#execute_call #await_execute #result_execute)
            }
        }
    }    
}

fn get_task_name(function_name: &str) -> syn::Ident {
    let name = function_name
        .split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<String>();

    Ident::new(&name, Span::call_site())
}

fn get_crate_path(name: &str) -> anyhow::Result<proc_macro2::TokenStream, syn::Error> {
    match crate_name(name) {
        Ok(FoundCrate::Itself) => Ok(quote!(crate)),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            Ok(quote!(#ident))
        }
        Err(err) => {
            eprintln!("Error finding crate: {}", err);
            Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("Crate '{}' not found", name),
            ))
        },
    }
}
