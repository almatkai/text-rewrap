// SPDX-License-Identifier: MIT
// Compiled unchanged as a host module; no OS, profile, persistent storage or JS runtime.
use self::engine::{format_message, validate_width, wrap};
use super::v2_sdk as sdk;
use crate::native_core::common::{CoreContext, CoreResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static EPOCH: AtomicU64 = AtomicU64::new(1);
pub fn commands() -> &'static [&'static str] {
    &[
        "text-rewrap-form",
        "text-rewrap-clipboard",
        "text-rewrap-selection",
    ]
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Draft {
    #[serde(default)]
    text: String,
    width: String,
    #[serde(rename = "copyResultToClipboard")]
    copy: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    draft: Draft,
    message: String,
}
#[derive(Serialize, Deserialize)]
struct Session {
    command: String,
    epoch: u64,
    revision: u64,
    defaults: Draft,
    draft: Draft,
    output: String,
    pending: Option<Pending>,
    feedback: Option<String>,
    width_error: Option<String>,
}
impl Session {
    fn action(&self, name: &str) -> String {
        format!("text-rewrap:{}:{}:{name}", self.epoch, self.revision)
    }
    fn field(&self, name: &str) -> String {
        format!("{name}:{}", self.epoch)
    }
    fn view(&self) -> Value {
        // Stable, submit-only server values let beta keep newer queued client drafts.
        let mut fields = Vec::new();
        if self.command == "text-rewrap-form" {
            fields.push(json!({"type":"Form.TextArea","props":{"id":self.field("text"),"title":"Text","placeholder":"Enter text to rewrap...","value":self.defaults.text}}));
        }
        fields.push(json!({"type":"Form.TextField","props":{"id":self.field("width"),"title":"Width","placeholder":"Leave blank for no wrapping, or enter a number like 80","value":self.defaults.width,"error":self.width_error}}));
        fields.push(json!({"type":"Form.Checkbox","props":{"id":self.field("copyResultToClipboard"),"title":"","label":"Copy result to Clipboard?","value":self.defaults.copy}}));
        if !self.output.is_empty() {
            fields.push(json!({"type":"Form.Description","props":{"title":"Rewrapped Text","text":self.output}}));
        }
        if let Some(feedback) = &self.feedback {
            fields.push(
                json!({"type":"Form.Description","props":{"title":"Status","text":feedback}}),
            );
        }
        let mut actions =
            vec![json!({"id":self.action("submit"),"title":"Rewrap Text","kind":"submit-form"})];
        if !self.output.is_empty() {
            actions.push(json!({"id":self.action("copy"),"title":"Copy Rewrapped Text","kind":"copy","shortcut":{"modifiers":["cmd"],"key":"c"}}));
        }
        let title = match self.command.as_str() {
            "text-rewrap-form" => "Text Rewrap from Form",
            "text-rewrap-clipboard" => "Text Rewrap from Clipboard",
            _ => "Text Rewrap from Selection",
        };
        sdk::view(
            title,
            json!({"type":"Form","props":{},"children":fields}),
            actions,
        )
    }
}

async fn copy_checked(ctx: &CoreContext, text: &str) -> Result<(), String> {
    let reply = sdk::copy(ctx, text).await?;
    let object = reply
        .as_object()
        .ok_or("Clipboard write returned a malformed reply")?;
    if object.get("ok") != Some(&json!(true))
        || object.get("error").is_some_and(|v| !v.is_null())
        || object.get("success").is_some_and(|v| v != &json!(true))
    {
        return Err("Clipboard write did not confirm success".into());
    }
    Ok(())
}
async fn finish_copy(ctx: &CoreContext, s: &mut Session, message: &str) {
    match copy_checked(ctx, &s.output).await {
        Ok(()) => {
            s.pending = None;
            s.feedback = Some(format!("Text rewrapped and copied! — {message}"));
        }
        Err(_) => {
            s.feedback = Some(if s.pending.is_some() {
                "Unable to copy result to clipboard. Result preserved. Submit the same inputs to retry the write, or use Copy Rewrapped Text when available."
            } else {
                "Unable to copy result to clipboard. Result preserved. Use Copy Rewrapped Text to retry the stored result."
            }.into());
        }
    }
}

pub async fn handle(
    ctx: &CoreContext,
    command: &str,
    operation: &str,
    payload: &Value,
    state: &mut Value,
) -> CoreResult {
    // Validate command before every operation and effect, even launch/dispose.
    if !commands().contains(&command) {
        return Err("Unknown Text Rewrap command".into());
    }
    if operation == "extension:run-command" {
        let p = &payload["preferences"];
        let defaults = Draft {
            text: String::new(),
            width: p["defaultWidth"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("80")
                .into(),
            copy: p["defaultCopyToClipboard"].as_bool().unwrap_or(true),
        };
        let s = Session {
            command: command.into(),
            epoch: EPOCH.fetch_add(1, Ordering::Relaxed),
            revision: 0,
            draft: defaults.clone(),
            defaults,
            output: String::new(),
            pending: None,
            feedback: None,
            width_error: None,
        };
        let view = s.view();
        *state = serde_json::to_value(s).map_err(|e| e.to_string())?;
        return Ok(view);
    }
    let mut s: Session =
        serde_json::from_value(state.clone()).map_err(|_| "Text Rewrap session is unavailable")?;
    if s.command != command {
        return Err("Text Rewrap session command mismatch".into());
    }
    if operation == "extension:dispose-session" {
        *state = Value::Null;
        return Ok(sdk::unchanged());
    }
    if operation == "extension:refresh-session" {
        return Ok(s.view());
    }
    if operation == "extension:load-more" {
        return Ok(sdk::unchanged());
    }
    if operation != "extension:invoke-action" {
        return Err("Unsupported Text Rewrap operation".into());
    }
    let id = payload["actionId"].as_str().ok_or("Missing action ID")?;
    if id == s.action("copy") && !s.output.is_empty() {
        let message = s
            .pending
            .as_ref()
            .map(|p| p.message.clone())
            .unwrap_or_default();
        finish_copy(ctx, &mut s, &message).await;
    } else if id == s.action("submit") {
        let values = &payload["formValues"];
        let draft = Draft {
            text: if command == "text-rewrap-form" {
                values[&s.field("text")]
                    .as_str()
                    .ok_or("Missing text draft")?
                    .into()
            } else {
                String::new()
            },
            width: values[&s.field("width")]
                .as_str()
                .ok_or("Missing width draft")?
                .into(),
            copy: values[&s.field("copyResultToClipboard")]
                .as_bool()
                .ok_or("Missing copy draft")?,
        };
        s.revision = s
            .revision
            .checked_add(1)
            .ok_or("Action revision exhausted")?;
        s.draft = draft.clone();
        s.width_error = None;
        match validate_width(&draft.width) {
            Err(error) => {
                s.width_error = Some(error.into());
                s.feedback = Some(format!("Invalid width — {error}"));
            }
            Ok(width) => {
                if let Some(pending) = s.pending.clone().filter(|p| p.draft == draft) {
                    // Failed automatic write retries the exact stored output, never an OS reread.
                    finish_copy(ctx, &mut s, &pending.message).await;
                } else {
                    let source = match command {
                        "text-rewrap-form" => Ok(draft.text.clone()),
                        "text-rewrap-clipboard" => match ctx.request("clipboard:read", vec![]).await {
                            Ok(Value::String(text)) if !text.is_empty() => Ok(text),
                            Ok(Value::String(_)) => Err("No text in clipboard — Please copy some text first"),
                            _ => Err("Unable to read clipboard — Please try again"),
                        },
                        _ => match ctx.request("core:platform:selected-text", vec![]).await {
                            Ok(Value::String(text)) if !text.is_empty() => Ok(text),
                            _ => Err("Unable to retrieve selection — Unable to retrieve highlighted selection from current app"),
                        },
                    };
                    match source {
                        Err(error) => s.feedback = Some(error.into()),
                        Ok(text) => {
                            s.output = wrap(&text, width);
                            let message = format_message(width);
                            s.pending = None;
                            if draft.copy {
                                s.pending = Some(Pending {
                                    draft,
                                    message: message.clone(),
                                });
                                finish_copy(ctx, &mut s, &message).await;
                            } else {
                                s.feedback = Some(format!("Text rewrapped! — {message}"));
                            }
                        }
                    }
                }
            }
        }
    } else {
        return Err("Stale or unavailable Text Rewrap action".into());
    }
    let view = s.view();
    *state = serde_json::to_value(s).map_err(|e| e.to_string())?;
    Ok(view)
}
