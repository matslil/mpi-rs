//! Procedural macros for `mpi` task declarations.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, Pat, PatType, Token, Type,
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
}

impl Parse for CallArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "reply" {
            return Err(syn::Error::new_spanned(key, "expected `reply`"));
        }
        input.parse::<Token![=]>()?;
        let reply = input.parse()?;
        Ok(Self { reply })
    }
}

struct StreamArgs {
    item: Type,
    error: Type,
    batch_size: TokenStream2,
}

impl Parse for StreamArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut item = None;
        let mut error = None;
        let mut batch_size = None;

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
            } else {
                return Err(syn::Error::new_spanned(
                    key,
                    "expected `item`, `error`, or `batch_size`",
                ));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let item = item.ok_or_else(|| input.error("missing `item` stream attribute"))?;
        let error = error.ok_or_else(|| input.error("missing `error` stream attribute"))?;
        let batch_size = batch_size.unwrap_or_else(|| quote! { 64usize });

        Ok(Self {
            item,
            error,
            batch_size,
        })
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
    },
    Stream {
        item: Box<Type>,
        error: Box<Type>,
        batch_size: TokenStream2,
    },
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
            "call" => HandlerKind::Call {
                reply: Box::new(attr.parse_args::<CallArgs>()?.reply),
            },
            "stream" => {
                let args = attr.parse_args::<StreamArgs>()?;
                HandlerKind::Stream {
                    item: Box::new(args.item),
                    error: Box::new(args.error),
                    batch_size: args.batch_size,
                }
            }
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

    variants.push(quote! { __StreamCancel { session_id: ::mpi::SessionId } });
    placements.push(quote! { Self::__StreamCancel { .. } => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! { #message_ident::__StreamCancel { session_id: _ } => {} });

    variants.push(quote! {
        __CallResponse {
            session_id: ::mpi::SessionId,
            value: Box<dyn ::std::any::Any + Send>,
        }
    });
    placements.push(quote! { Self::__CallResponse { .. } => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! {
        #message_ident::__CallResponse { session_id, value } => {
            let _ = ctx.inner.deliver_call_response(::mpi::QueuedCallResponse::new(session_id, value));
        }
    });

    variants.push(quote! {
        __StreamEvent {
            session_id: ::mpi::SessionId,
            event: Box<dyn ::std::any::Any + Send>,
        }
    });
    placements.push(quote! { Self::__StreamEvent { .. } => ::mpi::MessagePlacement::Priority });
    dispatch_arms.push(quote! {
        #message_ident::__StreamEvent { session_id, event } => {
            let _ = ctx.inner.deliver_stream_event(::mpi::QueuedStreamEvent::new(session_id, event));
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
            HandlerKind::Call { reply } => {
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
                        let __ctx_inner = ctx.inner.clone();
                        let value = ::mpi::block_on_task(
                            state.#method_ident(&mut ctx, #(#arg_idents),*),
                            inner_handle.queue(),
                            &__ctx_inner,
                            &mut deferred,
                        );
                        let _ = reply.send(::mpi::Response::new(session_id, value));
                    }
                });
                handle_methods.push(quote! {
                    pub fn #method_ident(
                        &self,
                        ctx: &mut impl ::mpi::TaskScope,
                        #(#arg_idents: #arg_tys),*
                    ) -> ::mpi::SuspendedCall<#reply> {
                        let (session_id, reply, future) = ctx.begin_call::<#reply>();
                        match self.inner.send_message(#message_ident::#variant_ident {
                            session_id,
                            reply
                            #(, #arg_idents)*
                        }) {
                            Ok(()) => future,
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
                        let (session_id, events, stream) = ctx.begin_stream::<#item, #error>(control);
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
            fn call_response(
                session_id: ::mpi::SessionId,
                value: Box<dyn ::std::any::Any + Send>,
            ) -> Self {
                Self::__CallResponse { session_id, value }
            }

            fn into_call_response(self) -> Result<::mpi::QueuedCallResponse, Self> {
                match self {
                    Self::__CallResponse { session_id, value } => {
                        Ok(::mpi::QueuedCallResponse::new(session_id, value))
                    }
                    other => Err(other),
                }
            }
        }

        impl ::mpi::StreamEventMessage for #message_ident {
            fn stream_event(
                session_id: ::mpi::SessionId,
                event: Box<dyn ::std::any::Any + Send>,
            ) -> Self {
                Self::__StreamEvent { session_id, event }
            }

            fn into_stream_event(self) -> Result<::mpi::QueuedStreamEvent, Self> {
                match self {
                    Self::__StreamEvent { session_id, event } => {
                        Ok(::mpi::QueuedStreamEvent::new(session_id, event))
                    }
                    other => Err(other),
                }
            }
        }

        struct #stream_control_ident {
            inner: ::mpi::TaskHandle<#message_ident, #queue_size>,
        }

        impl ::mpi::StreamControl for #stream_control_ident {
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

            fn begin_stream<T: Send + 'static, E: Send + 'static>(
                &mut self,
                control: ::std::sync::Arc<dyn ::mpi::StreamControl>,
            ) -> ::mpi::StreamSession<T, E> {
                self.inner.begin_stream::<T, E>(control)
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
