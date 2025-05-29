//! Layer 3 (define_plugin!) integration tests.

use std::path::PathBuf;

dynplug::define_plugin! {
    pub struct Calculator {
        fn add(a: i32, b: i32) -> i32;
        fn multiply(a: i32, b: i32) -> i32;
        fn negate(x: i32) -> i32;
    }
}

fn l3_plugin_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join(dynplug::lib_filename("dynplug-example-l3"))
}

#[test]
fn test_define_plugin_load() {
    let path = l3_plugin_path();
    let calc = Calculator::load(&path).expect("failed to load Calculator plugin");
    drop(calc);
}

#[test]
fn test_define_plugin_add() {
    let path = l3_plugin_path();
    let calc = Calculator::load(&path).unwrap();
    assert_eq!(calc.add(21, 21), 42);
}

#[test]
fn test_define_plugin_multiply() {
    let path = l3_plugin_path();
    let calc = Calculator::load(&path).unwrap();
    assert_eq!(calc.multiply(6, 7), 42);
}

#[test]
fn test_define_plugin_negate() {
    let path = l3_plugin_path();
    let calc = Calculator::load(&path).unwrap();
    assert_eq!(calc.negate(42), -42);
    assert_eq!(calc.negate(-1), 1);
}

#[test]
fn test_define_plugin_drop() {
    let path = l3_plugin_path();
    let calc = Calculator::load(&path).unwrap();
    drop(calc); // should not panic
}

#[test]
fn test_define_plugin_load_nonexistent() {
    let result = Calculator::load("/nonexistent/path");
    assert!(result.is_err());
}
