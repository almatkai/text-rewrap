// SPDX-License-Identifier: MIT
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tezbar_text_rewrap::native_core::common::{CoreContext, CorePaths, CoreResult};
use tezbar_text_rewrap::native_core::extensions::v2_text_rewrap::{commands, handle};
const RUN: &str = "extension:run-command";
const INVOKE: &str = "extension:invoke-action";
type Calls = Arc<Mutex<Vec<(String, Vec<Value>)>>>;
struct Harness {
    _temp: tempfile::TempDir,
    ctx: CoreContext,
    calls: Calls,
    replies: Arc<Mutex<VecDeque<CoreResult>>>,
    state: Value,
    view: Value,
    command: &'static str,
}
impl Harness {
    fn new(command: &'static str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let p = temp.path();
        let ctx = CoreContext::new(
            CorePaths {
                data_dir: p.join("data"),
                config_dir: p.join("config"),
                resource_dir: p.join("res"),
                temp_dir: p.join("tmp"),
                home_dir: p.join("home"),
            },
            None,
        );
        let calls: Calls = Arc::default();
        let replies: Arc<Mutex<VecDeque<CoreResult>>> = Arc::default();
        let (c, r) = (calls.clone(), replies.clone());
        ctx.set_router(Arc::new(move |route, args| {
            c.lock().unwrap().push((route, args));
            let reply = r.lock().unwrap().pop_front().expect("unexpected effect");
            Box::pin(async move { reply })
        }))
        .unwrap();
        Self {
            _temp: temp,
            ctx,
            calls,
            replies,
            state: Value::Null,
            view: Value::Null,
            command,
        }
    }
    async fn run(&mut self, prefs: Value) {
        self.view = handle(
            &self.ctx,
            self.command,
            RUN,
            &json!({"preferences":prefs}),
            &mut self.state,
        )
        .await
        .unwrap();
    }
    fn replies(&self, replies: Vec<CoreResult>) {
        self.replies.lock().unwrap().extend(replies);
    }
    async fn submit(&mut self, width: &str, copy: bool, text: &str) {
        let mut values = serde_json::Map::new();
        for field in self.view["root"]["children"].as_array().unwrap() {
            if let Some(id) = field["props"]["id"].as_str() {
                let value = match id.split(':').next().unwrap() {
                    "text" => json!(text),
                    "width" => json!(width),
                    _ => json!(copy),
                };
                values.insert(id.into(), value);
            }
        }
        let payload = json!({"actionId":self.view["actions"][0]["id"],"formValues":values});
        self.view = handle(&self.ctx, self.command, INVOKE, &payload, &mut self.state)
            .await
            .unwrap();
    }
    async fn copy(&mut self) {
        let payload =
            json!({"actionId":self.view["actions"][1]["id"],"text":"forged","output":"forged"});
        self.view = handle(&self.ctx, self.command, INVOKE, &payload, &mut self.state)
            .await
            .unwrap();
    }
    fn feedback(&self) -> &str {
        self.state["feedback"].as_str().unwrap_or("")
    }
    fn calls(&self) -> Vec<(String, Vec<Value>)> {
        self.calls.lock().unwrap().clone()
    }
}
#[tokio::test]
async fn original_component_preferences_fields_actions_and_callbacks() {
    let oracle: Value = serde_json::from_str(include_str!("../fixtures/oracle.json")).unwrap();
    for case in oracle["components"].as_array().unwrap() {
        let command = match case["command"].as_str().unwrap() {
            "form" => commands()[0],
            "clipboard" => commands()[1],
            _ => commands()[2],
        };
        let mut h = Harness::new(command);
        h.run(case["prefs"].clone()).await;
        assert!(h.calls().is_empty());
        let fields = h.view["root"]["children"].as_array().unwrap();
        for original in case["initial"].as_array().unwrap() {
            let native = fields
                .iter()
                .find(|f| {
                    f["props"]["id"]
                        .as_str()
                        .unwrap_or("")
                        .split(':')
                        .next()
                        .unwrap()
                        == original["id"].as_str().unwrap()
                })
                .unwrap();
            assert_eq!(native["type"], original["type"]);
            if !original["defaultValue"].is_null() {
                assert_eq!(native["props"]["value"], original["defaultValue"]);
            }
            for key in ["placeholder", "label", "title"] {
                if !original[key].is_null() {
                    assert_eq!(native["props"][key], original[key]);
                }
            }
        }
        assert_eq!(h.view["actions"].as_array().unwrap().len(), 1);
        h.submit("0", false, "a b c").await;
        assert!(h.calls().is_empty());
        assert!(h.feedback().starts_with("Invalid width"));
        if command != commands()[0] {
            h.replies(vec![Ok(json!("a b c"))]);
        }
        h.submit("3", false, "a b c").await;
        assert_eq!(h.state["output"], "a b\nc");
        assert_eq!(h.view["actions"][1]["title"], "Copy Rewrapped Text");
        assert_eq!(
            h.view["actions"][1]["shortcut"],
            json!({"modifiers":["cmd"],"key":"c"})
        );
        assert!(!h.calls().iter().any(|c| c.0 == "clipboard:write"));
        if command != commands()[0] {
            h.replies(vec![Ok(json!("a b c"))]);
        }
        h.replies(vec![Ok(json!({"ok":true}))]);
        h.submit("", true, "a b c").await;
        assert!(h.feedback().starts_with("Text rewrapped and copied!"));
        assert_eq!(
            h.calls().last().unwrap(),
            &("clipboard:write".into(), vec![json!("a b c")])
        );
        assert!(!h.ctx.paths.data_dir.exists());
        assert!(!h.ctx.paths.config_dir.exists());
    }
}
#[tokio::test]
async fn every_copy_path_rejects_malformed_failure_and_transport_replies() {
    let bad = vec![
        Ok(json!({"ok":false})),
        Ok(json!({"ok":true,"success":false})),
        Ok(json!({"ok":true,"error":"denied"})),
        Ok(json!({"ok":true,"error":false})),
        Ok(json!({"ok":true,"success":null})),
        Ok(json!({"ok":true,"success":"true"})),
        Ok(json!({"success":true})),
        Ok(json!(true)),
        Ok(Value::Null),
        Ok(json!([])),
        Ok(json!("ok")),
        Err("transport".into()),
    ];
    for reply in bad {
        for command in commands() {
            let mut h = Harness::new(command);
            h.run(json!({})).await;
            if *command != commands()[0] {
                h.replies(vec![Ok(json!("😀 a b"))]);
            }
            h.replies(vec![reply.clone()]);
            h.submit("4", true, "😀 a b").await;
            assert!(h.feedback().starts_with("Unable to copy"), "{reply:?}");
            assert_eq!(h.state["output"], "😀 a\nb");
            assert_eq!(
                h.state["draft"],
                json!({"text":if *command==commands()[0]{"😀 a b"}else{""},"width":"4","copyResultToClipboard":true})
            );
            let before = h.calls().len();
            h.replies(vec![reply.clone()]);
            h.copy().await;
            assert!(h.feedback().starts_with("Unable to copy"));
            assert_eq!(h.calls().len(), before + 1);
            h.replies(vec![reply.clone()]);
            h.submit("4", true, "😀 a b").await;
            assert!(h.feedback().starts_with("Unable to copy"));
            assert_eq!(h.calls().len(), before + 2);
            h.replies(vec![Ok(json!({"ok":true,"error":null,"success":true}))]);
            h.submit("4", true, "😀 a b").await;
            assert!(h.feedback().starts_with("Text rewrapped and copied!"));
            assert_eq!(h.calls().len(), before + 3);
            for (route, args) in h.calls().iter().filter(|c| c.0 == "clipboard:write") {
                assert_eq!(route, "clipboard:write");
                assert_eq!(args, &vec![json!("😀 a\nb")]);
            }
        }
    }
}
#[tokio::test]
async fn selection_and_clipboard_errors_preserve_drafts_output_and_retry() {
    for command in &commands()[1..] {
        for reply in [
            Err("transport".into()),
            Ok(json!(null)),
            Ok(json!({"text":"no"})),
            Ok(json!(42)),
            Ok(json!("")),
        ] {
            let mut h = Harness::new(command);
            h.run(json!({})).await;
            h.replies(vec![Ok(json!("old output"))]);
            h.submit("80", false, "").await;
            h.replies(vec![reply.clone()]);
            h.submit("5", false, "new draft").await;
            assert_eq!(h.state["output"], "old output");
            assert_eq!(h.state["draft"]["width"], "5");
            assert!(!h.feedback().starts_with("Text rewrapped"));
            if *command == commands()[2] {
                assert_eq!(h.feedback(),"Unable to retrieve selection — Unable to retrieve highlighted selection from current app");
            }
            if *command == commands()[1] && reply == Ok(json!("")) {
                assert_eq!(
                    h.feedback(),
                    "No text in clipboard — Please copy some text first"
                );
            }
            for (route, args) in h.calls() {
                assert_eq!(
                    route,
                    if *command == commands()[1] {
                        "clipboard:read"
                    } else {
                        "core:platform:selected-text"
                    }
                );
                assert!(args.is_empty());
            }
            h.replies(vec![Ok(json!("new output"))]);
            h.submit("5", false, "new draft").await;
            assert_eq!(h.state["output"], "new\noutput");
        }
    }
}
#[tokio::test]
async fn empty_outputs_infinity_and_copy_false() {
    for command in commands() {
        let mut h = Harness::new(command);
        h.run(json!({})).await;
        if *command != commands()[0] {
            h.replies(vec![Ok(json!(" \n \t"))]);
        }
        h.submit(&"9".repeat(400), false, " \n \t").await;
        assert_eq!(h.state["output"], "");
        assert_eq!(h.view["actions"].as_array().unwrap().len(), 1);
        assert!(h.feedback().contains("Infinity"));
        assert!(!h.view["root"]["children"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["props"]["title"] == "Rewrapped Text"));
        if *command != commands()[0] {
            h.replies(vec![Ok(json!(" \n \t"))]);
        }
        h.replies(vec![Err("copy fail".into()), Ok(json!({"ok":true}))]);
        h.submit("", true, " \n \t").await;
        let before = h.calls().len();
        h.submit("", true, " \n \t").await;
        assert_eq!(h.calls().len(), before + 1);
        assert!(h.feedback().starts_with("Text rewrapped and copied!"));
    }
}
#[tokio::test]
async fn lifecycle_tokens_command_and_payload_validation_precede_effects() {
    let mut h = Harness::new(commands()[0]);
    h.run(json!({})).await;
    let old = h.view["actions"][0]["id"].clone();
    h.submit("2", false, "a b c").await;
    assert!(handle(&h.ctx,h.command,INVOKE,&json!({"actionId":old,"formValues":{"text":"forged","width":"1","copyResultToClipboard":true}}),&mut h.state).await.is_err());
    let current = h.view["actions"][1]["id"].clone();
    for cmd in ["unknown", commands()[1]] {
        assert!(handle(
            &h.ctx,
            cmd,
            INVOKE,
            &json!({"actionId":current}),
            &mut h.state
        )
        .await
        .is_err());
    }
    let before = h.state.clone();
    assert!(handle(&h.ctx,h.command,INVOKE,&json!({"actionId":h.view["actions"][0]["id"],"formValues":{"width":2,"copyResultToClipboard":true}}),&mut h.state).await.is_err());
    assert_eq!(h.state, before);
    handle(
        &h.ctx,
        h.command,
        "extension:dispose-session",
        &json!({}),
        &mut h.state,
    )
    .await
    .unwrap();
    assert_eq!(h.state, Value::Null);
    assert!(handle(
        &h.ctx,
        h.command,
        INVOKE,
        &json!({"actionId":current}),
        &mut h.state
    )
    .await
    .is_err());
    h.run(json!({})).await;
    assert!(handle(
        &h.ctx,
        h.command,
        INVOKE,
        &json!({"actionId":current}),
        &mut h.state
    )
    .await
    .is_err());
    assert!(h.calls().is_empty());
}
#[tokio::test]
async fn validation_refresh_keeps_server_defaults_stable_for_queued_client_drafts() {
    let mut h = Harness::new(commands()[0]);
    h.run(json!({"defaultWidth":"12","defaultCopyToClipboard":false}))
        .await;
    let fields = h.view["root"]["children"].clone();
    h.submit("junk", true, "queued\ntext").await;
    let invalid = h.view["root"]["children"].as_array().unwrap();
    for (index, f) in fields.as_array().unwrap().iter().enumerate() {
        assert_eq!(invalid[index]["props"]["id"], f["props"]["id"]);
        assert_eq!(invalid[index]["props"]["value"], f["props"]["value"]);
        assert!(invalid[index]["props"]["actionId"].is_null());
    }
    assert!(invalid[1]["props"]["error"]
        .as_str()
        .unwrap()
        .contains("valid number"));
    let refresh = handle(
        &h.ctx,
        h.command,
        "extension:refresh-session",
        &json!({}),
        &mut h.state,
    )
    .await
    .unwrap();
    assert_eq!(refresh, h.view);
    assert!(h.calls().is_empty());
}
