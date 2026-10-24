// SPDX-License-Identifier: MIT
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn read(path: &str) -> Vec<u8> {
    std::fs::read(root().join(path)).unwrap()
}
fn json(path: &str) -> Value {
    serde_json::from_slice(&read(path)).unwrap()
}
#[test]
fn all_byte_exact_snapshots_icons_and_licenses_are_in_source_package() {
    for file in [
        "fixtures/provenance.json",
        "fixtures/original-provenance.json",
    ] {
        for (path, p) in json(file).as_object().unwrap() {
            assert_eq!(
                format!("{:x}", Sha256::digest(read(path))),
                p["sha256"],
                "{path}"
            );
        }
    }
    for (name, p) in json("fixtures/icons.json").as_object().unwrap() {
        let bytes = read(&format!("assets/{name}"));
        assert_eq!(bytes.len() as u64, p["bytes"].as_u64().unwrap());
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), p["sha256"]);
    }
    assert_eq!(
        json("fixtures/icons.json")["text-rewrap.png"]["sha256"],
        "605539b2bad407e75ad6fad90363bdaf7dddf65a8ac2d1f2ea3865d93869523f"
    );
    for (path, needle) in [
        ("LICENSE", "MIT License"),
        ("LICENSE", "rembridge"),
        ("fixtures/beta/LICENSE", "Apache License"),
        ("NOTICES", "Apache-2.0"),
    ] {
        assert!(String::from_utf8(read(path)).unwrap().contains(needle));
    }
    let sdk = String::from_utf8(read("fixtures/beta/v2_sdk.rs")).unwrap();
    assert!(sdk.contains("ctx.request(\"clipboard:write\", vec![json!(text)])"));
    let platform = String::from_utf8(read("fixtures/beta/platform.rs")).unwrap();
    assert!(platform
        .contains("\"core:platform:selected-text\" => selected_text().await.map(Value::String)"));
    let clipboard = String::from_utf8(read("fixtures/beta/clipboard.rs")).unwrap();
    assert!(clipboard.contains("return Ok(json!({\"ok\":true}));"));
}
#[test]
fn manifest_original_commands_preferences_platforms_and_paths() {
    let m = json("extension-v2.json");
    let original = json("fixtures/original/package.json");
    assert_eq!(m["schemaVersion"], 2);
    assert_eq!(m["id"], "com.tezbar.native.text-rewrap");
    assert_eq!(m["runtime"], "rust");
    assert_eq!(m["replaces"], serde_json::json!(["raycast.text-rewrap"]));
    assert!(m.get("author").is_none());
    assert_eq!(m["preferences"], original["preferences"]);
    for (i, c) in m["commands"].as_array().unwrap().iter().enumerate() {
        for key in ["name", "title", "description", "mode", "platforms"] {
            assert_eq!(c[key], original["commands"][i][key]);
        }
        assert_eq!(c["runtime"], "rust");
        let icon = c["icon"].as_str().unwrap();
        assert!(icon.starts_with("/extensions/icons/"));
        assert!(root()
            .join("assets")
            .join(icon.rsplit('/').next().unwrap())
            .is_file());
    }
    assert_eq!(m["commands"][2]["icon"], m["commands"][1]["icon"]);
    assert_eq!(m["verification"].as_array().unwrap().len(), 2);
    for v in m["verification"].as_array().unwrap() {
        assert_eq!(v["state"], "not-run");
    }
    let schema = json("fixtures/beta/extension-v2.schema.json");
    assert_eq!(schema["additionalProperties"], false);
    for key in schema["required"].as_array().unwrap() {
        assert!(m.get(key.as_str().unwrap()).is_some());
    }
    for key in m.as_object().unwrap().keys() {
        assert!(schema["properties"].get(key).is_some());
    }
}
