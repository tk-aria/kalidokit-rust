//! Sample dynplug plugin demonstrating export_plugin! macro usage.

dynplug::export_plugin! {
    name: "greeter",
    version: 1,
    invoke: handle_invoke,
}

fn handle_invoke(method: &str, input: &[u8]) -> Result<Vec<u8>, String> {
    match method {
        "greet" => {
            let name = std::str::from_utf8(input).unwrap_or("world");
            Ok(format!("Hello, {name}!").into_bytes())
        }
        "add" if input.len() == 8 => {
            let a = i32::from_le_bytes(input[..4].try_into().unwrap());
            let b = i32::from_le_bytes(input[4..8].try_into().unwrap());
            Ok((a + b).to_le_bytes().to_vec())
        }
        "noop" => Ok(Vec::new()),
        "panic_test" => {
            panic!("intentional panic for testing");
        }
        _ => Err(format!("unknown method: {method}")),
    }
}
