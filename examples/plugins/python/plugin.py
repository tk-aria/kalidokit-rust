"""
Extism Python plugin example for dynplug.

Build:
    extism-py plugin.py -o plugin.wasm

Test:
    cargo run -p dynplug --features wasm --example host_wasm_polyglot
"""

import extism


@extism.plugin_fn
def greet():
    name = extism.input_str()
    extism.output_str(f"Hello from Python, {name}!")


@extism.plugin_fn
def add():
    data = extism.input_bytes()
    if len(data) != 8:
        raise ValueError(f"expected 8 bytes, got {len(data)}")
    a = int.from_bytes(data[0:4], byteorder="little", signed=True)
    b = int.from_bytes(data[4:8], byteorder="little", signed=True)
    result = (a + b).to_bytes(4, byteorder="little", signed=True)
    extism.output_bytes(result)


@extism.plugin_fn
def language():
    extism.output_str("python")
