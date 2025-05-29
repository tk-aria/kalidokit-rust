//! Example Extism WASM plugin for dynplug.
//!
//! Build with: `cargo build -p dynplug-example-wasm --target wasm32-unknown-unknown`

use extism_pdk::*;

#[plugin_fn]
pub fn greet(name: String) -> FnResult<String> {
    Ok(format!("Hello, {name}!"))
}

#[plugin_fn]
pub fn add(input: Vec<u8>) -> FnResult<Vec<u8>> {
    if input.len() != 8 {
        return Err(extism_pdk::Error::msg(format!(
            "expected 8 bytes, got {}",
            input.len()
        ))
        .into());
    }
    let a = i32::from_le_bytes(input[..4].try_into().unwrap());
    let b = i32::from_le_bytes(input[4..8].try_into().unwrap());
    Ok((a + b).to_le_bytes().to_vec())
}

#[plugin_fn]
pub fn noop(_input: Vec<u8>) -> FnResult<Vec<u8>> {
    Ok(Vec::new())
}
