// SPDX-License-Identifier: MIT
use serde_json::{json, Value};
use tezbar_text_rewrap::native_core::{
    common::{CoreContext, CorePaths},
    extensions::v2_text_rewrap::handle,
};
#[tokio::test]
async fn native_views_match_frozen_renderer_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let p = temp.path();
    let ctx = CoreContext::new(
        CorePaths {
            data_dir: p.join("data"),
            config_dir: p.join("config"),
            resource_dir: p.join("r"),
            temp_dir: p.join("t"),
            home_dir: p.join("h"),
        },
        None,
    );
    fn values(view: &Value, width: &str) -> Value {
        let mut out = serde_json::Map::new();
        for field in view["root"]["children"].as_array().unwrap() {
            if let Some(id) = field["props"]["id"].as_str() {
                out.insert(
                    id.into(),
                    match id.split(':').next().unwrap() {
                        "text" => json!("a b c"),
                        "width" => json!(width),
                        _ => json!(false),
                    },
                );
            }
        }
        json!(out)
    }
    let mut views = Vec::new();
    let mut state = Value::Null;
    for command in [
        "text-rewrap-form",
        "text-rewrap-clipboard",
        "text-rewrap-selection",
    ] {
        let launch = handle(
            &ctx,
            command,
            "extension:run-command",
            &json!({}),
            &mut state,
        )
        .await
        .unwrap();
        views.push(launch.clone());
        let error = handle(
            &ctx,
            command,
            "extension:invoke-action",
            &json!({"actionId":launch["actions"][0]["id"],"formValues":values(&launch,"junk")}),
            &mut state,
        )
        .await
        .unwrap();
        views.push(error.clone());
        if command == "text-rewrap-form" {
            views.push(
                handle(
                    &ctx,
                    command,
                    "extension:invoke-action",
                    &json!({"actionId":error["actions"][0]["id"],"formValues":values(&error,"3")}),
                    &mut state,
                )
                .await
                .unwrap(),
            );
        }
    }
    // Epochs intentionally vary per process/session; normalize only IDs for fixture comparison.
    for view in &mut views {
        for field in view["root"]["children"].as_array_mut().unwrap() {
            if let Some(id) = field["props"]["id"].as_str() {
                field["props"]["id"] = json!(format!("{}:EPOCH", id.split(':').next().unwrap()));
            }
        }
        for action in view["actions"].as_array_mut().unwrap() {
            let id = action["id"].as_str().unwrap();
            let parts: Vec<_> = id.split(':').collect();
            action["id"] = json!(format!("text-rewrap:EPOCH:{}:{}", parts[2], parts[3]));
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/native-views.json");
    if std::env::var_os("UPDATE_NATIVE_VIEWS").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&views).unwrap() + "\n").unwrap();
    }
    let expected: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(json!(views), expected);
}
