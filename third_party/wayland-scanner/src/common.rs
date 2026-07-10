use std::fmt::Write;

use proc_macro2::{Ident, Literal, Span, TokenStream};

use quote::{format_ident, quote, ToTokens};

use crate::{protocol::*, util::*, Side};

pub(crate) fn generate_enums_for(interface: &Interface) -> TokenStream {
    interface.enums.iter().map(ToTokens::into_token_stream).collect()
}

impl ToTokens for Enum {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let enum_decl;
        let enum_impl;

        let doc_attr = self.description.as_ref().map(description_to_doc_attr);
        let ident = Ident::new(&snake_to_camel(&self.name), Span::call_site());

        if self.bitfield {
            let entries = self.entries.iter().map(|entry| {
                let doc_attr = entry
                    .description
                    .as_ref()
                    .map(description_to_doc_attr)
                    .or_else(|| entry.summary.as_ref().map(|s| to_doc_attr(s)));

                let prefix = if entry.name.chars().next().unwrap().is_numeric() { "_" } else { "" };
                let ident = format_ident!("{}{}", prefix, snake_to_camel(&entry.name));

                let value = Literal::u32_unsuffixed(entry.value);

                quote! {
                    #doc_attr
                    const #ident = #value;
                }
            });

            enum_decl = quote! {
                bitflags::bitflags! {
                    #doc_attr
                    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
                    pub struct #ident: u32 {
                        #(#entries)*
                    }
                }
            };
            enum_impl = quote! {
                impl std::convert::TryFrom<u32> for #ident {
                    type Error = ();
                    fn try_from(val: u32) -> Result<#ident, ()> {
                        #ident::from_bits(val).ok_or(())
                    }
                }
                impl std::convert::From<#ident> for u32 {
                    fn from(val: #ident) -> u32 {
                        val.bits()
                    }
                }
            };
        } else {
            let variants = self.entries.iter().map(|entry| {
                let doc_attr = entry
                    .description
                    .as_ref()
                    .map(description_to_doc_attr)
                    .or_else(|| entry.summary.as_ref().map(|s| to_doc_attr(s)));

                let prefix = if entry.name.chars().next().unwrap().is_numeric() { "_" } else { "" };
                let variant = format_ident!("{}{}", prefix, snake_to_camel(&entry.name));

                let value = Literal::u32_unsuffixed(entry.value);

                quote! {
                    #doc_attr
                    #variant = #value
                }
            });

            enum_decl = quote! {
                #doc_attr
                #[repr(u32)]
                #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
                #[non_exhaustive]
                pub enum #ident {
                    #(#variants,)*
                }
            };

            let match_arms = self.entries.iter().map(|entry| {
                let value = Literal::u32_unsuffixed(entry.value);

                let prefix = if entry.name.chars().next().unwrap().is_numeric() { "_" } else { "" };
                let variant = format_ident!("{}{}", prefix, snake_to_camel(&entry.name));

                quote! {
                    #value => Ok(#ident::#variant)
                }
            });

            enum_impl = quote! {
                impl std::convert::TryFrom<u32> for #ident {
                    type Error = ();
                    fn try_from(val: u32) -> Result<#ident, ()> {
                        match val {
                            #(#match_arms,)*
                            _ => Err(())
                        }
                    }
                }
                impl std::convert::From<#ident> for u32 {
                    fn from(val: #ident) -> u32 {
                        val as u32
                    }
                }
            };
        }

        enum_decl.to_tokens(tokens);
        enum_impl.to_tokens(tokens);
    }
}

pub(crate) fn gen_msg_constants(requests: &[Message], events: &[Message]) -> TokenStream {
    let req_constants = requests.iter().enumerate().map(|(opcode, msg)| {
        let since_cstname = format_ident!("REQ_{}_SINCE", msg.name.to_ascii_uppercase());
        let opcode_cstname = format_ident!("REQ_{}_OPCODE", msg.name.to_ascii_uppercase());
        let since = msg.since;
        let opcode = opcode as u16;
        quote! {
            /// The minimal object version supporting this request
            pub const #since_cstname: u32 = #since;
            /// The wire opcode for this request
            pub const #opcode_cstname: u16 = #opcode;
        }
    });
    let evt_constants = events.iter().enumerate().map(|(opcode, msg)| {
        let since_cstname = format_ident!("EVT_{}_SINCE", msg.name.to_ascii_uppercase());
        let opcode_cstname = format_ident!("EVT_{}_OPCODE", msg.name.to_ascii_uppercase());
        let since = msg.since;
        let opcode = opcode as u16;
        quote! {
            /// The minimal object version supporting this event
            pub const #since_cstname: u32 = #since;
            /// The wire opcode for this event
            pub const #opcode_cstname: u16 = #opcode;
        }
    });

    quote! {
        #(#req_constants)*
        #(#evt_constants)*
    }
}

pub(crate) fn gen_message_enum(
    name: &Ident,
    side: Side,
    receiver: bool,
    messages: &[Message],
) -> TokenStream {
    let variants = messages
        .iter()
        .map(|msg| {
            let mut docs = String::new();
            if let Some((ref short, ref long)) = msg.description {
                write!(docs, "{}\n\n{}\n", short, long.trim()).unwrap();
            }
            if let Some(Type::Destructor) = msg.typ {
                write!(
                    docs,
                    "\nThis is a destructor, once {} this object cannot be used any longer.",
                    if receiver { "received" } else { "sent" },
                )
                .unwrap()
            }
            if msg.since > 1 {
                write!(docs, "\nOnly available since version {} of the interface", msg.since)
                    .unwrap();
            }

            let doc_attr = to_doc_attr(&docs);
            let msg_name = Ident::new(&snake_to_camel(&msg.name), Span::call_site());
            let msg_variant_decl =
                if msg.args.is_empty() {
                    msg_name.into_token_stream()
                } else {
                    let fields = msg.args.iter().flat_map(|arg| {
                let field_name =
                    format_ident!("{}{}", if is_keyword(&arg.name) { "_" } else { "" }, arg.name);
                let field_type_inner = if let Some(ref enu) = arg.enum_ {
                    let enum_type = dotted_to_relname(enu);
                    quote! { WEnum<#enum_type> }
                } else {
                    match arg.typ {
                        Type::Uint => quote! { u32 },
                        Type::Int => quote! { i32 },
                        Type::Fixed => quote! { f64 },
                        Type::String => quote! { String },
                        Type::Array => quote! { Vec<u8> },
                        Type::Fd => {
                            if receiver {
                                quote! { OwnedFd }
                            } else {
                                quote! { std::os::unix::io::BorrowedFd<'a> }
                            }
                        }
                        Type::Object => {
                            if let Some(ref iface) = arg.interface {
                                let iface_mod = Ident::new(iface, Span::call_site());
                                let iface_type =
                                    Ident::new(&snake_to_camel(iface), Span::call_site());
                                quote! { super::#iface_mod::#iface_type }
                            } else if side == Side::Client {
                                quote! { super::wayland_client::ObjectId }
                            } else {
                                quote! { super::wayland_server::ObjectId }
                            }
                        }
                        Type::NewId if !receiver && side == Side::Client => {
                            // Client-side sending does not have a pre-existing object
                            // so skip serializing it
                            if arg.interface.is_some() {
                                return None;
                            } else {
                                quote! { (&'static Interface, u32) }
                            }
                        }
                        Type::NewId => {
                            if let Some(ref iface) = arg.interface {
                                let iface_mod = Ident::new(iface, Span::call_site());
                                let iface_type =
                                    Ident::new(&snake_to_camel(iface), Span::call_site());
                                if receiver && side == Side::Server {
                                    quote! { New<super::#iface_mod::#iface_type> }
                                } else {
                                    quote! { super::#iface_mod::#iface_type }
                                }
                            } else {
                                // bind-like function
                                if side == Side::Client {
                                    quote! { (String, u32, super::wayland_client::ObjectId) }
                                } else {
                                    quote! { (String, u32, super::wayland_server::ObjectId) }
                                }
                            }
                        }
                        Type::Destructor => panic!("An argument cannot have type \"destructor\"."),
                    }
                };

                let field_type = if arg.allow_null {
                    quote! { Option<#field_type_inner> }
                } else {
                    field_type_inner.into_token_stream()
                };

                let doc_attr = arg
                    .description
                    .as_ref()
                    .map(description_to_doc_attr)
                    .or_else(|| arg.summary.as_ref().map(|s| to_doc_attr(s)));

                Some(quote! {
                    #doc_attr
                    #field_name: #field_type
                })
            });

                    quote! {
                        #msg_name {
                            #(#fields,)*
                        }
                    }
                };

            quote! {
                #doc_attr
                #msg_variant_decl
            }
        })
        .collect::<Vec<_>>();

    let opcodes = messages.iter().enumerate().map(|(opcode, msg)| {
        let msg_name = Ident::new(&snake_to_camel(&msg.name), Span::call_site());
        let opcode = opcode as u16;
        if msg.args.is_empty() {
            quote! {
                #name::#msg_name => #opcode
            }
        } else {
            quote! {
                #name::#msg_name { .. } => #opcode
            }
        }
    });

    // Placeholder to allow generic argument to be added later, without ABI
    // break.
    // TODO Use never type.
    let (generic, phantom_variant, phantom_case) = if !receiver {
        (
            quote! { 'a },
            quote! { #[doc(hidden)] __phantom_lifetime { phantom: std::marker::PhantomData<&'a ()>, never: std::convert::Infallible } },
            quote! { #name::__phantom_lifetime { never, .. } => match never {} },
        )
    } else {
        (quote! {}, quote! {}, quote! {})
    };

    quote! {
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum #name<#generic> {
            #(#variants,)*
            #phantom_variant
        }

        impl<#generic> #name<#generic> {
            #[doc="Get the opcode number of this message"]
            pub fn opcode(&self) -> u16 {
                match *self {
                    #(#opcodes,)*
                    #phantom_case
                }
            }
        }
    }
}
