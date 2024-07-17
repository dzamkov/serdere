mod deserialize;
mod serialize;

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::DeriveInput;
use syn::spanned::Spanned;

#[proc_macro_derive(Serialize, attributes(serde))]
pub fn derive_serialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(input as DeriveInput);
    serialize::expand(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Deserialize, attributes(serde))]
pub fn derive_deserialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(input as DeriveInput);
    deserialize::expand(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// The default name for the field which contains the tag for an enum.
const DEFAULT_TAG: &str = "type";

/// Describes how a struct is represented during serialization and deserialization.
enum StructRepr {
    /// Serialization and deserialization is deferred to the struct's sole field.
    Transparent,

    /// The struct is serialized and deserialized as a `Struct`.
    Struct {
        /// The name of the `Struct`.
        name: String,
    },
}

/// Describes how an enum is represented during serialization and deserialization.
enum EnumRepr {
    /// The "enum" is serialized and deserialized as a single tag.
    Tag,

    /// The "enum" is serialized and deserialized as a `Struct`.
    Struct {
        /// The name of the `Struct`.
        name: String,

        /// The name of field which contains the tag for this enum.
        tag: String,
    },
}

/// Describes how an enum variant is represented during serialization and deserialization.
struct VariantRepr {
    /// The name of this variant.
    name: String,

    /// The index of this variant.
    index: usize,

    /// Indicates whether serialization and deserialization is deferred to the variant's sole
    /// field.
    is_transparent: bool,
}

/// Describes how a field is represented during serialization and deserialization in a `Struct`.
struct FieldRepr {
    /// Specifies a "proxy" type that the field is serialized and/or deserialized.
    proxy: Option<syn::Type>,

    /// The location of the data for this field in its serialized form.
    location: FieldLocation,
}

/// Describes the location of the data of a field in its serialized form.
enum FieldLocation {
    /// The field is inlined into the `Struct`, spreading out across multiple `Struct` fields.
    Inlined,

    /// The field is serialized as a `Struct` field.
    Named {
        /// The name of the field in its serialized form.
        name: String,

        /// If `true`, the field value will be `null`-checked during deserialization and `null`
        /// values will be replaced with [`Default::default()`].
        use_default: bool,
    },
}

impl EnumRepr {
    /// Gets the representation for the given enum.
    pub fn get(
        attrs: &[syn::Attribute],
        ident: &syn::Ident,
        en: &syn::DataEnum,
    ) -> syn::Result<Self> {
        let mut rename = None;
        let mut tag = None;

        // Parse attributes
        for attr in attrs.iter() {
            if attr.path().is_ident("serde") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        let lit: syn::LitStr = meta.value()?.parse()?;
                        rename = Some(lit.value());
                    } else if meta.path.is_ident("tag") {
                        let lit: syn::LitStr = meta.value()?.parse()?;
                        tag = Some(lit.value());
                    } else {
                        let path = meta.path.to_token_stream().to_string().replace(' ', "");
                        return Err(
                            meta.error(format_args!("unknown serde enum attribute `{}`", path))
                        );
                    }
                    Ok(())
                })?;
            }
        }

        // Determine whether to use tag representation
        let mut use_tag_repr = tag.is_none();
        if use_tag_repr {
            for variant in en.variants.iter() {
                if !matches!(variant.fields, syn::Fields::Unit) {
                    use_tag_repr = false;
                    break;
                }
            }
        }

        // Construct representation
        Ok(if use_tag_repr {
            EnumRepr::Tag
        } else {
            EnumRepr::Struct {
                name: rename.unwrap_or_else(|| ident.to_string()),
                tag: tag.unwrap_or_else(|| DEFAULT_TAG.to_string()),
            }
        })
    }
}

impl VariantRepr {
    /// Gets the representation for the given variant.
    pub fn get(variant: &syn::Variant, index: &mut usize) -> syn::Result<Self> {
        let mut rename = None;
        let mut reindex = None;
        let mut is_transparent = false;
        for attr in variant.attrs.iter() {
            if attr.path().is_ident("serde") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        let lit: syn::LitStr = meta.value()?.parse()?;
                        rename = Some(lit.value());
                    } else if meta.path.is_ident("reindex") {
                        let lit: syn::LitInt = meta.value()?.parse()?;
                        reindex = Some(lit.base10_parse()?);
                    } else if meta.path.is_ident("transparent") {
                        is_transparent = true;
                        if variant.fields.len() != 1 {
                            return Err(
                                meta.error("transparent variants must have exactly one field")
                            );
                        }
                    } else {
                        let path = meta.path.to_token_stream().to_string().replace(' ', "");
                        return Err(
                            meta.error(format_args!("unknown serde variant attribute `{}`", path))
                        );
                    }
                    Ok(())
                })?;
            }
        }

        // Parse discriminant
        if let Some((_, discriminant)) = &variant.discriminant {
            match discriminant {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int), .. }) => {
                    *index = int.base10_parse()?;
                }
                _ => {
                    return Err(syn::Error::new(
                        discriminant.span(),
                        "serialization requires integer literal for enum discriminant",
                    ));
                }
            }
        }

        // Construct representation
        Ok(VariantRepr {
            name: rename.unwrap_or_else(|| variant.ident.to_string()),
            index: reindex.unwrap_or(*index),
            is_transparent,
        })
    }
}

impl FieldRepr {
    /// Gets the representation for the given field.
    pub fn get(field: &syn::Field) -> syn::Result<Self> {
        let mut is_inlined = false;
        let mut rename = None;
        let mut proxy = None;
        let mut use_default = false;
        for attr in field.attrs.iter() {
            if attr.path().is_ident("serde") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("flatten") {
                        is_inlined = true;
                    } else if meta.path.is_ident("rename") {
                        let lit: syn::LitStr = meta.value()?.parse()?;
                        rename = Some(lit.value());
                    } else if meta.path.is_ident("proxy") {
                        let ty: syn::Type = meta.value()?.parse()?;
                        proxy = Some(ty);
                    } else if meta.path.is_ident("default") {
                        use_default = true;
                    } else {
                        let path = meta.path.to_token_stream().to_string().replace(' ', "");
                        return Err(
                            meta.error(format_args!("unknown serde field attribute `{}`", path))
                        );
                    }
                    Ok(())
                })?;
            }
        }
        Ok(FieldRepr {
            proxy,
            location: if is_inlined {
                // TODO: Check for incompatible attributes
                FieldLocation::Inlined
            } else {
                FieldLocation::Named {
                    name: rename.unwrap_or_else(|| {
                        field
                            .ident
                            .as_ref()
                            .expect("field name required for serialization")
                            .to_string()
                    }),
                    use_default
                }
            },
        })
    }
}