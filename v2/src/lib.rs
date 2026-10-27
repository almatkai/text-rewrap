// SPDX-License-Identifier: MIT
//! Production engine plus a host-contract harness. See README for compiled registration.
pub mod engine;
// The production adapter is compiled unchanged against byte-exact beta modules.
#[cfg(feature = "beta-contract")]
extern crate self as tauri;
#[cfg(feature = "beta-contract")]
#[derive(Clone)]
pub struct AppHandle;
#[cfg(feature = "beta-contract")]
pub trait Emitter {
    fn emit(&self, channel: &str, payload: serde_json::Value) -> Result<(), String>;
}
#[cfg(feature = "beta-contract")]
impl Emitter for AppHandle {
    fn emit(&self, _: &str, _: serde_json::Value) -> Result<(), String> {
        Err("OS event emission is disabled in the contract harness".into())
    }
}
#[cfg(feature = "beta-contract")]
#[path = "../fixtures/beta/v2_sdk.rs"]
pub mod beta_sdk;
#[cfg(feature = "beta-contract")]
pub mod native_core {
    pub mod common {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/beta/common.rs"
        ));
    }
    pub mod extensions {
        // Path modules preserve the unmodified inner doc comment in beta's SDK.
        pub use crate::beta_sdk as v2_sdk;
        pub mod v2_text_rewrap {
            pub use crate::engine;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/tezbar_adapter.rs"
            ));
        }
    }
}
