/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, Fields, Lit, Meta};
use synstructure::decl_derive;

decl_derive!([ServoPreferences] => servo_preferences_derive);
decl_derive!([EmbedderPreferences, attributes(namespace)] => embedder_preferences_derive);

/// A derive macro that adds string-based getter and setter for each field of this struct
/// (enums and other types are not supported). Each field must be able to be convertable
/// (with `into()`) into a `PrefValue`.
fn servo_preferences_derive(input: synstructure::Structure) -> TokenStream {
    let ast = input.ast();

    let Data::Struct(ref data) = ast.data else {
        unimplemented!();
    };
    let Fields::Named(ref named_fields) = data.fields else {
        unimplemented!()
    };

    let mut get_match_cases = quote!();
    for field in named_fields.named.iter() {
        let name = field.ident.as_ref().unwrap();
        get_match_cases.extend(quote!(stringify!(#name) => self.#name.clone().into(),))
    }

    let mut set_match_cases = quote!();
    for field in named_fields.named.iter() {
        let name = field.ident.as_ref().unwrap();
        set_match_cases.extend(quote!(stringify!(#name) => self.#name = value.try_into().unwrap(),))
    }

    let mut comparisons = quote!();
    for field in named_fields.named.iter() {
        let name = field.ident.as_ref().unwrap();
        comparisons.extend(quote!(if self.#name != other.#name { changes.push((stringify!(#name), self.#name.clone().into(),)) }))
    }

    let structure_name = &ast.ident;
    quote! {
        impl #structure_name {
            pub fn get_value(&self, name: &str) -> PrefValue {
                match name {
                    #get_match_cases
                    _ => { panic!("Unknown preference: {:?}", name); }
                }
            }

            pub fn set_value(&mut self, name: &str, value: PrefValue) {
                match name {
                    #set_match_cases
                    _ => { panic!("Unknown preference: {:?}", name); }
                }
            }

            pub fn diff(&self, other: &Self) -> Vec<(&'static str, PrefValue)> {
                let mut changes = vec![];
                #comparisons
                changes
            }
        }
    }
}

/// A derive macro for embedder preferences that implements the `EmbedderPreferences` trait.
///
/// This macro is similar to `ServoPreferences` but generates a trait implementation
/// instead of inherent methods, and returns `Option` types instead of panicking.
///
/// # Usage
///
/// ```rust,ignore
/// #[derive(Clone, Debug, EmbedderPreferences)]
/// #[namespace = "browserhtml"]
/// pub struct BrowserHtmlPreferences {
///     pub theme: String,
/// }
/// ```
fn embedder_preferences_derive(input: synstructure::Structure) -> TokenStream {
    let ast = input.ast();

    // Extract namespace from attribute
    let namespace = ast
        .attrs
        .iter()
        .find_map(|attr| {
            if attr.path().is_ident("namespace") {
                if let Meta::NameValue(meta) = &attr.meta {
                    if let syn::Expr::Lit(expr_lit) = &meta.value {
                        if let Lit::Str(lit) = &expr_lit.lit {
                            return Some(lit.value());
                        }
                    }
                }
            }
            None
        })
        .expect("EmbedderPreferences requires #[namespace = \"...\"] attribute");

    let Data::Struct(ref data) = ast.data else {
        panic!("EmbedderPreferences can only be derived for structs");
    };
    let Fields::Named(ref named_fields) = data.fields else {
        panic!("EmbedderPreferences requires named fields");
    };

    // Generate get_value match arms (returns Option<PrefValue>)
    let mut get_match_cases = quote!();
    for field in named_fields.named.iter() {
        let name = field.ident.as_ref().unwrap();
        get_match_cases.extend(quote!(
            stringify!(#name) => Some(self.#name.clone().into()),
        ));
    }

    // Generate set_value match arms (returns bool)
    let mut set_match_cases = quote!();
    for field in named_fields.named.iter() {
        let name = field.ident.as_ref().unwrap();
        set_match_cases.extend(quote!(
            stringify!(#name) => {
                if let Ok(v) = value.try_into() {
                    self.#name = v;
                    true
                } else {
                    false
                }
            },
        ));
    }

    // Generate diff comparisons
    let mut comparisons = quote!();
    for field in named_fields.named.iter() {
        let name = field.ident.as_ref().unwrap();
        comparisons.extend(quote!(
            if self.#name != other.#name {
                changes.push((stringify!(#name), self.#name.clone().into()));
            }
        ));
    }

    // Generate pref_names list
    let pref_names: Vec<_> = named_fields
        .named
        .iter()
        .map(|f| {
            let name = f.ident.as_ref().unwrap();
            quote!(stringify!(#name))
        })
        .collect();

    let structure_name = &ast.ident;
    let namespace_str = &namespace;

    quote! {
        impl servo_config::embedder_prefs::EmbedderPreferences for #structure_name {
            fn namespace() -> &'static str {
                #namespace_str
            }

            fn get_value(&self, name: &str) -> Option<servo_config::pref_util::PrefValue> {
                match name {
                    #get_match_cases
                    _ => None,
                }
            }

            fn set_value(&mut self, name: &str, value: servo_config::pref_util::PrefValue) -> bool {
                match name {
                    #set_match_cases
                    _ => false,
                }
            }

            fn diff(&self, other: &Self) -> Vec<(&'static str, servo_config::pref_util::PrefValue)> {
                let mut changes = vec![];
                #comparisons
                changes
            }

            fn pref_names() -> &'static [&'static str] {
                &[#(#pref_names),*]
            }
        }
    }
}
