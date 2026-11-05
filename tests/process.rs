// SPDX-License-Identifier: MIT
//! Real-pipe tests against the compiled executable. The "host" here is a
//! recording stub: it answers typed reverse RPCs with canned values and never
//! touches the OS clipboard, selection, focus or any GUI.
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const ID: &str = "com.tezbar.native.text-rewrap";

struct Host {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next: u64,
    /// Every reverse request the extension sent, in order.
    calls: Vec<Value>,
}

impl Host {
    async fn start(grants: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tezbar-text-rewrap"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn extension");
        let stdin = child.stdin.take().unwrap();
        let lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut host = Host {
            child,
            stdin,
            lines,
            next: 0,
            calls: Vec::new(),
        };
        let hello = host.frame().await;
        assert_eq!(hello["type"], "hello");
        assert_eq!(hello["id"], ID);
        assert_eq!(hello["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(hello["protocol"], 1);
        let temp = std::env::temp_dir();
        let dir = temp.to_string_lossy();
        let init = host
            .call(
                "initialize",
                json!({"protocol":1,"extensionId":ID,"platform":"aarch64-apple-darwin",
                    "packageDir":dir,"dataDir":dir,"tempDir":dir,
                    "grantedCapabilities":grants,"reason":"command","migrationPaths":{}}),
                &mut |_| panic!("initialize must not call the host"),
            )
            .await
            .unwrap();
        assert_eq!(init["ready"], true);
        host
    }
    async fn frame(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(10), self.lines.next_line())
            .await
            .expect("frame timeout")
            .unwrap()
            .expect("unexpected EOF");
        serde_json::from_str(&line).unwrap()
    }
    async fn send(&mut self, value: Value) {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await.unwrap();
        self.stdin.flush().await.unwrap();
    }
    fn id(&mut self) -> String {
        self.next += 1;
        format!("h:00000000-0000-4000-8000-{:012}", self.next)
    }
    async fn request(&mut self, method: &str, params: Value) -> String {
        let id = self.id();
        self.send(json!({"type":"request","id":id,"method":method,"params":params}))
            .await;
        id
    }
    /// Sends a request and serves reverse RPCs with `reply` until its response.
    async fn call(
        &mut self,
        method: &str,
        params: Value,
        reply: &mut dyn FnMut(&Value) -> Option<Result<Value, String>>,
    ) -> Result<Value, String> {
        let id = self.request(method, params).await;
        self.await_response(&id, reply).await
    }
    async fn await_response(
        &mut self,
        id: &str,
        reply: &mut dyn FnMut(&Value) -> Option<Result<Value, String>>,
    ) -> Result<Value, String> {
        loop {
            let frame = self.frame().await;
            match frame["type"].as_str() {
                Some("response") if frame["id"] == id => {
                    return match frame.get("error") {
                        Some(error) => Err(error["message"].as_str().unwrap().to_owned()),
                        None => Ok(frame["result"].clone()),
                    }
                }
                Some("request") => {
                    self.calls.push(frame.clone());
                    // None = leave the reverse request pending (cancellation tests).
                    if let Some(answer) = reply(&frame) {
                        let rid = frame["id"].clone();
                        let response = match answer {
                            Ok(result) => json!({"type":"response","id":rid,"result":result}),
                            Err(message) => json!({"type":"response","id":rid,
                                "error":{"code":"host-error","message":message}}),
                        };
                        self.send(response).await;
                    }
                }
                Some("event") | Some("cancel") => {}
                other => panic!("unexpected frame {other:?}: {frame}"),
            }
        }
    }
    async fn shutdown(mut self) {
        let result = self.call("shutdown", json!({}), &mut |_| None).await;
        assert!(result.is_ok(), "{result:?}");
        let status = tokio::time::timeout(Duration::from_secs(5), self.child.wait())
            .await
            .expect("exit timeout")
            .unwrap();
        assert!(status.success());
    }
}

fn no_host(frame: &Value) -> Option<Result<Value, String>> {
    panic!("unexpected reverse RPC {frame}")
}

fn field<'a>(view: &'a Value, title: &str) -> Option<&'a Value> {
    view["root"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["props"]["title"] == title)
}
fn field_id(view: &Value, title: &str) -> String {
    field(view, title).unwrap()["props"]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn action(view: &Value, title: &str) -> Option<String> {
    view["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == title)
        .map(|a| a["id"].as_str().unwrap().to_owned())
}
fn text(view: &Value, title: &str) -> Option<String> {
    field(view, title).map(|f| {
        let key = if f["type"] == "Form.TextArea" {
            "value"
        } else {
            "text"
        };
        f["props"][key].as_str().unwrap().to_owned()
    })
}
fn submit(view: &Value, values: Value) -> Value {
    json!({"actionId":action(view,"Rewrap Text").unwrap(),"formValues":values})
}

const GRANTS: &[&str] = &[
    "host.clipboard.read",
    "host.clipboard.write",
    "host.selection.read",
];

#[tokio::test]
async fn protocol_info_matches_manifest() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_tezbar-text-rewrap"))
        .arg("--protocol-info")
        .output()
        .unwrap();
    let info: Value = serde_json::from_slice(&out.stdout).unwrap();
    let manifest: Value = serde_json::from_str(include_str!("../extension.json")).unwrap();
    assert_eq!(info["id"], manifest["id"]);
    assert_eq!(info["version"], manifest["version"]);
    assert_eq!(info["protocol"], 1);
    let names: Vec<_> = manifest["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].clone())
        .collect();
    assert_eq!(info["commands"], json!(names));
}

#[tokio::test]
async fn form_rewraps_and_copies_with_typed_host_write() {
    let mut host = Host::start(GRANTS).await;
    let view = host
        .call(
            "session.open",
            json!({"sessionId":"s1","commandName":"text-rewrap-form",
                "preferences":{"defaultWidth":"10","defaultCopyToClipboard":true},"argumentValues":{}}),
            &mut no_host,
        )
        .await
        .unwrap();
    assert_eq!(view["mode"], "view");
    assert_eq!(view["title"], "Text Rewrap from Form");
    let width = field(&view, "Width").unwrap();
    assert_eq!(width["props"]["value"], "10");
    let values = json!({
        field_id(&view,"Text"): "the quick brown fox jumps",
        field_id(&view,"Width"): "10",
        field_id(&view,""): true,
    });
    let after = host
        .call(
            "session.call",
            json!({"sessionId":"s1","operation":"extension:invoke-action","payload":submit(&view,values)}),
            &mut |f| {
                assert_eq!(f["method"], "host.clipboard.write");
                Some(Ok(json!({"ok":true})))
            },
        )
        .await
        .unwrap();
    assert_eq!(host.calls.len(), 1);
    assert_eq!(
        host.calls[0]["params"],
        json!({"text":"the quick\nbrown fox\njumps"})
    );
    assert_eq!(
        text(&after, "Rewrapped Text").unwrap(),
        "the quick\nbrown fox\njumps"
    );
    assert_eq!(
        text(&after, "Status").unwrap(),
        "Text rewrapped and copied! — Text re-wrapped at 10 characters"
    );
    // Old action IDs rotate on accepted submit and are rejected before effects.
    let stale = host
        .call(
            "session.call",
            json!({"sessionId":"s1","operation":"extension:invoke-action","payload":submit(&view,json!({}))}),
            &mut no_host,
        )
        .await;
    assert_eq!(
        stale.unwrap_err(),
        "Stale or unavailable Text Rewrap action"
    );
    // Explicit Copy action uses the stored result.
    let copy = action(&after, "Copy Rewrapped Text").unwrap();
    host.calls.clear();
    host.call(
        "session.call",
        json!({"sessionId":"s1","operation":"extension:invoke-action","payload":{"actionId":copy}}),
        &mut |_| Some(Ok(json!({"ok":true}))),
    )
    .await
    .unwrap();
    assert_eq!(host.calls[0]["method"], "host.clipboard.write");
    host.call("session.dispose", json!({"sessionId":"s1"}), &mut no_host)
        .await
        .unwrap();
    host.shutdown().await;
}

#[tokio::test]
async fn clipboard_command_reads_then_reports_failed_write_and_retries() {
    let mut host = Host::start(GRANTS).await;
    let view = host
        .call(
            "session.open",
            json!({"sessionId":"c","commandName":"text-rewrap-clipboard","preferences":{},"argumentValues":{}}),
            &mut no_host,
        )
        .await
        .unwrap();
    assert!(field(&view, "Text").is_none());
    assert_eq!(field(&view, "Width").unwrap()["props"]["value"], "80");
    let values = json!({field_id(&view,"Width"):"", field_id(&view,""):true});
    // Invalid width: no reverse RPC, field error.
    let bad = host
        .call(
            "session.call",
            json!({"sessionId":"c","operation":"extension:invoke-action",
                "payload":submit(&view,json!({field_id(&view,"Width"):"0", field_id(&view,""):true}))}),
            &mut no_host,
        )
        .await
        .unwrap();
    assert_eq!(
        field(&bad, "Width").unwrap()["props"]["error"],
        "Width must be a positive number (or leave blank for no text wrapping)"
    );
    // Read succeeds, write reports failure via malformed success envelope.
    let failed = host
        .call(
            "session.call",
            json!({"sessionId":"c","operation":"extension:invoke-action","payload":submit(&bad,values.clone())}),
            &mut |f| match f["method"].as_str().unwrap() {
                "host.clipboard.read" => Some(Ok(json!("a\nb\n\n  c   d"))),
                "host.clipboard.write" => Some(Ok(json!({"ok":true,"success":false}))),
                m => panic!("{m}"),
            },
        )
        .await
        .unwrap();
    assert_eq!(host.calls[0]["params"], json!({}));
    assert_eq!(text(&failed, "Rewrapped Text").unwrap(), "a b\n\nc d");
    assert!(text(&failed, "Status").unwrap().starts_with(
        "Unable to copy result to clipboard. Result preserved. Submit the same inputs"
    ));
    // Same-input submit retries the stored write without re-reading the clipboard.
    host.calls.clear();
    let retried = host
        .call(
            "session.call",
            json!({"sessionId":"c","operation":"extension:invoke-action","payload":submit(&failed,values)}),
            &mut |f| {
                assert_eq!(f["method"], "host.clipboard.write");
                Some(Ok(json!({"ok":true})))
            },
        )
        .await
        .unwrap();
    assert_eq!(host.calls.len(), 1);
    assert_eq!(
        text(&retried, "Status").unwrap(),
        "Text rewrapped and copied! — No text wrapping applied"
    );
    host.shutdown().await;
}

#[tokio::test]
async fn selection_uses_typed_selection_read_and_reports_host_errors() {
    let mut host = Host::start(GRANTS).await;
    let view = host
        .call(
            "session.open",
            json!({"sessionId":"sel","commandName":"text-rewrap-selection",
                "preferences":{"defaultCopyToClipboard":false},"argumentValues":{}}),
            &mut no_host,
        )
        .await
        .unwrap();
    assert_eq!(field(&view, "").unwrap()["props"]["value"], false);
    let values = json!({field_id(&view,"Width"):"5", field_id(&view,""):false});
    let failed = host
        .call(
            "session.call",
            json!({"sessionId":"sel","operation":"extension:invoke-action","payload":submit(&view,values.clone())}),
            &mut |_| Some(Err("Accessibility permission denied".into())),
        )
        .await
        .unwrap();
    assert_eq!(host.calls[0]["method"], "host.selection.read");
    assert_eq!(
        text(&failed, "Status").unwrap(),
        "Unable to retrieve selection — Unable to retrieve highlighted selection from current app"
    );
    assert!(text(&failed, "Rewrapped Text").is_none());
    let ok = host
        .call(
            "session.call",
            json!({"sessionId":"sel","operation":"extension:invoke-action","payload":submit(&failed,values)}),
            &mut |_| Some(Ok(json!("one two three"))),
        )
        .await
        .unwrap();
    // Unchecked: never writes automatically.
    assert_eq!(host.calls.len(), 2);
    assert_eq!(text(&ok, "Rewrapped Text").unwrap(), "one\ntwo\nthree");
    assert_eq!(
        text(&ok, "Status").unwrap(),
        "Text rewrapped! — Text re-wrapped at 5 characters"
    );
    host.shutdown().await;
}

#[tokio::test]
async fn ungranted_capability_fails_closed_without_reverse_rpc() {
    let mut host = Host::start(&["host.clipboard.write"]).await;
    let view = host
        .call(
            "session.open",
            json!({"sessionId":"c","commandName":"text-rewrap-clipboard","preferences":{},"argumentValues":{}}),
            &mut no_host,
        )
        .await
        .unwrap();
    let values = json!({field_id(&view,"Width"):"80", field_id(&view,""):true});
    let result = host
        .call(
            "session.call",
            json!({"sessionId":"c","operation":"extension:invoke-action","payload":submit(&view,values)}),
            &mut no_host,
        )
        .await
        .unwrap();
    assert_eq!(
        text(&result, "Status").unwrap(),
        "Unable to read clipboard — Please try again"
    );
    host.shutdown().await;
}

#[tokio::test]
async fn cancelled_action_does_not_commit_state() {
    let mut host = Host::start(GRANTS).await;
    let view = host
        .call(
            "session.open",
            json!({"sessionId":"c","commandName":"text-rewrap-clipboard","preferences":{},"argumentValues":{}}),
            &mut no_host,
        )
        .await
        .unwrap();
    let values = json!({field_id(&view,"Width"):"80", field_id(&view,""):false});
    let id = host
        .request(
            "session.call",
            json!({"sessionId":"c","operation":"extension:invoke-action","payload":submit(&view,values)}),
        )
        .await;
    // Wait for the pending clipboard read, then cancel the host request.
    let pending = host.frame().await;
    assert_eq!(pending["method"], "host.clipboard.read");
    host.send(json!({"type":"cancel","requestId":id})).await;
    // The read result arrives late; the call must still resolve as cancelled.
    let mut saw_cancel_of_reverse = false;
    let result = loop {
        let frame = host.frame().await;
        match frame["type"].as_str() {
            Some("cancel") => {
                assert_eq!(frame["requestId"], pending["id"]);
                saw_cancel_of_reverse = true;
            }
            Some("response") if frame["id"] == id => break frame,
            _ => {}
        }
    };
    assert!(result.get("error").is_some(), "{result}");
    assert!(
        saw_cancel_of_reverse,
        "dropped reverse RPC must be cancelled"
    );
    // Refresh shows the pre-action state: same (still valid) action IDs, no output.
    let refreshed = host
        .call(
            "session.call",
            json!({"sessionId":"c","operation":"extension:refresh-session","payload":{}}),
            &mut no_host,
        )
        .await
        .unwrap();
    assert_eq!(
        action(&refreshed, "Rewrap Text"),
        action(&view, "Rewrap Text")
    );
    assert!(text(&refreshed, "Rewrapped Text").is_none());
    host.shutdown().await;
}

#[tokio::test]
async fn unknown_command_is_rejected() {
    let mut host = Host::start(GRANTS).await;
    let result = host
        .call(
            "session.open",
            json!({"sessionId":"x","commandName":"index","preferences":{},"argumentValues":{}}),
            &mut no_host,
        )
        .await;
    assert!(result.is_err());
    host.shutdown().await;
}

#[test]
fn narrow_host_mapping() {
    use text_rewrap_process::native_core::common::host_method;
    assert_eq!(
        host_method("clipboard:write", &[json!("x")]).unwrap(),
        ("host.clipboard.write", json!({"text":"x"}))
    );
    assert_eq!(
        host_method("clipboard:read", &[]).unwrap(),
        ("host.clipboard.read", json!({}))
    );
    assert_eq!(
        host_method("core:platform:selected-text", &[]).unwrap(),
        ("host.selection.read", json!({}))
    );
    assert!(host_method("shell:open", &[json!("https://x")]).is_err());
    assert!(host_method("clipboard:write", &[json!(1)]).is_err());
}

#[tokio::test]
async fn saved_checkbox_encodings_are_respected_for_every_command() {
    let mut host = Host::start(GRANTS).await;
    for command in [
        "text-rewrap-form",
        "text-rewrap-clipboard",
        "text-rewrap-selection",
    ] {
        for (encoded, expected) in [
            (json!("false"), false),
            (json!(false), false),
            (json!("true"), true),
            (json!(true), true),
            (json!(null), true),
            (json!("invalid"), true),
        ] {
            let view = host
                .call(
                    "session.open",
                    json!({
                        "sessionId":"prefs", "commandName":command,
                        "preferences":{"defaultCopyToClipboard":encoded}, "argumentValues":{}
                    }),
                    &mut no_host,
                )
                .await
                .unwrap();
            assert_eq!(
                field(&view, "").unwrap()["props"]["value"],
                expected,
                "{command} {encoded}"
            );
            let mut values = json!({field_id(&view,"Width"):"5", field_id(&view,""):expected});
            if command == "text-rewrap-form" {
                values[field_id(&view, "Text")] = json!("one two three\n\nfour five");
            }
            host.calls.clear();
            let result = host.call("session.call", json!({"sessionId":"prefs", "operation":"extension:invoke-action", "payload":submit(&view, values)}), &mut |f| {
                Some(Ok(match f["method"].as_str().unwrap() {
                    "host.clipboard.read" | "host.selection.read" => json!("one two three\n\nfour five"),
                    "host.clipboard.write" => json!({"ok":true}),
                    m => panic!("unexpected host method {m}"),
                }))
            }).await.unwrap();
            let output = field(&result, "Rewrapped Text").unwrap();
            assert_eq!(output["type"], "Form.TextArea");
            assert_eq!(output["props"]["value"], "one\ntwo\nthree\n\nfour\nfive");
            assert_eq!(
                host.calls
                    .iter()
                    .filter(|f| f["method"] == "host.clipboard.write")
                    .count(),
                usize::from(expected)
            );
            assert_eq!(result["effects"][0]["style"], "success");
            host.call(
                "session.dispose",
                json!({"sessionId":"prefs"}),
                &mut no_host,
            )
            .await
            .unwrap();
        }
    }
    host.shutdown().await;
}

#[tokio::test]
async fn failed_explicit_copy_emits_failure_and_keeps_stored_output_for_retry() {
    let mut host = Host::start(GRANTS).await;
    let view = host.call("session.open", json!({"sessionId":"copy", "commandName":"text-rewrap-form", "preferences":{"defaultCopyToClipboard":"false"}, "argumentValues":{}}), &mut no_host).await.unwrap();
    let result = host.call("session.call", json!({"sessionId":"copy", "operation":"extension:invoke-action", "payload":submit(&view, json!({
        field_id(&view,"Text"):"one two three\n\nfour five", field_id(&view,"Width"):"5", field_id(&view,""):false
    }))}), &mut no_host).await.unwrap();
    let copy = action(&result, "Copy Rewrapped Text").unwrap();
    let failed = host.call("session.call", json!({"sessionId":"copy", "operation":"extension:invoke-action", "payload":{"actionId":copy}}), &mut |f| {
        assert_eq!(f["method"], "host.clipboard.write");
        assert_eq!(f["params"]["text"], "one\ntwo\nthree\n\nfour\nfive");
        Some(Err("OS clipboard write denied".into()))
    }).await.unwrap();
    assert_eq!(failed["mode"], "view");
    assert_eq!(failed["effects"][0]["kind"], "toast");
    assert_eq!(failed["effects"][0]["style"], "failure");
    assert!(failed["effects"][0]["title"]
        .as_str()
        .unwrap()
        .starts_with("Unable to copy"));
    assert_eq!(
        text(&failed, "Rewrapped Text"),
        text(&result, "Rewrapped Text")
    );
    assert_eq!(action(&failed, "Copy Rewrapped Text").unwrap(), copy);
    let retry = host.call("session.call", json!({"sessionId":"copy", "operation":"extension:invoke-action", "payload":{"actionId":copy}}), &mut |f| {
        assert_eq!(f["method"], "host.clipboard.write");
        assert_eq!(f["params"]["text"], "one\ntwo\nthree\n\nfour\nfive");
        Some(Ok(json!({"ok":true})))
    }).await.unwrap();
    assert_eq!(retry["effects"][0]["style"], "success");
    host.shutdown().await;
}
