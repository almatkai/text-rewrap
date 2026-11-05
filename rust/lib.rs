// SPDX-License-Identifier: MIT
//! Standalone process adapter for Text Rewrap.
//!
//! The reviewed Native V2 adapter (`v2/src/tezbar_adapter.rs`) and engine
//! (`v2/src/engine.rs`) are compiled **unchanged**. This crate only supplies the
//! two host-facing modules that adapter imports, backed by the independent SDK:
//! `native_core::common::CoreContext` maps the adapter's three legacy channels to
//! narrowly granted typed `host.*` reverse RPCs; `v2_sdk` provides view helpers.
use serde_json::Value;
use tezbar_extension_sdk::{BoxFuture, Extension, ExtensionContext};

pub const EXTENSION_ID: &str = "com.tezbar.native.text-rewrap";

pub mod native_core {
    pub mod common {
        use serde_json::{json, Value};
        use tezbar_extension_sdk::ExtensionContext;

        pub type CoreResult = tezbar_extension_sdk::CoreResult;

        /// Narrow host boundary. Only the channels the adapter actually uses are
        /// mapped; everything else fails closed before any reverse RPC is sent.
        pub struct CoreContext {
            inner: ExtensionContext,
        }
        impl CoreContext {
            pub fn new(inner: ExtensionContext) -> Self {
                Self { inner }
            }
            pub async fn request(&self, channel: &str, args: Vec<Value>) -> CoreResult {
                let (method, params) = host_method(channel, &args)?;
                self.inner.host(method, params).await
            }
        }

        /// Channel → typed host request, exactly as the Tezbar process host
        /// routes these methods back to the same core channels.
        pub fn host_method(channel: &str, args: &[Value]) -> Result<(&'static str, Value), String> {
            match channel {
                "clipboard:read" if args.is_empty() => Ok(("host.clipboard.read", json!({}))),
                "clipboard:write" => match args {
                    [Value::String(text)] => Ok(("host.clipboard.write", json!({ "text": text }))),
                    _ => Err("Clipboard write expects one text argument".into()),
                },
                "core:platform:selected-text" if args.is_empty() => {
                    Ok(("host.selection.read", json!({})))
                }
                _ => Err(format!(
                    "Host channel not available to Text Rewrap: {channel}"
                )),
            }
        }
    }

    pub mod extensions {
        pub mod v2_sdk {
            use crate::native_core::common::{CoreContext, CoreResult};
            use serde_json::json;
            pub use tezbar_extension_sdk::{unchanged, view};
            pub async fn copy(ctx: &CoreContext, text: &str) -> CoreResult {
                ctx.request("clipboard:write", vec![json!(text)]).await
            }
        }
        pub mod v2_text_rewrap {
            pub use tezbar_text_rewrap::engine;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/v2/src/tezbar_adapter.rs"
            ));
        }
    }
}

use native_core::extensions::v2_text_rewrap as adapter;

pub struct TextRewrap;
impl Extension for TextRewrap {
    fn id(&self) -> &'static str {
        EXTENSION_ID
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn commands(&self) -> &'static [&'static str] {
        adapter::commands()
    }
    fn handle<'a>(
        &'a self,
        ctx: &'a ExtensionContext,
        command: &'a str,
        operation: &'a str,
        payload: &'a Value,
        state: &'a mut Value,
    ) -> BoxFuture<'a, tezbar_extension_sdk::CoreResult> {
        Box::pin(async move {
            let core = native_core::common::CoreContext::new(ctx.clone());
            adapter::handle(&core, command, operation, payload, state).await
        })
    }
}
