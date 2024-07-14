use super::*;

/// Generates a `Serialize` implementation.
pub fn expand(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    let ser = quote! { ::serdere };
    let mut ctx = SerializeImplContext::new(input, &ser);
    Ok(match &input.data {
        syn::Data::Struct(st) => {
            let (fields, body) = serialize_fields(&mut ctx, &st.fields)?;
            let name = input.ident.to_string();
            ctx.generate_struct(
                &name,
                quote! {
                    let Self #fields = self;
                    #body
                },
            )
        }
        syn::Data::Enum(en) => {
            let variant_reprs = en
                .variants
                .iter()
                .map(VariantRepr::get)
                .collect::<syn::Result<Vec<_>>>()?;
            let max_index = en.variants.len() - 1;
            match EnumRepr::get(&input.attrs, &input.ident, en)? {
                EnumRepr::Tag => {
                    let variant_index = 0usize..;
                    let variant_ident = en.variants.iter().map(|v| &v.ident);
                    let variant_name = variant_reprs.iter().map::<&str, _>(|v| v.name.as_ref());
                    ctx.generate_value(
                        false,
                        quote! {
                            match self {
                                #(
                                    Self::#variant_ident => value.put_tag(
                                        #max_index,
                                        #variant_index,
                                        Some(#variant_name)
                                    )?,
                                )*
                            }
                        },
                    )
                }
                EnumRepr::Struct { name, tag } => {
                    let mut variant_arm = Vec::new();
                    for (variant_index, (v, repr)) in
                        en.variants.iter().zip(variant_reprs).enumerate()
                    {
                        let ident = &v.ident;
                        let (fields, body) = if repr.is_transparent {
                            let (fields, field_ty) = match &v.fields {
                                syn::Fields::Named(fields) => {
                                    let field = fields.named.first().unwrap();
                                    let field_ident = field.ident.as_ref().unwrap();
                                    (quote! { { #field_ident: inner } }, &field.ty)
                                }
                                syn::Fields::Unnamed(fields) => {
                                    let field = fields.unnamed.first().unwrap();
                                    (quote! { (inner) }, &field.ty)
                                }
                                syn::Fields::Unit => unreachable!(),
                            };
                            let SerializeImplContext {
                                ser,
                                s_ty,
                                ctx_ty,
                                where_clause,
                                ..
                            } = &mut ctx;
                            where_clause.predicates.push(
                                syn::parse2(quote! {
                                    #field_ty: #ser::serialize::SerializeStruct<#s_ty, #ctx_ty>
                                })
                                .unwrap(),
                            );
                            (fields, quote! { st.inline_put_using(inner, ctx)?; })
                        } else {
                            serialize_fields(&mut ctx, &v.fields)?
                        };
                        let variant_name: &str = repr.name.as_ref();
                        variant_arm.push(quote! {
                            Self::#ident #fields => {
                                st.field(#tag)?.put_tag(
                                    #max_index,
                                    #variant_index,
                                    Some(#variant_name)
                                )?;
                                #body
                            }
                        });
                    }
                    ctx.generate_struct(
                        name.as_str(),
                        quote! {
                            match self {
                                #(#variant_arm),*
                            }
                        },
                    )
                }
            }
        }
        syn::Data::Union(_) => todo!(),
    })
}

/// Encapsulate the context information required to generate a `Serialize` implementation.
struct SerializeImplContext<'a> {
    ser: &'a TokenStream,
    s_ty: syn::Ident,
    ctx_ty: syn::Ident,
    impl_generics_params: syn::punctuated::Punctuated<syn::GenericParam, syn::Token![,]>,
    ident: &'a syn::Ident,
    ty_generics: syn::TypeGenerics<'a>,
    where_clause: syn::WhereClause,
}

impl<'a> SerializeImplContext<'a> {
    /// Creates a new [`SerializeImplContext`] for the given input.
    pub fn new(input: &'a syn::DeriveInput, ser: &'a TokenStream) -> Self {
        let s_ty = syn::Ident::new("S", Span::call_site());
        let ctx_ty = syn::Ident::new("Ctx", Span::call_site());
        let (_, ty_generics, where_clause) = input.generics.split_for_impl();
        let mut impl_generics_params = input.generics.params.clone();
        impl_generics_params
            .push(syn::parse2(quote! { #s_ty: #ser::Serializer + ?Sized }).unwrap());
        impl_generics_params.push(syn::parse2(quote! { #ctx_ty: ?Sized }).unwrap());
        let where_clause = where_clause.cloned().unwrap_or(syn::WhereClause {
            where_token: Default::default(),
            predicates: syn::punctuated::Punctuated::new(),
        });
        Self {
            ser,
            s_ty,
            ctx_ty,
            impl_generics_params,
            ident: &input.ident,
            ty_generics,
            where_clause,
        }
    }

    /// Generates a `Serialize` implementation.
    pub fn generate_value(self, nullable: bool, body: TokenStream) -> TokenStream {
        let Self {
            ser,
            s_ty,
            ctx_ty,
            impl_generics_params,
            ident,
            ty_generics,
            where_clause,
            ..
        } = self;
        quote! {
            #[automatically_derived]
            impl <#impl_generics_params> #ser::Serialize<#s_ty, #ctx_ty>
                for #ident #ty_generics
                #where_clause
            {
                const NULLABLE: bool = #nullable;
                fn serialize(&self, value: #ser::Value<#s_ty>, ctx: &mut #ctx_ty)
                    -> ::core::result::Result<(), <#s_ty as #ser::Outliner>::Error>
                {
                    #body
                    ::core::result::Result::Ok(())
                }
            }
        }
    }

    /// Generates a `SerializeStruct` implementation.
    pub fn generate_struct(self, name: &str, body: TokenStream) -> TokenStream {
        let Self {
            ser,
            s_ty,
            ctx_ty,
            impl_generics_params,
            ident,
            ty_generics,
            where_clause,
            ..
        } = self;
        quote! {
            #[automatically_derived]
            impl <#impl_generics_params> #ser::Serialize<#s_ty, #ctx_ty>
                for #ident #ty_generics
                #where_clause
            {
                const NULLABLE: bool = false;
                fn serialize(&self, value: #ser::Value<#s_ty>, ctx: &mut #ctx_ty)
                    -> ::core::result::Result<(), <#s_ty as #ser::Outliner>::Error>
                {
                    #ser::serialize::serialize_struct(value, self, ctx,
                        ::core::option::Option::Some(#name))
                }
            }

            #[automatically_derived]
            impl <#impl_generics_params> #ser::serialize::SerializeStruct<#s_ty, #ctx_ty>
                for #ident #ty_generics
                #where_clause
            {
                fn serialize_content(
                    &self,
                    st: &mut #ser::Struct<#s_ty>,
                    ctx: &mut #ctx_ty)
                    -> ::core::result::Result<(), <#s_ty as #ser::Outliner>::Error>
                {
                    #body
                    ::core::result::Result::Ok(())
                }
            }
        }
    }
}

/// Generates code to serialize the fields of a struct or enum variant into a `Struct` named
/// `st`.
fn serialize_fields(
    ctx: &mut SerializeImplContext<'_>,
    fields: &syn::Fields,
) -> syn::Result<(TokenStream, TokenStream)> {
    Ok(match fields {
        syn::Fields::Named(fields) => {
            let mut cons = TokenStream::new();
            let mut body = TokenStream::new();
            for field in &fields.named {
                let field_ident = field.ident.as_ref().unwrap();
                let field_repr = FieldRepr::get(field)?;
                let serialize = field_repr.serialize(ctx, &field.ty, quote! { #field_ident });
                cons.extend(quote! { #field_ident, });
                body.extend(serialize);
            }
            (quote! { { #cons } }, body)
        }
        syn::Fields::Unnamed(_) => todo!(),
        syn::Fields::Unit => (quote! {}, TokenStream::new()),
    })
}

impl FieldRepr {
    /// Generates the code to serialize a field with this representation and adds the required
    /// bounds to the `where` clause.
    fn serialize(
        &self,
        ctx: &mut SerializeImplContext<'_>,
        field_ty: &syn::Type,
        value: TokenStream,
    ) -> TokenStream {
        let SerializeImplContext {
            ser,
            s_ty,
            ctx_ty,
            where_clause,
            ..
        } = ctx;
        if let Some(proxy_ty) = &self.proxy {
            todo!()
        }
        match &self.location {
            FieldLocation::Inlined => {
                where_clause.predicates.push(
                    syn::parse2(
                        quote! { #field_ty: #ser::serialize::SerializeStruct<#s_ty, #ctx_ty> },
                    )
                    .unwrap(),
                );
                quote! { st.inline_put_using(#value, ctx)?; }
            }
            FieldLocation::Named { name, .. } => {
                where_clause.predicates.push(
                    syn::parse2(quote! { #field_ty: #ser::Serialize<#s_ty, #ctx_ty> }).unwrap(),
                );
                quote! { st.field(#name)?.put_using(#value, ctx)?; }
            }
        }
    }
}
