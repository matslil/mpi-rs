//! Procedural macros for `mpi` task declarations.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, LitStr, Pat, PatType, Token, Type,
    parse_macro_input,
};

struct TaskArgs {
    queue_size: TokenStream2,
}

impl Parse for TaskArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "queue_size" {
            return Err(syn::Error::new_spanned(key, "expected `queue_size`"));
        }
        input.parse::<Token![=]>()?;
        let value: syn::Expr = input.parse()?;
        Ok(Self {
            queue_size: value.into_token_stream(),
        })
    }
}

struct CallArgs {
    reply: Type,
    late_reply_policy: TokenStream2,
}

impl Parse for CallArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut reply = None;
        let mut late_reply_policy = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "reply" {
                reply = Some(input.parse()?);
            } else if key == "late_reply" {
                late_reply_policy = Some(parse_late_reply_policy(input.parse()?)?);
            } else {
                return Err(syn::Error::new_spanned(
                    key,
                    "expected `reply` or `late_reply`",
                ));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let reply = reply.ok_or_else(|| input.error("missing `reply` call attribute"))?;
        let late_reply_policy =
            late_reply_policy.unwrap_or_else(|| quote! { ::mpi::LateReplyPolicy::Report });
        Ok(Self {
            reply,
            late_reply_policy,
        })
    }
}

struct StreamArgs {
    item: Type,
    error: Type,
    batch_size: TokenStream2,
    late_reply_policy: TokenStream2,
}

impl Parse for StreamArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut item = None;
        let mut error = None;
        let mut batch_size = None;
        let mut late_reply_policy = None;

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
            } else {
                return Err(syn::Error::new_spanned(
                    key,
                    "expected `item`, `error`, `batch_size`, or `late_reply`",
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
        })
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
    },
    Call {
        reply: Box<Type>,
        late_reply_policy: TokenStream2,
    },
    Stream {
        item: Box<Type>,
        error: Box<Type>,
        batch_size: TokenStream2,
        late_reply_policy: TokenStream2,
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
            "event" => HandlerKind::Event {
                priority: attr.to_token_stream().to_string().contains("priority"),
            },
            "call" => {
                let args = attr.parse_args::<CallArgs>()?;
                HandlerKind::Call {
                    reply: Box::new(args.reply),
                    late_reply_policy: args.late_reply_policy,
                }
            }
            "stream" => {
                let args = attr.parse_args::<StreamArgs>()?;
                HandlerKind::Stream {
                    item: Box::new(args.item),
                    error: Box::new(args.error),
                    batch_size: args.batch_size,
                    late_reply_policy: args.late_reply_policy,
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

fn payload_args(method: &ImplItemFn, skip_stream_sink: bool) -> syn::Result<Vec<HandlerArg>> {
    let mut inputs = method.sig.inputs.iter();

    match inputs.next() {
        Some(FnArg::Receiver(_)) => {}
        _ => {
            return Err(syn::Error::new_spanned(
                &method.sig,
                "handler must take `&mut self` as first parameter",
            ));
        }
    }

    match inputs.next() {
        Some(FnArg::Typed(_)) => {}
        _ => {
            return Err(syn::Error::new_spanned(
                &method.sig,
                "handler must take a context parameter after `self`",
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
                    let skip_stream_sink = matches!(kind, HandlerKind::Stream { .. });
                    let args = match payload_args(&method, skip_stream_sink) {
                        Ok(args) => args,
                        Err(error) => return compile_error(error),
                    };
                    if matches!(kind, HandlerKind::LateReply) && args.len() != 1 {
                        return compile_error(syn::Error::new_spanned(
                            &method.sig,
                            "late reply handler must take exactly one late reply argument after the context",
                        ));
                    }
                    strip_handler_attrs(&mut method);
                    handlers.push(Handler { kind, method, args });
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
    if start_count != 1 {
        return compile_error(syn::Error::new_spanned(
            &task_ident,
            "#[task] requires exactly one #[start] handler",
        ));
    }
    let late_reply_count = handlers
        .iter()
        .filter(|handler| matches!(handler.kind, HandlerKind::LateReply))
        .count();
    if late_reply_count > 1 {
        return compile_error(syn::Error::new_spanned(
            &task_ident,
            "#[task] supports at most one #[late_reply] handler",
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
    let mut start_args = Vec::new();
    let mut start_variant = None;
    let late_reply_method = handlers
        .iter()
        .find(|handler| matches!(handler.kind, HandlerKind::LateReply))
        .map(|handler| handler.method.sig.ident.clone());
    let deliver_call_response = if let Some(method_ident) = &late_reply_method {
        quote! {
            let __ctx_inner = ctx.inner.clone();
            let _ = __ctx_inner.deliver_call_response_with_late_reply_handler(
                ::mpi::QueuedCallResponse::with_late_reply_policy(
                    session_id,
                    value,
                    late_reply_policy,
                ),
                |late_reply| state.#method_ident(&mut ctx, late_reply),
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
    let deliver_stream_event = if let Some(method_ident) = &late_reply_method {
        quote! {
            let __ctx_inner = ctx.inner.clone();
            let _ = __ctx_inner.deliver_stream_event_with_late_reply_handler(
                ::mpi::QueuedStreamEvent::with_late_reply_policy(
                    session_id,
                    event,
                    late_reply_policy,
                ),
                |late_reply| state.#method_ident(&mut ctx, late_reply),
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
                        ::mpi::block_on_task(
                            state.#method_ident(&mut ctx, #(#arg_idents),*),
                            inner_handle.queue(),
                            &__ctx_inner,
                            &mut deferred,
                        );
                    }
                });
            }
            HandlerKind::Event { priority } => {
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
                        let __ctx_inner = ctx.inner.clone();
                        ::mpi::block_on_task(
                            state.#method_ident(&mut ctx, #(#arg_idents),*),
                            inner_handle.queue(),
                            &__ctx_inner,
                            &mut deferred,
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
            }
            HandlerKind::Call {
                reply,
                late_reply_policy,
            } => {
                let reply = &**reply;
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
                            let value = ::mpi::block_on_task(
                                state.#method_ident(&mut ctx, #(#arg_idents),*),
                                inner_handle.queue(),
                                &__ctx_inner,
                                &mut deferred,
                            );
                            let _ = reply.send(::mpi::Response::new(session_id, value));
                        }
                    }
                });
                handle_methods.push(quote! {
                    pub fn #method_ident(
                        &self,
                        ctx: &mut impl ::mpi::TaskScope,
                        #(#arg_idents: #arg_tys),*
                    ) -> ::mpi::SuspendedCall<#reply> {
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
            }
            HandlerKind::Stream {
                item,
                error,
                batch_size,
                late_reply_policy,
            } => {
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
                        let mut out = ::mpi::StreamSink::new(
                            session_id,
                            #batch_size,
                            Box::new(move |event: ::mpi::StreamEvent<#item, #error>| {
                                events.send(event)
                            }) as Box<dyn ::mpi::StreamEventSink<#item, #error> + Send>,
                        );
                        let __ctx_inner = ctx.inner.clone();
                        let result = ::mpi::block_on_task(
                            state.#method_ident(
                                &mut ctx,
                                &mut out,
                                #(#arg_idents),*
                            ),
                            inner_handle.queue(),
                            &__ctx_inner,
                            &mut deferred,
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
                    pub fn #method_ident(
                        &self,
                        ctx: &mut impl ::mpi::TaskScope,
                        #(#arg_idents: #arg_tys),*
                    ) -> Result<::mpi::SuspendedMessageStream<#item, #error>, ::mpi::SendError> {
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
                        self.inner.send_message(#message_ident::#variant_ident {
                            session_id,
                            events
                            #(, #arg_idents)*
                        })?;
                        let control = ::std::sync::Arc::new(#stream_control_ident {
                            inner: self.inner.clone(),
                        });
                        Ok(::mpi::BlockingMessageStream::new(session_id, control, receiver))
                    }
                });
            }
            HandlerKind::LateReply => {}
        }
    }

    let start_variant = start_variant.expect("start handler counted above");
    let start_arg_idents: Vec<_> = start_args.iter().map(|arg| &arg.ident).collect();
    let start_arg_tys: Vec<_> = start_args.iter().map(|arg| &arg.ty).collect();

    let expanded = quote! {
        #item_impl

        enum #message_ident {
            #(#variants),*
        }

        impl ::mpi::TaskMessage for #message_ident {
            fn placement(&self) -> ::mpi::MessagePlacement {
                match self {
                    #(#placements),*
                }
            }
        }

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

        pub struct #context_ident {
            inner: ::mpi::TaskContext<#message_ident, #queue_size>,
        }

        impl ::mpi::TaskScope for #context_ident {
            fn begin_call<T: Send + 'static>(&mut self) -> ::mpi::CallSession<T> {
                self.inner.begin_call::<T>()
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

        }

        impl #task_ident {
            pub fn spawn(
                mut state: Self
                #(, #start_arg_idents: #start_arg_tys)*
            ) -> Result<(#handle_ident, ::mpi::TaskRuntime<()>), ::mpi::SendError>
            where
                Self: Send + 'static,
            {
                let (inner, runtime) = ::mpi::spawn_task::<#message_ident, _, _, #queue_size>(
                    #start_variant,
                    move |inner_handle| {
                        let mut ctx = #context_ident {
                            inner: ::mpi::TaskContext::new(inner_handle.clone()),
                        };
                        let mut deferred = ::std::collections::VecDeque::<#message_ident>::new();

                        loop {
                            if ctx.is_stopped() {
                                break;
                            }

                            let message = match deferred.pop_front() {
                                Some(message) => message,
                                None => match inner_handle.queue().recv() {
                                    Ok(message) => message,
                                    Err(_) => break,
                                },
                            };

                            match message {
                                #(#dispatch_arms),*
                            }

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

/// Marks the optional handler for reported late replies.
#[proc_macro_attribute]
pub fn late_reply(attr: TokenStream, item: TokenStream) -> TokenStream {
    passthrough(attr, item)
}
