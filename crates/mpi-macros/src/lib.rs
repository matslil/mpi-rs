//! Procedural macros for `mpi` task and protocol declarations.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Attribute, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, LitStr, Pat, PatType, Path,
    ReturnType, Token, Type, Visibility, braced, parenthesized, parse_macro_input,
};

mod kw {
    syn::custom_keyword!(protocol);
    syn::custom_keyword!(event);
    syn::custom_keyword!(call);
    syn::custom_keyword!(stream);
}

struct TaskArgs {
    queue_size: TokenStream2,
    priority_reserved: TokenStream2,
    receives: Vec<Type>,
}

impl Parse for TaskArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut queue_size = None;
        let mut priority_reserved = None;
        let mut receives = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            if key == "queue_size" {
                input.parse::<Token![=]>()?;
                let value: syn::Expr = input.parse()?;
                queue_size = Some(value.into_token_stream());
            } else if key == "priority_reserved" {
                input.parse::<Token![=]>()?;
                let value: syn::Expr = input.parse()?;
                priority_reserved = Some(value.into_token_stream());
            } else if key == "receives" {
                let content;
                parenthesized!(content in input);
                receives = Punctuated::<Type, Token![,]>::parse_terminated(&content)?
                    .into_iter()
                    .collect();
            } else {
                return Err(syn::Error::new_spanned(
                    key,
                    "expected `queue_size`, `priority_reserved`, or `receives`",
                ));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let queue_size = queue_size.ok_or_else(|| input.error("missing `queue_size`"))?;
        let priority_reserved = priority_reserved.unwrap_or_else(|| quote!(1usize));
        Ok(Self {
            queue_size,
            priority_reserved,
            receives,
        })
    }
}

struct EventArgs {
    priority: bool,
    protocol: Option<Path>,
}

impl Parse for EventArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut priority = false;
        let mut protocol = None;

        while !input.is_empty() {
            if input.peek(Ident) {
                let key: Ident = input.parse()?;
                if key == "priority" {
                    priority = true;
                } else if key == "protocol" {
                    input.parse::<Token![=]>()?;
                    protocol = Some(input.parse()?);
                } else {
                    return Err(syn::Error::new_spanned(
                        key,
                        "expected `priority` or `protocol`",
                    ));
                }
            } else {
                return Err(input.error("expected `priority` or `protocol`"));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { priority, protocol })
    }
}

struct CallArgs {
    late_reply_policy: TokenStream2,
    protocol: Option<Path>,
}

impl Parse for CallArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut late_reply_policy = None;
        let mut protocol = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "late_reply" {
                late_reply_policy = Some(parse_late_reply_policy(input.parse()?)?);
            } else if key == "protocol" {
                protocol = Some(input.parse()?);
            } else {
                return Err(syn::Error::new_spanned(
                    key,
                    "expected `late_reply` or `protocol`",
                ));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let late_reply_policy =
            late_reply_policy.unwrap_or_else(|| quote! { ::mpi::LateReplyPolicy::Report });
        Ok(Self {
            late_reply_policy,
            protocol,
        })
    }
}

struct StreamArgs {
    item: Type,
    error: Type,
    batch_size: TokenStream2,
    late_reply_policy: TokenStream2,
    protocol: Option<Path>,
}

impl Parse for StreamArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut item = None;
        let mut error = None;
        let mut batch_size = None;
        let mut late_reply_policy = None;
        let mut protocol = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "item" {
                item = Some(input.parse()?);
            } else if key == "error" {
                error = Some(input.parse()?);
            } else if key == "batch_size" {
                let value: syn::Expr = input.parse()?;
                batch_size = Some(value.into_token_stream());
            } else if key == "late_reply" {
                late_reply_policy = Some(parse_late_reply_policy(input.parse()?)?);
            } else if key == "protocol" {
                protocol = Some(input.parse()?);
            } else {
                return Err(syn::Error::new_spanned(
                    key,
                    "expected `item`, `error`, `batch_size`, `late_reply`, or `protocol`",
                ));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let item = item.ok_or_else(|| input.error("missing `item` stream attribute"))?;
        let error = error.ok_or_else(|| input.error("missing `error` stream attribute"))?;
        let batch_size = batch_size.unwrap_or_else(|| quote! { 64usize });
        let late_reply_policy =
            late_reply_policy.unwrap_or_else(|| quote! { ::mpi::LateReplyPolicy::Report });

        Ok(Self {
            item,
            error,
            batch_size,
            late_reply_policy,
            protocol,
        })
    }
}

enum ProtocolItemKind {
    Event {
        payload: Box<Type>,
    },
    Call {
        request: Box<Type>,
        reply: Box<Type>,
    },
    Stream {
        request: Box<Type>,
        item: Box<Type>,
        error: Box<Type>,
    },
}

struct ProtocolItemParsed {
    kind: ProtocolItemKind,
    name: Ident,
}

struct ProtocolDecl {
    vis: Visibility,
    name: Ident,
    items: Vec<ProtocolItemParsed>,
}

impl Parse for ProtocolDecl {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let vis: Visibility = input.parse()?;
        input.parse::<kw::protocol>()?;
        let name: Ident = input.parse()?;
        let content;
        braced!(content in input);
        let mut items = Vec::new();

        while !content.is_empty() {
            let lookahead = content.lookahead1();
            let (name, kind) = if lookahead.peek(kw::event) {
                content.parse::<kw::event>()?;
                let name: Ident = content.parse()?;
                let args;
                parenthesized!(args in content);
                let payload = Box::new(args.parse()?);
                (name, ProtocolItemKind::Event { payload })
            } else if lookahead.peek(kw::call) {
                content.parse::<kw::call>()?;
                let name: Ident = content.parse()?;
                let args;
                parenthesized!(args in content);
                let request = Box::new(args.parse()?);
                content.parse::<Token![->]>()?;
                let reply = Box::new(content.parse()?);
                (name, ProtocolItemKind::Call { request, reply })
            } else if lookahead.peek(kw::stream) {
                content.parse::<kw::stream>()?;
                let name: Ident = content.parse()?;
                let args;
                parenthesized!(args in content);
                let request = Box::new(args.parse()?);
                content.parse::<Token![->]>()?;
                let item = Box::new(content.parse()?);
                content.parse::<Token![,]>()?;
                let error = Box::new(content.parse()?);
                (
                    name,
                    ProtocolItemKind::Stream {
                        request,
                        item,
                        error,
                    },
                )
            } else {
                return Err(lookahead.error());
            };

            content.parse::<Token![;]>()?;
            items.push(ProtocolItemParsed { kind, name });
        }

        Ok(Self { vis, name, items })
    }
}

fn parse_late_reply_policy(value: LitStr) -> syn::Result<TokenStream2> {
    match value.value().as_str() {
        "report" => Ok(quote! { ::mpi::LateReplyPolicy::Report }),
        "ignore" => Ok(quote! { ::mpi::LateReplyPolicy::Ignore }),
        _ => Err(syn::Error::new_spanned(
            value,
            "expected late_reply = \"report\" or late_reply = \"ignore\"",
        )),
    }
}

#[derive(Clone)]
struct HandlerArg {
    ident: Ident,
    ty: Type,
}

enum HandlerKind {
    Start,
    Event {
        priority: bool,
        protocol: Option<Path>,
    },
    Call {
        reply: Option<Box<Type>>,
        late_reply_policy: TokenStream2,
        protocol: Option<Path>,
    },
    Stream {
        item: Box<Type>,
        error: Box<Type>,
        batch_size: TokenStream2,
        late_reply_policy: TokenStream2,
        protocol: Option<Path>,
    },
    LateReply,
}

struct Handler {
    kind: HandlerKind,
    method: ImplItemFn,
    args: Vec<HandlerArg>,
}

fn compile_error(error: syn::Error) -> TokenStream {
    error.to_compile_error().into()
}

fn special_attr_name(attr: &Attribute) -> Option<&'static str> {
    if attr.path().is_ident("start") {
        Some("start")
    } else if attr.path().is_ident("event") {
        Some("event")
    } else if attr.path().is_ident("call") {
        Some("call")
    } else if attr.path().is_ident("stream") {
        Some("stream")
    } else if attr.path().is_ident("late_reply") {
        Some("late_reply")
    } else {
        None
    }
}

fn handler_kind(attrs: &[Attribute]) -> syn::Result<Option<HandlerKind>> {
    let mut result = None;

    for attr in attrs {
        let Some(name) = special_attr_name(attr) else {
            continue;
        };

        if result.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "handler may only have one mpi handler attribute",
            ));
        }

        result = Some(match name {
            "start" => HandlerKind::Start,
            "event" => {
                let args = if matches!(attr.meta, syn::Meta::Path(_)) {
                    EventArgs {
                        priority: false,
                        protocol: None,
                    }
                } else {
                    attr.parse_args::<EventArgs>()?
                };
                HandlerKind::Event {
                    priority: args.priority,
                    protocol: args.protocol,
                }
            }
            "call" => {
                let args = if matches!(attr.meta, syn::Meta::Path(_)) {
                    CallArgs {
                        late_reply_policy: quote! { ::mpi::LateReplyPolicy::Report },
                        protocol: None,
                    }
                } else {
                    attr.parse_args::<CallArgs>()?
                };
                HandlerKind::Call {
                    reply: None,
                    late_reply_policy: args.late_reply_policy,
                    protocol: args.protocol,
                }
            }
            "stream" => {
                let args = attr.parse_args::<StreamArgs>()?;
                HandlerKind::Stream {
                    item: Box::new(args.item),
                    error: Box::new(args.error),
                    batch_size: args.batch_size,
                    late_reply_policy: args.late_reply_policy,
                    protocol: args.protocol,
                }
            }
            "late_reply" => HandlerKind::LateReply,
            _ => unreachable!(),
        });
    }

    Ok(result)
}

fn strip_handler_attrs(method: &mut ImplItemFn) {
    method
        .attrs
        .retain(|attr| special_attr_name(attr).is_none());
}

fn normalize_handler_method(method: &mut ImplItemFn, kind: &HandlerKind) {
    strip_handler_attrs(method);

    if !matches!(kind, HandlerKind::LateReply) {
        method.sig.asyncness = Some(Default::default());
    }
}

fn infer_call_reply(method: &ImplItemFn) -> syn::Result<Type> {
    match &method.sig.output {
        ReturnType::Type(_, ty) => Ok((**ty).clone()),
        ReturnType::Default => Err(syn::Error::new_spanned(
            &method.sig,
            "call handler must return a reply payload type",
        )),
    }
}

fn resolve_handler_kind(method: &ImplItemFn, kind: HandlerKind) -> syn::Result<HandlerKind> {
    match kind {
        HandlerKind::Call {
            reply,
            late_reply_policy,
            protocol,
        } => {
            let reply = match (reply, &protocol) {
                (Some(reply), _) => reply,
                (None, Some(protocol)) => {
                    let reply: Type = syn::parse2(quote! { #protocol::ReplyPayload })?;
                    Box::new(reply)
                }
                (None, None) => Box::new(infer_call_reply(method)?),
            };
            Ok(HandlerKind::Call {
                reply: Some(reply),
                late_reply_policy,
                protocol,
            })
        }
        other => Ok(other),
    }
}

fn payload_args(method: &ImplItemFn, skip_stream_sink: bool) -> syn::Result<Vec<HandlerArg>> {
    let mut inputs = method.sig.inputs.iter();

    match inputs.next() {
        Some(FnArg::Receiver(receiver)) => {
            return Err(syn::Error::new_spanned(
                receiver,
                "handler must not take `self`; access task state through `ctx.with_state(|state| ...)`",
            ));
        }
        Some(FnArg::Typed(_)) => {}
        None => {
            return Err(syn::Error::new_spanned(
                &method.sig,
                "handler must take a context parameter",
            ));
        }
    }

    if skip_stream_sink {
        match inputs.next() {
            Some(FnArg::Typed(PatType { pat, .. })) => {
                if !matches!(&**pat, Pat::Ident(_)) {
                    return Err(syn::Error::new_spanned(
                        pat,
                        "stream sink parameter must use a simple identifier",
                    ));
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    &method.sig,
                    "stream handler must take a stream sink parameter after the context",
                ));
            }
        }
    }

    inputs
        .map(|input| match input {
            FnArg::Typed(PatType { pat, ty, .. }) => match &**pat {
                Pat::Ident(ident) => Ok(HandlerArg {
                    ident: ident.ident.clone(),
                    ty: (**ty).clone(),
                }),
                _ => Err(syn::Error::new_spanned(
                    pat,
                    "message payload parameters must use simple identifiers",
                )),
            },
            FnArg::Receiver(receiver) => Err(syn::Error::new_spanned(
                receiver,
                "unexpected receiver in payload parameter list",
            )),
        })
        .collect()
}

fn self_type_ident(item: &ItemImpl) -> syn::Result<Ident> {
    match &*item.self_ty {
        Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            Ok(path.path.segments[0].ident.clone())
        }
        other => Err(syn::Error::new_spanned(
            other,
            "#[task] currently supports simple concrete task types",
        )),
    }
}

fn to_variant_ident(method: &Ident) -> Ident {
    let name = method.to_string();
    let mut out = String::new();
    let mut upper_next = true;
    for ch in name.chars() {
        if ch == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    format_ident!("{}", out)
}

fn to_snake_ident(ident: &Ident) -> Ident {
    let name = ident.to_string();
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else {
            out.push(ch);
        }
    }
    format_ident!("{}", out)
}

fn protocol_item_method(protocol: &Path) -> syn::Result<Ident> {
    protocol
        .segments
        .last()
        .map(|segment| to_snake_ident(&segment.ident))
        .ok_or_else(|| syn::Error::new_spanned(protocol, "protocol path must name a message"))
}

/// Generates protocol message identity modules and protocol-derived bindings.
#[proc_macro]
pub fn protocol(input: TokenStream) -> TokenStream {
    let decl = parse_macro_input!(input as ProtocolDecl);
    let vis = decl.vis;
    let protocol_ident = decl.name;
    let mut item_modules = Vec::new();
    let mut binding_methods = Vec::new();

    for item in decl.items {
        let item_ident = item.name;
        let method_ident = to_snake_ident(&item_ident);
        let blocking_method_ident = format_ident!("{}_blocking", method_ident);
        let legacy_item_alias = if item_ident == method_ident {
            quote! {}
        } else {
            quote! {
                #[allow(non_snake_case)]
                pub use #method_ident as #item_ident;
            }
        };

        match item.kind {
            ProtocolItemKind::Event { payload } => {
                item_modules.push(quote! {
                    pub mod #method_ident {
                        use super::super::*;

                        pub type Payload = #payload;

                        pub trait Target: Clone {
                            fn #method_ident(
                                &self,
                                ctx: &mut impl ::mpi::TaskScope,
                                payload: Payload,
                            ) -> Result<(), ::mpi::SendError>;

                            fn #blocking_method_ident(
                                &self,
                                payload: Payload,
                            ) -> Result<(), ::mpi::SendError>;
                        }
                    }

                    #legacy_item_alias
                });
                binding_methods.push(quote! {
                    pub fn #method_ident(
                        &self,
                        ctx: &mut impl ::mpi::TaskScope,
                        payload: #method_ident::Payload,
                    ) -> Result<(), ::mpi::SendError>
                    where
                        H: #method_ident::Target,
                    {
                        self.handle.#method_ident(ctx, payload)
                    }

                    pub fn #blocking_method_ident(
                        &self,
                        payload: #method_ident::Payload,
                    ) -> Result<(), ::mpi::SendError>
                    where
                        H: #method_ident::Target,
                    {
                        self.handle.#blocking_method_ident(payload)
                    }
                });
            }
            ProtocolItemKind::Call { request, reply } => {
                item_modules.push(quote! {
                    pub mod #method_ident {
                        use super::super::*;

                        pub type Request = #request;
                        pub type ReplyPayload = #reply;

                        pub struct Reply(pub ::mpi::Response<ReplyPayload>);

                        impl ::mpi::ProtocolReceive for Reply {
                            fn into_task_message<M>(self) -> M
                            where
                                M: ::mpi::TaskMessage + ::mpi::CallResponseMessage + ::mpi::StreamEventMessage,
                            {
                                let response = self.0;
                                M::call_response(
                                    response.session_id,
                                    Box::new(response.value) as Box<dyn ::std::any::Any + Send>,
                                )
                            }
                        }

                        pub trait Target: Clone {
                            fn #method_ident<C>(
                                &self,
                                ctx: &mut C,
                                request: Request,
                            ) -> ::mpi::SuspendedCall<ReplyPayload>
                            where
                                C: ::mpi::TaskScope,
                                C::Message: ::mpi::CanReceive<Reply>;

                            fn #blocking_method_ident(
                                &self,
                                request: Request,
                            ) -> Result<ReplyPayload, ::mpi::CallError>;
                        }
                    }

                    #legacy_item_alias
                });
                binding_methods.push(quote! {
                    pub fn #method_ident<C>(
                        &self,
                        ctx: &mut C,
                        request: #method_ident::Request,
                    ) -> ::mpi::SuspendedCall<#method_ident::ReplyPayload>
                    where
                        H: #method_ident::Target,
                        C: ::mpi::TaskScope,
                        C::Message: ::mpi::CanReceive<#method_ident::Reply>,
                    {
                        self.handle.#method_ident(ctx, request)
                    }

                    pub fn #blocking_method_ident(
                        &self,
                        request: #method_ident::Request,
                    ) -> Result<#method_ident::ReplyPayload, ::mpi::CallError>
                    where
                        H: #method_ident::Target,
                    {
                        self.handle.#blocking_method_ident(request)
                    }
                });
            }
            ProtocolItemKind::Stream {
                request,
                item,
                error,
            } => {
                item_modules.push(quote! {
                    pub mod #method_ident {
                        use super::super::*;

                        pub type Request = #request;
                        pub type ItemPayload = #item;
                        pub type ErrorPayload = #error;

                        pub struct Item(pub ::mpi::StreamEvent<ItemPayload, ErrorPayload>);
                        pub struct Finish(pub ::mpi::StreamEvent<ItemPayload, ErrorPayload>);
                        pub struct Error(pub ::mpi::StreamEvent<ItemPayload, ErrorPayload>);
                        pub type Event = Item;

                        impl ::mpi::ProtocolReceive for Item {
                            fn into_task_message<M>(self) -> M
                            where
                                M: ::mpi::TaskMessage + ::mpi::CallResponseMessage + ::mpi::StreamEventMessage,
                            {
                                let event = self.0;
                                let session_id = ::mpi::HasSessionId::session_id(&event);
                                M::stream_event(
                                    session_id,
                                    Box::new(event) as Box<dyn ::std::any::Any + Send>,
                                )
                            }
                        }

                        impl ::mpi::ProtocolReceive for Finish {
                            fn into_task_message<M>(self) -> M
                            where
                                M: ::mpi::TaskMessage + ::mpi::CallResponseMessage + ::mpi::StreamEventMessage,
                            {
                                let event = self.0;
                                let session_id = ::mpi::HasSessionId::session_id(&event);
                                M::stream_event(
                                    session_id,
                                    Box::new(event) as Box<dyn ::std::any::Any + Send>,
                                )
                            }
                        }

                        impl ::mpi::ProtocolReceive for Error {
                            fn into_task_message<M>(self) -> M
                            where
                                M: ::mpi::TaskMessage + ::mpi::CallResponseMessage + ::mpi::StreamEventMessage,
                            {
                                let event = self.0;
                                let session_id = ::mpi::HasSessionId::session_id(&event);
                                M::stream_event(
                                    session_id,
                                    Box::new(event) as Box<dyn ::std::any::Any + Send>,
                                )
                            }
                        }

                        pub trait Target: Clone {
                            fn #method_ident<C>(
                                &self,
                                ctx: &mut C,
                                request: Request,
                            ) -> Result<::mpi::SuspendedMessageStream<ItemPayload, ErrorPayload>, ::mpi::SendError>
                            where
                                C: ::mpi::TaskScope,
                                C::Message: ::mpi::CanReceive<Item>;

                            fn #blocking_method_ident(
                                &self,
                                request: Request,
                            ) -> Result<::mpi::BlockingMessageStream<ItemPayload, ErrorPayload>, ::mpi::SendError>;
                        }
                    }

                    #legacy_item_alias
                });
                binding_methods.push(quote! {
                    pub fn #method_ident<C>(
                        &self,
                        ctx: &mut C,
                        request: #method_ident::Request,
                    ) -> Result<::mpi::SuspendedMessageStream<#method_ident::ItemPayload, #method_ident::ErrorPayload>, ::mpi::SendError>
                    where
                        H: #method_ident::Target,
                        C: ::mpi::TaskScope,
                        C::Message: ::mpi::CanReceive<#method_ident::Item>,
                    {
                        self.handle.#method_ident(ctx, request)
                    }

                    pub fn #blocking_method_ident(
                        &self,
                        request: #method_ident::Request,
                    ) -> Result<::mpi::BlockingMessageStream<#method_ident::ItemPayload, #method_ident::ErrorPayload>, ::mpi::SendError>
                    where
                        H: #method_ident::Target,
                    {
                        self.handle.#blocking_method_ident(request)
                    }
                });
            }
        }
    }

    quote! {
        #[allow(non_snake_case)]
        #vis mod #protocol_ident {
            use super::*;

            #[derive(Clone)]
            pub struct Binding<H> {
                handle: H,
            }

            pub fn bind<H>(handle: H) -> Binding<H> {
                Binding { handle }
            }

            impl<H> Binding<H> {
                pub fn handle(&self) -> &H {
                    &self.handle
                }

                #(#binding_methods)*
            }

            #(#item_modules)*
        }
    }
    .into()
}

/// Generates task message enum, context, handle, send methods, spawn helper,
/// placement implementation, and dispatch for one task impl block.
#[rustfmt::skip]
#[proc_macro_attribute]
pub fn task(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as TaskArgs);
    let mut item_impl = parse_macro_input!(item as ItemImpl);

    let task_ident = match self_type_ident(&item_impl) {
        Ok(ident) => ident,
        Err(error) => return compile_error(error),
    };

    let message_ident = format_ident!("{}Message", task_ident);
    let handle_ident = format_ident!("{}Handle", task_ident);
    let context_ident = format_ident!("{}Context", task_ident);
    let stream_control_ident = format_ident!("{}StreamControl", task_ident);
    let queue_size = args.queue_size;
    let priority_reserved = args.priority_reserved;
    let receives = args.receives;

    let mut handlers = Vec::new();
    let mut stripped_items = Vec::new();

    for item in item_impl.items.into_iter() {
        match item {
            ImplItem::Fn(mut method) => {
                let kind = match handler_kind(&method.attrs) {
                    Ok(kind) => kind,
                    Err(error) => return compile_error(error),
                };

                if let Some(kind) = kind {
                    let kind = match resolve_handler_kind(&method, kind) {
                        Ok(kind) => kind,
                        Err(error) => return compile_error(error),
                    };
                    let skip_stream_sink = matches!(kind, HandlerKind::Stream { .. });
                    let args = match payload_args(&method, skip_stream_sink) {
                        Ok(args) => args,
                        Err(error) => return compile_error(error),
                    };
                    if matches!(kind, HandlerKind::LateReply) && args.len() != 1 {
                        return compile_error(syn::Error::new_spanned(
                            &method.sig,
                            "late reply callback must take exactly one late reply argument after the context",
                        ));
                    }
                    normalize_handler_method(&mut method, &kind);
                    handlers.push(Handler {
                        kind,
                        method,
                        args,
                    });
                } else {
                    stripped_items.push(ImplItem::Fn(method));
                }
            }
            other => stripped_items.push(other),
        }
    }

    let start_count = handlers
        .iter()
        .filter(|handler| matches!(handler.kind, HandlerKind::Start))
        .count();
    if start_count > 1 {
        return compile_error(syn::Error::new_spanned(
            &task_ident,
            "#[task] supports at most one #[start] handler",
        ));
    }
    let late_reply_count = handlers
        .iter()
        .filter(|handler| matches!(handler.kind, HandlerKind::LateReply))
        .count();
    if late_reply_count > 1 {
        return compile_error(syn::Error::new_spanned(
            &task_ident,
            "#[task] supports at most one #[late_reply] callback",
        ));
    }

    let mut original_items = stripped_items;
    original_items.extend(
        handlers
            .iter()
            .map(|handler| ImplItem::Fn(handler.method.clone())),
    );
    item_impl.items = original_items;

    let mut variants = Vec::new();
    let mut placements = Vec::new();
    let mut dispatch_arms = Vec::new();
    let mut handle_methods = Vec::new();
    let mut protocol_impls = Vec::new();
    let mut start_args = Vec::new();
    let mut start_variant = None;
    let late_reply_callback = handlers
        .iter()
        .find(|handler| matches!(handler.kind, HandlerKind::LateReply))
        .map(|handler| handler.method.sig.ident.clone());
    let deliver_call_response = if let Some(method_ident) = &late_reply_callback {
        quote! {
            let __ctx_inner = ctx.inner.clone();
            let _ = __ctx_inner.deliver_call_response_with_late_reply_callback(
                ::mpi::QueuedCallResponse::with_late_reply_policy(
                    session_id,
                    value,
                    late_reply_policy,
                ),
                |late_reply| { #task_ident::#method_ident(&mut ctx, late_reply) },
            );
        }
    } else {
        quote! {
            let _ = ctx.inner.deliver_call_response(
                ::mpi::QueuedCallResponse::with_late_reply_policy(
                    session_id,
                    value,
                    late_reply_policy,
                ),
            );
        }
    };
    let deliver_stream_event = if let Some(method_ident) = &late_reply_callback {
        quote! {
            let __ctx_inner = ctx.inner.clone();
            let _ = __ctx_inner.deliver_stream_event_with_late_reply_callback(
                ::mpi::QueuedStreamEvent::with_late_reply_policy(
                    session_id,
                    event,
                    late_reply_policy,
                ),
                |late_reply| { #task_ident::#method_ident(&mut ctx, late_reply) },
            );
        }
    } else {
        quote! {
            let _ = ctx.inner.deliver_stream_event(
                ::mpi::QueuedStreamEvent::with_late_reply_policy(
                    session_id,
                    event,
                    late_reply_policy,
                ),
            );
        }
    };

    variants.push(quote! { __StreamCancel { session_id: ::mpi::SessionId } });
    placements.push(quote! { Self::__StreamCancel { .. } => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! {
        #message_ident::__StreamCancel { session_id } => {
            ctx.inner.record_stream_cancel(::mpi::StreamCancel::new(session_id));
        }
    });

    variants.push(quote! { __QueueSpaceWakeup });
    placements.push(quote! { Self::__QueueSpaceWakeup => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! {
        #message_ident::__QueueSpaceWakeup => {}
    });

    variants.push(quote! {
        __CallResponse {
            session_id: ::mpi::SessionId,
            value: Box<dyn ::std::any::Any + Send>,
            late_reply_policy: ::mpi::LateReplyPolicy,
        }
    });
    placements.push(quote! { Self::__CallResponse { .. } => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! {
        #message_ident::__CallResponse { session_id, value, late_reply_policy } => {
            #deliver_call_response
        }
    });

    variants.push(quote! { __CallRelease { session_id: ::mpi::SessionId } });
    placements.push(quote! { Self::__CallRelease { .. } => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! {
        #message_ident::__CallRelease { session_id } => {
            ctx.inner.record_call_release(::mpi::QueuedCallRelease::new(session_id));
        }
    });

    variants.push(quote! { __StreamPull { session_id: ::mpi::SessionId, credit: u32 } });
    placements.push(quote! { Self::__StreamPull { .. } => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! {
        #message_ident::__StreamPull { session_id, credit } => {
            ctx.inner.record_stream_pull(::mpi::StreamPull::new(session_id, credit));
        }
    });

    variants.push(quote! {
        __StreamEvent {
            session_id: ::mpi::SessionId,
            event: Box<dyn ::std::any::Any + Send>,
            late_reply_policy: ::mpi::LateReplyPolicy,
        }
    });
    placements.push(quote! { Self::__StreamEvent { .. } => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! {
        #message_ident::__StreamEvent { session_id, event, late_reply_policy } => {
            #deliver_stream_event
        }
    });

    for handler in &handlers {
        let method_ident = &handler.method.sig.ident;
        let variant_ident = if matches!(handler.kind, HandlerKind::Start) {
            format_ident!("Start")
        } else {
            to_variant_ident(method_ident)
        };
        let arg_idents: Vec<_> = handler.args.iter().map(|arg| &arg.ident).collect();
        let arg_tys: Vec<_> = handler.args.iter().map(|arg| &arg.ty).collect();
        let handler_call = quote! {
            #task_ident::#method_ident(&mut ctx, #(#arg_idents),*)
        };

        match &handler.kind {
            HandlerKind::Start => {
                start_args = handler.args.clone();
                start_variant =
                    Some(quote! { #message_ident::#variant_ident { #(#arg_idents),* } });
                variants.push(quote! { #variant_ident { #(#arg_idents: #arg_tys),* } });
                placements.push(quote! {
                    Self::#variant_ident { .. } => ::mpi::MessagePlacement::Priority
                });
                dispatch_arms.push(quote! {
                    #message_ident::#variant_ident { #(#arg_idents),* } => {
                        let __ctx_inner = ctx.inner.clone();
                        ::mpi::block_on_handler(
                            #handler_call,
                            inner_handle.task_endpoint(),
                            &__ctx_inner,
                            &mut *deferred,
                        );
                    }
                });
            }
            HandlerKind::Event { priority, protocol } => {
                let placement = if *priority {
                    quote! { ::mpi::MessagePlacement::Priority }
                } else {
                    quote! { ::mpi::MessagePlacement::Normal }
                };
                let blocking_method = format_ident!("{}_blocking", method_ident);
                variants.push(quote! { #variant_ident { #(#arg_idents: #arg_tys),* } });
                placements.push(quote! {
                    Self::#variant_ident { .. } => #placement
                });
                dispatch_arms.push(quote! {
                    #message_ident::#variant_ident { #(#arg_idents),* } => {
                        let mut __mpi_handler_ctx = #context_ident {
                            inner: ctx.inner.clone(),
                            state: state.clone(),
                        };
                        let mut __mpi_handler_future = {
                            let ctx = &mut __mpi_handler_ctx;
                            #task_ident::#method_ident(ctx, #(#arg_idents),*)
                        };

                        ::mpi::block_on_ctx_task_with_dispatch(
                            ::mpi::from_std_future(__mpi_handler_future),
                            inner_handle.task_endpoint(),
                            &mut ctx.inner,
                            |__mpi_message, __mpi_inner| {
                                let ctx = #context_ident {
                                    inner: __mpi_inner.clone(),
                                    state: state.clone(),
                                };
                                let _ctx = __dispatch_message(
                                    state,
                                    inner_handle,
                                    ctx,
                                    deferred,
                                    __mpi_message,
                                );
                            },
                        );
                    }
                });
                handle_methods.push(quote! {
                    pub fn #method_ident(&self, _ctx: &mut impl ::mpi::TaskScope #(, #arg_idents: #arg_tys)*) -> Result<(), ::mpi::SendError> {
                        self.inner.send_message(#message_ident::#variant_ident { #(#arg_idents),* })
                    }

                    pub fn #blocking_method(&self, #(#arg_idents: #arg_tys),*) -> Result<(), ::mpi::SendError> {
                        self.inner.send_message(#message_ident::#variant_ident { #(#arg_idents),* })
                    }
                });
                if let Some(protocol) = protocol {
                    let protocol_method = match protocol_item_method(protocol) {
                        Ok(method) => method,
                        Err(error) => return compile_error(error),
                    };
                    let protocol_blocking_method = format_ident!("{}_blocking", protocol_method);
                    protocol_impls.push(quote! {
                        impl #protocol::Target for #handle_ident {
                            fn #protocol_method(
                                &self,
                                ctx: &mut impl ::mpi::TaskScope,
                                payload: #protocol::Payload,
                            ) -> Result<(), ::mpi::SendError> {
                                self.#method_ident(ctx, payload)
                            }

                            fn #protocol_blocking_method(
                                &self,
                                payload: #protocol::Payload,
                            ) -> Result<(), ::mpi::SendError> {
                                self.#blocking_method(payload)
                            }
                        }
                    });
                }
            }
            HandlerKind::Call {
                reply,
                late_reply_policy,
                protocol,
            } => {
                let reply = reply
                    .as_deref()
                    .expect("call reply resolved before code generation");
                let blocking_method = format_ident!("{}_blocking", method_ident);
                variants.push(quote! {
                    #variant_ident {
                        session_id: ::mpi::SessionId,
                        reply: ::mpi::SyncReplySender<#reply>
                        #(, #arg_idents: #arg_tys)*
                    }
                });
                placements.push(quote! {
                    Self::#variant_ident { .. } => ::mpi::MessagePlacement::Normal
                });
                dispatch_arms.push(quote! {
                    #message_ident::#variant_ident { session_id, reply #(, #arg_idents)* } => {
                        if !ctx.inner.take_call_released(session_id) {
                            let __ctx_inner = ctx.inner.clone();
                            let value = ::mpi::block_on_handler(
                                #handler_call,
                                inner_handle.task_endpoint(),
                                &__ctx_inner,
                                &mut *deferred,
                            );
                            let _ = reply.send_from(
                                inner_handle.queue_space_wakeup_target(),
                                ::mpi::Response::new(session_id, value),
                            );
                        }
                    }
                });
                handle_methods.push(quote! {
                    pub fn #method_ident<C>(
                        &self,
                        ctx: &mut C,
                        #(#arg_idents: #arg_tys),*
                    ) -> ::mpi::SuspendedCall<#reply>
                    where
                        C: ::mpi::TaskScope,
                        C::Message: ::mpi::CanReceive<::mpi::Response<#reply>>,
                    {
                        let (session_id, reply, future) =
                            ctx.begin_call_with_late_reply_policy::<#reply>(#late_reply_policy);
                        match self.inner.send_message(#message_ident::#variant_ident {
                            session_id,
                            reply
                            #(, #arg_idents)*
                        }) {
                            Ok(()) => {
                                let __call_lifecycle = self.inner.clone();
                                future.with_additional_on_drop(move |session_id| {
                                    let _ = __call_lifecycle.send_message(
                                        #message_ident::__CallRelease { session_id }
                                    );
                                })
                            }
                            Err(error) => ::mpi::SuspendedCall::failed(error.into()),
                        }
                    }

                    pub fn #blocking_method(&self, #(#arg_idents: #arg_tys),*) -> Result<#reply, ::mpi::CallError> {
                        self.inner
                            .call_blocking(|session_id, reply| #message_ident::#variant_ident {
                                session_id,
                                reply
                                #(, #arg_idents)*
                            })
                            .map(|response| response.value)
                    }
                });
                if let Some(protocol) = protocol {
                    let protocol_method = match protocol_item_method(protocol) {
                        Ok(method) => method,
                        Err(error) => return compile_error(error),
                    };
                    let protocol_blocking_method = format_ident!("{}_blocking", protocol_method);
                    protocol_impls.push(quote! {
                        impl #protocol::Target for #handle_ident {
                            fn #protocol_method<C>(
                                &self,
                                ctx: &mut C,
                                request: #protocol::Request,
                            ) -> ::mpi::SuspendedCall<#protocol::ReplyPayload>
                            where
                                C: ::mpi::TaskScope,
                                C::Message: ::mpi::CanReceive<#protocol::Reply>,
                            {
                                let (session_id, reply, future) =
                                    ctx.begin_call_with_late_reply_policy::<#protocol::ReplyPayload>(#late_reply_policy);
                                match self.inner.send_message(#message_ident::#variant_ident {
                                    session_id,
                                    reply
                                    #(, #arg_idents: request)*
                                }) {
                                    Ok(()) => {
                                        let __call_lifecycle = self.inner.clone();
                                        future.with_additional_on_drop(move |session_id| {
                                            let _ = __call_lifecycle.send_message(
                                                #message_ident::__CallRelease { session_id }
                                            );
                                        })
                                    }
                                    Err(error) => ::mpi::SuspendedCall::failed(error.into()),
                                }
                            }

                            fn #protocol_blocking_method(
                                &self,
                                request: #protocol::Request,
                            ) -> Result<#protocol::ReplyPayload, ::mpi::CallError> {
                                self.#blocking_method(request)
                            }
                        }
                    });
                }
            }
            HandlerKind::Stream {
                item,
                error,
                batch_size,
                late_reply_policy,
                protocol,
            } => {
                let stream_handler_call = quote! {
                    #task_ident::#method_ident(
                        &mut ctx,
                        &mut out,
                        #(#arg_idents),*
                    )
                };
                let item = &**item;
                let error = &**error;
                let blocking_method = format_ident!("{}_blocking", method_ident);
                variants.push(quote! {
                    #variant_ident {
                        session_id: ::mpi::SessionId,
                        events: ::mpi::StreamEventSender<#item, #error>
                        #(, #arg_idents: #arg_tys)*
                    }
                });
                placements.push(quote! {
                    Self::#variant_ident { .. } => ::mpi::MessagePlacement::Normal
                });
                dispatch_arms.push(quote! {
                    #message_ident::#variant_ident { session_id, mut events #(, #arg_idents)* } => {
                        let __sender = inner_handle.queue_space_wakeup_target();
                        let mut out = ::mpi::StreamSink::new_flow_controlled_from(
                            __sender.clone(),
                            session_id,
                            #batch_size,
                            Box::new(move |event: ::mpi::StreamEvent<#item, #error>| {
                                events.send_from(__sender.clone(), event)
                            }) as Box<dyn ::mpi::StreamEventSink<#item, #error> + Send>,
                        );
                        let __ctx_inner = ctx.inner.clone();
                        let result = ::mpi::block_on_handler(
                            #stream_handler_call,
                            inner_handle.task_endpoint(),
                            &__ctx_inner,
                            &mut *deferred,
                        );
                        match result {
                            Ok(()) => {
                                let _ = out.finish();
                            }
                            Err(error) => {
                                let _ = out.fail(error);
                            }
                        }
                    }
                });
                handle_methods.push(quote! {
                    pub fn #method_ident<C>(
                        &self,
                        ctx: &mut C,
                        #(#arg_idents: #arg_tys),*
                    ) -> Result<::mpi::SuspendedMessageStream<#item, #error>, ::mpi::SendError>
                    where
                        C: ::mpi::TaskScope,
                        C::Message: ::mpi::CanReceive<::mpi::StreamEvent<#item, #error>>,
                    {
                        let control = ::std::sync::Arc::new(#stream_control_ident {
                            inner: self.inner.clone(),
                        });
                        let (session_id, events, stream) =
                            ctx.begin_stream_with_late_reply_policy::<#item, #error>(
                                control,
                                #late_reply_policy,
                            );
                        self.inner.send_message(#message_ident::#variant_ident {
                            session_id,
                            events
                            #(, #arg_idents)*
                        })?;
                        Ok(stream)
                    }

                    pub fn #blocking_method(
                        &self,
                        #(#arg_idents: #arg_tys),*
                    ) -> Result<::mpi::BlockingMessageStream<#item, #error>, ::mpi::SendError> {
                        let session_id = self.inner.next_external_session_id();
                        let (events, receiver) = ::std::sync::mpsc::channel::<::mpi::StreamEvent<#item, #error>>();
                        let events = ::mpi::StreamEventSender::new(Box::new(move |event| {
                            events
                                .send(event)
                                .map_err(|_| ::mpi::SendError::TaskStopped)
                        }) as Box<dyn ::mpi::StreamEventSink<#item, #error> + Send>);
                        let control = ::std::sync::Arc::new(#stream_control_ident {
                            inner: self.inner.clone(),
                        });
                        let stream = ::mpi::BlockingMessageStream::new(session_id, control, receiver);
                        self.inner.send_message(#message_ident::#variant_ident {
                            session_id,
                            events
                            #(, #arg_idents)*
                        })?;
                        Ok(stream)
                    }
                });
                if let Some(protocol) = protocol {
                    let protocol_method = match protocol_item_method(protocol) {
                        Ok(method) => method,
                        Err(error) => return compile_error(error),
                    };
                    let protocol_blocking_method = format_ident!("{}_blocking", protocol_method);
                    protocol_impls.push(quote! {
                        impl #protocol::Target for #handle_ident {
                            fn #protocol_method<C>(
                                &self,
                                ctx: &mut C,
                                request: #protocol::Request,
                            ) -> Result<::mpi::SuspendedMessageStream<#protocol::ItemPayload, #protocol::ErrorPayload>, ::mpi::SendError>
                            where
                                C: ::mpi::TaskScope,
                                C::Message: ::mpi::CanReceive<#protocol::Item>,
                            {
                                let control = ::std::sync::Arc::new(#stream_control_ident {
                                    inner: self.inner.clone(),
                                });
                                let (session_id, events, stream) =
                                    ctx.begin_stream_with_late_reply_policy::<#protocol::ItemPayload, #protocol::ErrorPayload>(
                                        control,
                                        #late_reply_policy,
                                    );
                                self.inner.send_message(#message_ident::#variant_ident {
                                    session_id,
                                    events
                                    #(, #arg_idents: request)*
                                })?;
                                Ok(stream)
                            }

                            fn #protocol_blocking_method(
                                &self,
                                request: #protocol::Request,
                            ) -> Result<::mpi::BlockingMessageStream<#protocol::ItemPayload, #protocol::ErrorPayload>, ::mpi::SendError> {
                                self.#blocking_method(request)
                            }
                        }
                    });
                }
            }
            HandlerKind::LateReply => {}
        }
    }

    if start_count == 0 {
        let variant_ident = format_ident!("Start");
        start_variant = Some(quote! { #message_ident::#variant_ident });
        variants.push(quote! { #variant_ident });
        placements.push(quote! {
            Self::#variant_ident => ::mpi::MessagePlacement::Priority
        });
        dispatch_arms.push(quote! {
            #message_ident::#variant_ident => {}
        });
    }

    let start_variant = start_variant.expect("start variant synthesized above");
    let start_arg_idents: Vec<_> = start_args.iter().map(|arg| &arg.ident).collect();
    let start_arg_tys: Vec<_> = start_args.iter().map(|arg| &arg.ty).collect();
    let receive_impls = receives.iter().map(|receive_ty| {
        quote! {
            impl ::mpi::CanReceive<#receive_ty> for #message_ident
            where
                #receive_ty: ::mpi::ProtocolReceive,
            {
                fn wrap(value: #receive_ty) -> Self {
                    <#receive_ty as ::mpi::ProtocolReceive>::into_task_message::<Self>(value)
                }
            }
        }
    });

    let expanded = quote! {
        #item_impl

        #[allow(private_interfaces)]
        pub enum #message_ident {
            #(#variants),*
        }

        impl ::mpi::TaskMessage for #message_ident {
            fn placement(&self) -> ::mpi::MessagePlacement {
                match self {
                    #(#placements),*
                }
            }
        }

        #(#receive_impls)*

        impl ::mpi::CallResponseMessage for #message_ident {
            fn call_response_with_late_reply_policy(
                session_id: ::mpi::SessionId,
                value: Box<dyn ::std::any::Any + Send>,
                late_reply_policy: ::mpi::LateReplyPolicy,
            ) -> Self {
                Self::__CallResponse {
                    session_id,
                    value,
                    late_reply_policy,
                }
            }

            fn into_call_response(self) -> Result<::mpi::QueuedCallResponse, Self> {
                match self {
                    Self::__CallResponse {
                        session_id,
                        value,
                        late_reply_policy,
                    } => {
                        Ok(::mpi::QueuedCallResponse::with_late_reply_policy(
                            session_id,
                            value,
                            late_reply_policy,
                        ))
                    }
                    other => Err(other),
                }
            }
        }

        impl ::mpi::CallReleaseMessage for #message_ident {
            fn call_release(session_id: ::mpi::SessionId) -> Self {
                Self::__CallRelease { session_id }
            }

            fn into_call_release(self) -> Result<::mpi::QueuedCallRelease, Self> {
                match self {
                    Self::__CallRelease { session_id } => {
                        Ok(::mpi::QueuedCallRelease::new(session_id))
                    }
                    other => Err(other),
                }
            }
        }

        impl ::mpi::StreamPullMessage for #message_ident {
            fn stream_pull(session_id: ::mpi::SessionId, credit: u32) -> Self {
                Self::__StreamPull { session_id, credit }
            }

            fn into_stream_pull(self) -> Result<::mpi::StreamPull, Self> {
                match self {
                    Self::__StreamPull { session_id, credit } => {
                        Ok(::mpi::StreamPull::new(session_id, credit))
                    }
                    other => Err(other),
                }
            }
        }

        impl ::mpi::StreamCancelMessage for #message_ident {
            fn stream_cancel(session_id: ::mpi::SessionId) -> Self {
                Self::__StreamCancel { session_id }
            }

            fn into_stream_cancel(self) -> Result<::mpi::StreamCancel, Self> {
                match self {
                    Self::__StreamCancel { session_id } => {
                        Ok(::mpi::StreamCancel::new(session_id))
                    }
                    other => Err(other),
                }
            }
        }

        impl ::mpi::StreamEventMessage for #message_ident {
            fn stream_event_with_late_reply_policy(
                session_id: ::mpi::SessionId,
                event: Box<dyn ::std::any::Any + Send>,
                late_reply_policy: ::mpi::LateReplyPolicy,
            ) -> Self {
                Self::__StreamEvent {
                    session_id,
                    event,
                    late_reply_policy,
                }
            }

            fn into_stream_event(self) -> Result<::mpi::QueuedStreamEvent, Self> {
                match self {
                    Self::__StreamEvent {
                        session_id,
                        event,
                        late_reply_policy,
                    } => {
                        Ok(::mpi::QueuedStreamEvent::with_late_reply_policy(
                            session_id,
                            event,
                            late_reply_policy,
                        ))
                    }
                    other => Err(other),
                }
            }
        }

        impl ::mpi::QueueSpaceWakeupMessage for #message_ident {
            fn queue_space_wakeup() -> Self {
                Self::__QueueSpaceWakeup
            }

            fn into_queue_space_wakeup(self) -> Result<::mpi::QueueSpaceWakeup, Self> {
                match self {
                    Self::__QueueSpaceWakeup => Ok(::mpi::QueueSpaceWakeup),
                    other => Err(other),
                }
            }
        }

        struct #stream_control_ident {
            inner: ::mpi::TaskHandle<#message_ident, #queue_size>,
        }

        impl ::mpi::StreamControl for #stream_control_ident {
            fn try_pull(&self, session_id: ::mpi::SessionId, credit: u32) -> Result<(), ::mpi::SendError> {
                self.inner.send_message(#message_ident::__StreamPull { session_id, credit })
            }

            fn try_cancel(&self, session_id: ::mpi::SessionId) -> Result<(), ::mpi::SendError> {
                self.inner.send_message(#message_ident::__StreamCancel { session_id })
            }
        }

        #[derive(Clone)]
        pub struct #handle_ident {
            inner: ::mpi::TaskHandle<#message_ident, #queue_size>,
        }

        impl #handle_ident {
            pub fn endpoint(&self) -> ::mpi::EndpointId {
                self.inner.endpoint()
            }

            pub fn close(&self) {
                self.inner.close();
            }

            #(#handle_methods)*
        }

        impl From<#handle_ident> for ::mpi::TaskHandle<#message_ident, #queue_size> {
            fn from(handle: #handle_ident) -> Self {
                handle.inner
            }
        }

        #(#protocol_impls)*

        pub struct #context_ident {
            inner: ::mpi::TaskContext<#message_ident, #queue_size>,
            state: ::std::rc::Rc<::std::cell::RefCell<#task_ident>>,
        }

        impl ::mpi::TaskScope for #context_ident {
            type Message = #message_ident;

            fn begin_call<T: Send + 'static>(&mut self) -> ::mpi::CallSession<T> {
                self.inner.begin_call::<T>()
            }

            fn endpoint(&self) -> ::mpi::EndpointId {
                self.inner.endpoint()
            }

            fn begin_call_with_late_reply_policy<T: Send + 'static>(
                &mut self,
                late_reply_policy: ::mpi::LateReplyPolicy,
            ) -> ::mpi::CallSession<T> {
                self.inner.begin_call_with_late_reply_policy::<T>(late_reply_policy)
            }

            fn begin_stream<T: Send + 'static, E: Send + 'static>(
                &mut self,
                control: ::std::sync::Arc<dyn ::mpi::StreamControl>,
            ) -> ::mpi::StreamSession<T, E> {
                self.inner.begin_stream::<T, E>(control)
            }

            fn begin_stream_with_late_reply_policy<T: Send + 'static, E: Send + 'static>(
                &mut self,
                control: ::std::sync::Arc<dyn ::mpi::StreamControl>,
                late_reply_policy: ::mpi::LateReplyPolicy,
            ) -> ::mpi::StreamSession<T, E> {
                self.inner.begin_stream_with_late_reply_policy::<T, E>(
                    control,
                    late_reply_policy,
                )
            }
        }

        impl #context_ident {
            pub fn with_state<R>(&mut self, f: impl FnOnce(&mut #task_ident) -> R) -> R {
                let mut state = self.state.borrow_mut();
                f(&mut state)
            }

            pub fn self_handle(&self) -> #handle_ident {
                #handle_ident {
                    inner: self.inner.self_handle(),
                }
            }

            pub fn next_session_id(&mut self) -> ::mpi::SessionId {
                self.inner.next_session_id()
            }

            pub fn stop(&mut self) {
                self.inner.stop();
            }

            pub fn is_stopped(&self) -> bool {
                self.inner.is_stopped()
            }

            pub fn diagnostics_snapshot(&self) -> ::mpi::TaskDiagnosticsSnapshot {
                self.inner.diagnostics_snapshot()
            }

        }

        impl #task_ident {
            pub fn spawn(
                state: Self
                #(, #start_arg_idents: #start_arg_tys)*
            ) -> Result<(#handle_ident, ::mpi::TaskRuntime<()>), ::mpi::SendError>
            where
                Self: Send + 'static,
            {
                let (inner, runtime) = ::mpi::spawn_task::<#message_ident, _, _, #queue_size>(
                    #start_variant,
                    #priority_reserved,
                    move |inner_handle| {
                        let state = ::std::rc::Rc::new(::std::cell::RefCell::new(state));

                        fn __dispatch_message(
                            state: &::std::rc::Rc<::std::cell::RefCell<#task_ident>>,
                            inner_handle: &::mpi::TaskHandle<#message_ident, #queue_size>,
                            mut ctx: #context_ident,
                            deferred: &mut ::std::collections::VecDeque<#message_ident>,
                            message: #message_ident,
                        ) -> #context_ident {
                            match message {
                                #(#dispatch_arms),*
                            }
                            ctx
                        }

                        let mut ctx = #context_ident {
                            inner: ::mpi::TaskContext::new(inner_handle.clone()),
                            state: state.clone(),
                        };
                        let mut deferred = ::std::collections::VecDeque::<#message_ident>::new();

                        loop {
                            if ctx.is_stopped() {
                                break;
                            }

                            let message = match deferred.pop_front() {
                                Some(message) => message,
                                None => match inner_handle.recv_message() {
                                    Ok(message) => message,
                                    Err(_) => break,
                                },
                            };

                            ctx = __dispatch_message(
                                &state,
                                &inner_handle,
                                ctx,
                                &mut deferred,
                                message,
                            );

                            if ctx.is_stopped() {
                                break;
                            }
                        }
                    },
                )?;

                Ok((#handle_ident { inner }, runtime))
            }
        }
    };

    expanded.into()
}

fn passthrough(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks the start handler for a task.
#[proc_macro_attribute]
pub fn start(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}

/// Marks an asynchronous event handler.
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}

/// Marks a synchronous call handler.
#[proc_macro_attribute]
pub fn call(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}

/// Marks a streaming call handler.
#[proc_macro_attribute]
pub fn stream(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}

/// Marks the optional callback for reported late replies.
#[proc_macro_attribute]
pub fn late_reply(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}
