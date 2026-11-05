// SPDX-License-Identifier: MIT
//! Standalone process adapter for Text Rewrap.
//!
//! The reviewed Native V2 adapter (`v2/src/tezbar_adapter.rs`) and engine
//! (`v2/src/engine.rs`) are compiled **unchanged**. This crate only supplies the
//! two host-facing modules that adapter imports, backed by the independent SDK:
//! `native_core::common::CoreContext` maps the adapter's three legacy channels to
//! narrowly granted typed `host.*` reverse RPCs; `v2_sdk` provides view helpers.
use serde_json::{json, Value};
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
            let mut payload = payload.clone();
            // PreferenceSetup and retained legacy preferences can encode checkboxes
            // as strings. Normalize only the recognized values; v2 owns defaults.
            if let Some(value) = payload.pointer_mut("/preferences/defaultCopyToClipboard") {
                match value.as_str() {
                    Some("false") => *value = json!(false),
                    Some("true") => *value = json!(true),
                    _ => {}
                }
            }
            let mut result = adapter::handle(&core, command, operation, &payload, state).await?;
            // The current invoke-action consumer requires a complete view. Keep
            // protocol-level unchanged out of that renderer path.
            if operation == "extension:invoke-action" && result["mode"] == "unchanged" {
                result = adapter::handle(
                    &core,
                    command,
                    "extension:refresh-session",
                    &json!({}),
                    state,
                )
                .await?;
            }
            adapt_view(&mut result, state, operation == "extension:invoke-action");
            Ok(result)
        })
    }
}

/// Translate the frozen v2 view into surfaces the current launcher consumes.
/// The engine and session state (including retry output) stay owned by v2.
fn adapt_view(result: &mut Value, state: &Value, action: bool) {
    if let Some(fields) = result
        .pointer_mut("/root/children")
        .and_then(Value::as_array_mut)
    {
        for field in fields {
            if field["type"] == "Form.Description" && field["props"]["title"] == "Rewrapped Text" {
                *field = json!({
                    "type": "Form.TextArea",
                    "props": {
                        "id": format!("output:{}:{}", state["epoch"], state["revision"]),
                        "title": "Rewrapped Text",
                        "value": state["output"],
                        "info": "Generated preview. Copy Rewrapped Text copies the stored result."
                    }
                });
            }
        }
    }
    // In-form descriptions do not suppress the launcher's optimistic Copy
    // fallback. Emit the feedback as a supported toast effect on every action.
    if action {
        if let Some(feedback) = state["feedback"].as_str() {
            let style = if feedback.starts_with("Text rewrapped") {
                "success"
            } else {
                "failure"
            };
            result["effects"] = json!([{"kind":"toast", "style":style, "title":feedback}]);
        }
    }
}
