use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};

use crate::{protocol::*, util::*, Side};

pub(crate) fn gen_parse_body(interface: &Interface, side: Side) -> TokenStream {
    let msgs = match side {
        Side::Client => &interface.events,
        Side::Server => &interface.requests,
    };
    let object_type = Ident::new(
        match side {
            Side::Client => "Proxy",
            Side::Server => "Resource",
        },
        Span::call_site(),
    );
    let msg_type = Ident::new(
        match side {
            Side::Client => "Event",
            Side::Server => "Request",
        },
        Span::call_site(),
    );

    let match_arms = msgs.iter().enumerate().map(|(opcode, msg)| {
        let opcode = opcode as u16;
        let msg_name = Ident::new(&snake_to_camel(&msg.name), Span::call_site());
        let args_pat = msg.args.iter().map(|arg| {
            let arg_name = Ident::new(
                &format!("{}{}", if is_keyword(&arg.name) { "_" } else { "" }, arg.name),
                Span::call_site(),
            );
            match arg.typ {
                Type::Uint => quote!{ Some(Argument::Uint(#arg_name)) },
                Type::Int => quote!{ Some(Argument::Int(#arg_name)) },
                Type::String => quote!{ Some(Argument::Str(#arg_name)) },
                Type::Fixed => quote!{ Some(Argument::Fixed(#arg_name)) },
                Type::Array => quote!{ Some(Argument::Array(#arg_name)) },
                Type::Object => quote!{ Some(Argument::Object(#arg_name)) },
                Type::NewId => quote!{ Some(Argument::NewId(#arg_name)) },
                Type::Fd => quote!{ Some(Argument::Fd(#arg_name)) },
                Type::Destructor => panic!("Argument {}.{}.{} has type destructor ?!", interface.name, msg.name, arg.name),
            }
        });

        let args_iter = msg.args.iter().map(|_| quote!{ arg_iter.next() });

        let arg_names = msg.args.iter().map(|arg| {
            let arg_name = format_ident!("{}{}", if is_keyword(&arg.name) { "_" } else { "" }, arg.name);
            if arg.enum_.is_some() {
                quote! { #arg_name: From::from(#arg_name as u32) }
            } else {
                match arg.typ {
                    Type::Uint | Type::Int | Type::Fd => quote!{ #arg_name },
                    Type::Fixed => quote!{ #arg_name: (#arg_name as f64) / 256.},
                    Type::String => {
                        if arg.allow_null {
                            quote! {
                                #arg_name: #arg_name.as_ref().map(|s| String::from_utf8_lossy(s.as_bytes()).into_owned())
                            }
                        } else {
                            quote! {
                                #arg_name: String::from_utf8_lossy(#arg_name.as_ref().unwrap().as_bytes()).into_owned()
                            }
                        }
                    },
                    Type::Object => {
                        let create_proxy = if let Some(ref created_interface) = arg.interface {
                            let created_iface_mod = Ident::new(created_interface, Span::call_site());
                            let created_iface_type = Ident::new(&snake_to_camel(created_interface), Span::call_site());
                            quote! {
                                match <super::#created_iface_mod::#created_iface_type as #object_type>::from_id(conn, #arg_name.clone()) {
                                    Ok(p) => p,
                                    Err(_) => return Err(DispatchError::BadMessage {
                                        sender_id: msg.sender_id,
                                        interface: Self::interface().name,
                                        opcode: msg.opcode
                                    }),
                                }
                            }
                        } else {
                            quote! { #arg_name.clone() }
                        };
                        if arg.allow_null {
                            quote! {
                                #arg_name: if #arg_name.is_null() { None } else { Some(#create_proxy) }
                            }
                        } else {
                            quote! {
                                #arg_name: #create_proxy
                            }
                        }
                    },
                    Type::NewId => {
                        let create_proxy = if let Some(ref created_interface) = arg.interface {
                            let created_iface_mod = Ident::new(created_interface, Span::call_site());
                            let created_iface_type = Ident::new(&snake_to_camel(created_interface), Span::call_site());
                            quote! {
                                match <super::#created_iface_mod::#created_iface_type as #object_type>::from_id(conn, #arg_name.clone()) {
                                    Ok(p) => p,
                                    Err(_) => return Err(DispatchError::BadMessage {
                                        sender_id: msg.sender_id,
                                        interface: Self::interface().name,
                                        opcode: msg.opcode,
                                    }),
                                }
                            }
                        } else if side == Side::Server {
                            quote! { New::wrap(#arg_name.clone()) }
                        } else {
                            quote! { #arg_name.clone() }
                        };
                        if arg.allow_null {
                            if side == Side::Server {
                                quote! {
                                    #arg_name: if #arg_name.is_null() { None } else { Some(New::wrap(#create_proxy)) }
                                }
                            } else {
                                quote! {
                                    #arg_name: if #arg_name.is_null() { None } else { Some(#create_proxy) }
                                }
                            }
                        } else if side == Side::Server {
                            quote! {
                                #arg_name: New::wrap(#create_proxy)
                            }
                        } else  {
                            quote! {
                                #arg_name: #create_proxy
                            }
                        }
                    },
                    Type::Array => {
                        if arg.allow_null {
                            quote! { if #arg_name.len() == 0 { None } else { Some(*#arg_name) } }
                        } else {
                            quote! { #arg_name: *#arg_name }
                        }
                    },
                    Type::Destructor => unreachable!(),
                }
            }
        });

        quote! {
            #opcode => {
                if let (#(#args_pat),*) = (#(#args_iter),*) {
                    Ok((me, #msg_type::#msg_name { #(#arg_names),* }))
                } else {
                    Err(DispatchError::BadMessage { sender_id: msg.sender_id, interface: Self::interface().name, opcode: msg.opcode })
                }
            }
        }
    });

    quote! {
        let me = Self::from_id(conn, msg.sender_id.clone()).unwrap();
        let mut arg_iter = msg.args.into_iter();
        match msg.opcode {
            #(#match_arms),*
            _ => Err(DispatchError::BadMessage { sender_id: msg.sender_id, interface: Self::interface().name, opcode: msg.opcode }),
        }
    }
}

pub(crate) fn gen_write_body(interface: &Interface, side: Side) -> TokenStream {
    let msgs = match side {
        Side::Client => &interface.requests,
        Side::Server => &interface.events,
    };
    let msg_type = Ident::new(
        match side {
            Side::Client => "Request",
            Side::Server => "Event",
        },
        Span::call_site(),
    );
    let arms = msgs.iter().enumerate().map(|(opcode, msg)| {
        let msg_name = Ident::new(&snake_to_camel(&msg.name), Span::call_site());
        let opcode = opcode as u16;
        let arg_names = msg.args.iter().flat_map(|arg| {
            if arg.typ == Type::NewId && arg.interface.is_some() && side == Side::Client {
                None
            } else {
                Some(format_ident!("{}{}", if is_keyword(&arg.name) { "_" } else { "" }, arg.name))
            }
        });
        let mut child_spec = None;
        let args = msg.args.iter().flat_map(|arg| {
            let arg_name = format_ident!("{}{}", if is_keyword(&arg.name) { "_" } else { "" }, arg.name);

            match arg.typ {
                Type::Int => vec![if arg.enum_.is_some() { quote!{ Argument::Int(Into::<u32>::into(#arg_name) as i32) } } else { quote!{ Argument::Int(#arg_name) } }],
                Type::Uint => vec![if arg.enum_.is_some() { quote!{ Argument::Uint(#arg_name.into()) } } else { quote!{ Argument::Uint(#arg_name) } }],
                Type::Fd => vec![quote!{ Argument::Fd(#arg_name) }],
                Type::Fixed => vec![quote! { Argument::Fixed((#arg_name * 256.) as i32) }],
                Type::Object => if arg.allow_null {
                    if side == Side::Server {
                        vec![quote! { if let Some(obj) = #arg_name { Argument::Object(Resource::id(&obj)) } else { Argument::Object(ObjectId::null()) } }]
                    } else {
                        vec![quote! { if let Some(obj) = #arg_name { Argument::Object(Proxy::id(&obj)) } else { Argument::Object(ObjectId::null()) } }]
                    }
                } else if side == Side::Server {
                    vec![quote!{ Argument::Object(Resource::id(&#arg_name)) }]
                } else {
                    vec![quote!{ Argument::Object(Proxy::id(&#arg_name)) }]
                },
                Type::Array => if arg.allow_null {
                    vec![quote! { if let Some(array) = #arg_name { Argument::Array(Box::new(array)) } else { Argument::Array(Box::new(Vec::new()))}}]
                } else {
                    vec![quote! { Argument::Array(Box::new(#arg_name)) }]
                },
                Type::String => if arg.allow_null {
                    vec![quote! { Argument::Str(#arg_name.map(|s| Box::new(std::ffi::CString::new(s).unwrap()))) }]
                } else {
                    vec![quote! { Argument::Str(Some(Box::new(std::ffi::CString::new(#arg_name).unwrap()))) }]
                },
                Type::NewId => if side == Side::Client {
                    if let Some(ref created_interface) = arg.interface {
                        let created_iface_mod = Ident::new(created_interface, Span::call_site());
                        let created_iface_type = Ident::new(&snake_to_camel(created_interface), Span::call_site());
                        assert!(child_spec.is_none());
                        child_spec = Some(quote! { {
                            let my_info = conn.object_info(self.id())?;
                            Some((super::#created_iface_mod::#created_iface_type::interface(), my_info.version))
                        } });
                        vec![quote! { Argument::NewId(ObjectId::null()) }]
                    } else {
                        assert!(child_spec.is_none());
                        child_spec = Some(quote! {
                            Some((#arg_name.0, #arg_name.1))
                        });
                        vec![
                            quote! {
                                Argument::Str(Some(Box::new(std::ffi::CString::new(#arg_name.0.name).unwrap())))
                            },
                            quote! {
                                Argument::Uint(#arg_name.1)
                            },
                            quote! {
                                Argument::NewId(ObjectId::null())
                            },
                        ]
                    }
                } else {
                    // server-side NewId is the same as Object
                    if arg.allow_null {
                        vec![quote! { if let Some(obj) = #arg_name { Argument::NewId(Resource::id(&obj)) } else { Argument::NewId(ObjectId::null()) } }]
                    } else {
                        vec![quote!{ Argument::NewId(Resource::id(&#arg_name)) }]
                    }
                },
                Type::Destructor => panic!("Argument {}.{}.{} has type destructor ?!", interface.name, msg.name, arg.name),
            }
        });
        let args = if msg.args.is_empty() {
            quote! {
                smallvec::SmallVec::new()
            }
        } else if msg.args.len() <= 4 {
            // Note: Keep in sync with `wayland_backend::protocol::INLINE_ARGS`.
            // Fits in SmallVec inline capacity
            quote! { {
                let mut vec = smallvec::SmallVec::new();
                #(
                    vec.push(#args);
                )*
                vec
            } }
        } else {
            quote! {
                smallvec::SmallVec::from_vec(vec![#(#args),*])
            }
        };
        if side == Side::Client {
            let child_spec = child_spec.unwrap_or_else(|| quote! { None });
            quote! {
                #msg_type::#msg_name { #(#arg_names),* } => {
                    let child_spec = #child_spec;
                    let args = #args;
                    Ok((Message {
                        sender_id: self.id.clone(),
                        opcode: #opcode,
                        args
                    }, child_spec))
                }
            }
        } else {
            quote! {
                #msg_type::#msg_name { #(#arg_names),* } => Ok(Message {
                    sender_id: self.id.clone(),
                    opcode: #opcode,
                    args: #args,
                })
            }
        }
    });
    quote! {
        match msg {
            #(#arms,)*
            #msg_type::__phantom_lifetime { never, .. } => match never {}
        }
    }
}
