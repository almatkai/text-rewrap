// SPDX-License-Identifier: MIT
use serde_json::Value;
use tezbar_text_rewrap::engine::*;
fn oracle() -> Value {
    serde_json::from_str(include_str!("../fixtures/oracle.json")).unwrap()
}
fn number(s: &str) -> f64 {
    match s {
        "Infinity" => f64::INFINITY,
        "NaN" => f64::NAN,
        _ => s.parse().unwrap(),
    }
}
#[test]
fn original_typescript_width_and_message_oracle() {
    for case in oracle()["validation"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let actual = validate_width(input);
        assert_eq!(
            actual.is_ok(),
            case["valid"].as_bool().unwrap(),
            "{input:?}"
        );
        match actual {
            Err(error) => assert_eq!(error, case["error"].as_str().unwrap(), "{input:?}"),
            Ok(width) => {
                assert_eq!(
                    format_message(width),
                    case["message"].as_str().unwrap(),
                    "{input:?}"
                );
                match width {
                    Width::Unwrap => assert_eq!(case["width"], "unwrap"),
                    Width::Infinity => assert_eq!(case["width"], "Infinity"),
                    Width::Finite(n) => {
                        assert_eq!(n, number(case["width"].as_str().unwrap()), "{input:?}")
                    }
                }
            }
        }
    }
}
#[test]
fn original_typescript_random_unicode_paragraph_oracle() {
    for case in oracle()["engine"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let width = validate_width(case["input"].as_str().unwrap()).unwrap_or(Width::Finite(80.0));
        assert_eq!(
            wrap(text, width),
            case["output"].as_str().unwrap(),
            "{case}"
        );
    }
}
#[test]
fn original_typescript_pure_util_edge_cases() {
    for case in oracle()["pure"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let width = case["width"].as_str().unwrap();
        let width = if width == "undefined" {
            None
        } else {
            Some(number(width))
        };
        assert_eq!(
            rewrap_text(text, width),
            case["output"].as_str().unwrap(),
            "{case}"
        );
        assert_eq!(
            rewrap_paragraphs(text, width.unwrap_or(80.0)),
            case["paragraphs"].as_str().unwrap(),
            "{case}"
        );
    }
}
