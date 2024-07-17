use super::*;

/// Generates a `Deserialize` implementation.
pub fn expand(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    let ser = quote! { ::serdere };
    let mut ctx = DeserializeImplContext::new(input, &ser);
    Ok(match &input.data {
        syn::Data::Struct(st) => {
            let fields = deserialize_fields(&mut ctx, &st.fields)?;
            let name = input.ident.to_string();
            let body = quote! { Self #fields };
            ctx.generate_struct(&name, body)
        }
        syn::Data::Enum(en) => {
            let ser = ctx.ser;
            let mut variant_reprs = Vec::new();
            let mut index = 0;
            for variant in en.variants.iter() {
                variant_reprs.push(VariantRepr::get(variant, &mut index)?);
                index += 1;
            }
            let variant_name = variant_reprs.iter().map(|v| v.name.as_str());
            let variant_index = variant_reprs.iter().map(|v| v.index);
            let name_map = quote! {
                #ser::FixedNameMap::new([
                    #(
                        (#variant_name, #variant_index)
                    ),*
                ]).unfix()
            };
            // TODO: Handle empty enum
            let max_index = variant_reprs.iter().map(|v| v.index).max().unwrap();
            let variant_index = variant_reprs.iter().map(|v| v.index);
            match EnumRepr::get(&input.attrs, &input.ident, en)? {
                EnumRepr::Tag => {
                    let variant_ident = en.variants.iter().map(|v| &v.ident);
                    ctx.generate_value(
                        false,
                        quote! {{
                            const NAMES: &#ser::NameMap<usize> = #name_map;
                            let (de, done_flag) = value.into_raw();
                            let index = de.get_tag(#max_index, NAMES)?;
                            *done_flag = true;
                            match index {
                                #(
                                    #variant_index => Self::#variant_ident,
                                )*
                                index => {
                                    let err = de.error_invalid_index(index);
                                    return ::std::result::Result::Err(err);
                                }
                            }
                        }},
                    )
                }
                EnumRepr::Struct { name, tag } => {
                    let mut variant_body = Vec::new();
                    for (v, repr) in en.variants.iter().zip(variant_reprs.iter()) {
                        let variant_ident = &v.ident;
                        if repr.is_transparent {
                            let field_ty = match &v.fields {
                                syn::Fields::Named(fields) => {
                                    let field = fields.named.first().unwrap();
                                    let field_ident = field.ident.as_ref().unwrap();
                                    variant_body.push(quote! {
                                        Self::#variant_ident {
                                            #field_ident: st.inline_get_using(ctx)?
                                        }
                                    });
                                    &field.ty
                                }
                                syn::Fields::Unnamed(fields) => {
                                    let field = fields.unnamed.first().unwrap();
                                    variant_body.push(quote! {
                                        Self::#variant_ident(st.inline_get_using(ctx)?)
                                    });
                                    &field.ty
                                }
                                syn::Fields::Unit => unreachable!(),
                            };
                            let DeserializeImplContext {
                                ser,
                                d_ty,
                                ctx_ty,
                                where_clause,
                                ..
                            } = &mut ctx;
                            where_clause.predicates.push(
                                syn::parse2(quote! {
                                    #field_ty: #ser::deserialize::DeserializeStruct<#d_ty, #ctx_ty>
                                })
                                .unwrap(),
                            );
                        } else {
                            let fields = deserialize_fields(&mut ctx, &v.fields)?;
                            variant_body.push(quote! { Self::#variant_ident #fields });
                        }
                    }
                    ctx.generate_struct(
                        name.as_str(),
                        quote! {{
                            const NAMES: &#ser::NameMap<usize> = #name_map;
                            let (de, done_flag) = st.field(#tag)?.into_raw();
                            let index = de.get_tag(#max_index, NAMES)?;
                            *done_flag = true;
                            match index {
                                #(
                                    #variant_index => #variant_body,
                                )*
                                index => {
                                    let err = de.error_invalid_index(index);
                                    return ::std::result::Result::Err(err);
                                }
                            }
                        }},
                    )
                }
            }
        }
        syn::Data::Union(_) => todo!(),
    })
}

/// Encapsulate the context information required to generate a `Deserialize` implementation.
struct DeserializeImplContext<'a> {
    ser: &'a TokenStream,
    d_ty: syn::Ident,
    ctx_ty: syn::Ident,
    impl_generics_params: syn::punctuated::Punctuated<syn::GenericParam, syn::Token![,]>,
    ident: &'a syn::Ident,
    ty_generics: syn::TypeGenerics<'a>,
    where_clause: syn::WhereClause,
}

impl<'a> DeserializeImplContext<'a> {
    /// Creates a new [`DeserializeImplContext`] for the given input.
    pub fn new(input: &'a syn::DeriveInput, ser: &'a TokenStream) -> Self {
        let d_ty = syn::Ident::new("D", Span::call_site());
        let ctx_ty = syn::Ident::new("Ctx", Span::call_site());
        let (_, ty_generics, where_clause) = input.generics.split_for_impl();
        let mut impl_generics_params = input.generics.params.clone();
        impl_generics_params
            .push(syn::parse2(quote! { #d_ty: #ser::Deserializer + ?Sized }).unwrap());
        impl_generics_params.push(syn::parse2(quote! { #ctx_ty: ?Sized }).unwrap());
        let where_clause = where_clause.cloned().unwrap_or(syn::WhereClause {
            where_token: Default::default(),
            predicates: syn::punctuated::Punctuated::new(),
        });
        Self {
            ser,
            d_ty,
            ctx_ty,
            impl_generics_params,
            ident: &input.ident,
            ty_generics,
            where_clause,
        }
    }

    /// Generates a `Deserialize` implementation.
    pub fn generate_value(self, nullable: bool, body: TokenStream) -> TokenStream {
        let Self {
            ser,
            d_ty,
            ctx_ty,
            impl_generics_params,
            ident,
            ty_generics,
            where_clause,
            ..
        } = self;
        quote! {
            #[automatically_derived]
            impl <#impl_generics_params> #ser::Deserialize<#d_ty, #ctx_ty>
                for #ident #ty_generics
                #where_clause
            {
                const NULLABLE: bool = #nullable;
                fn deserialize(value: #ser::Value<#d_ty>, ctx: &mut #ctx_ty)
                    -> ::core::result::Result<Self, <#d_ty as #ser::Outliner>::Error>
                {
                    ::core::result::Result::Ok(#body)
                }
            }
        }
    }

    /// Generates a `DeserializeStruct` implementation.
    pub fn generate_struct(self, name: &str, body: TokenStream) -> TokenStream {
        let Self {
            ser,
            d_ty,
            ctx_ty,
            impl_generics_params,
            ident,
            ty_generics,
            where_clause,
            ..
        } = self;
        quote! {
            #[automatically_derived]
            impl <#impl_generics_params> #ser::Deserialize<#d_ty, #ctx_ty>
                for #ident #ty_generics
                #where_clause
            {
                const NULLABLE: bool = false;
                fn deserialize(value: #ser::Value<#d_ty>, ctx: &mut #ctx_ty)
                    -> ::core::result::Result<Self, <#d_ty as #ser::Outliner>::Error>
                {
                    #ser::deserialize::deserialize_struct(value, ctx,
                        ::core::option::Option::Some(#name))
                }
            }

            #[automatically_derived]
            impl <#impl_generics_params> #ser::deserialize::DeserializeStruct<#d_ty, #ctx_ty>
                for #ident #ty_generics
                #where_clause
            {
                fn deserialize_content(
                    st: &mut #ser::Struct<#d_ty>,
                    ctx: &mut #ctx_ty)
                    -> ::core::result::Result<Self, <#d_ty as #ser::Outliner>::Error>
                {
                    ::core::result::Result::Ok(#body)
                }
            }
        }
    }
}

/// Generates code to deserialize the fields of a struct or enum variant from a `Struct` named
/// `st`.
fn deserialize_fields(
    ctx: &mut DeserializeImplContext<'_>,
    fields: &syn::Fields,
) -> syn::Result<TokenStream> {
    Ok(match fields {
        syn::Fields::Named(fields) => {
            let mut body = TokenStream::new();
            for field in &fields.named {
                let field_ident = field.ident.as_ref().unwrap();
                let field_repr = FieldRepr::get(field)?;
                let deserialize = field_repr.deserialize(ctx, &field.ty);
                body.extend(quote! { #field_ident: #deserialize, });
            }
            quote! { { #body } }
        }
        syn::Fields::Unnamed(_) => todo!(),
        syn::Fields::Unit => TokenStream::new(),
    })
}

impl FieldRepr {
    /// Generates the code to deserialize a field with this representation and adds the required
    /// bounds to the `where` clause.
    fn deserialize(
        &self,
        ctx: &mut DeserializeImplContext<'_>,
        field_ty: &syn::Type,
    ) -> TokenStream {
        let DeserializeImplContext {
            ser,
            d_ty,
            ctx_ty,
            where_clause,
            ..
        } = ctx;
        let mut des_ty = field_ty;
        match &self.location {
            FieldLocation::Inlined => {
                let mut value = quote! { st.inline_get_using(ctx)? };
                apply_proxy(where_clause, &mut value, &mut des_ty, &self.proxy);
                where_clause.predicates.push(
                    syn::parse2(quote! {
                        #des_ty: #ser::deserialize::DeserializeStruct<#d_ty, #ctx_ty>
                    })
                    .unwrap(),
                );
                value
            }
            FieldLocation::Named { name, use_default } => {
                let res = if *use_default {
                    let mut value = quote! { value.get_using(ctx)? };
                    apply_proxy(where_clause, &mut value, &mut des_ty, &self.proxy);
                    where_clause
                        .predicates
                        .push(syn::parse2(quote! { #field_ty: ::core::default::Default }).unwrap());
                    quote! {{
                        let mut value = st.field(#name)?;
                        if value.check_null()? {
                            <#field_ty as ::core::default::Default>::default()
                        } else {
                            #value
                        }
                    }}
                } else {
                    let mut value = quote! { st.field(#name)?.get_using(ctx)? };
                    apply_proxy(where_clause, &mut value, &mut des_ty, &self.proxy);
                    value
                };
                where_clause.predicates.push(
                    syn::parse2(quote! { #des_ty: #ser::Deserialize<#d_ty, #ctx_ty> }).unwrap(),
                );
                res
            }
        }
    }
}

/// Applies proxy conversion to a parsed value if needed.
fn apply_proxy<'a>(
    where_clause: &mut syn::WhereClause,
    value: &mut TokenStream,
    des_ty: &mut &'a syn::Type,
    proxy: &'a Option<syn::Type>,
) {
    if let Some(proxy_ty) = proxy {
        where_clause.predicates.push(
            syn::parse2(quote! {
                #proxy_ty: ::core::convert::Into<#des_ty>
            })
            .unwrap(),
        );
        *value = quote! {
            <#proxy_ty as ::core::convert::Into<#des_ty>>::into(#value)
        };
        *des_ty = proxy_ty;
    }
}
