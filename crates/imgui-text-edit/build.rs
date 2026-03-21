use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=third-party/ImGuiColorTextEdit/TextEditor.h");
    println!("cargo:rerun-if-changed=third-party/ImGuiColorTextEdit/TextEditor.cpp");
    println!("cargo:rerun-if-changed=third-party/ImGuiColorTextEdit/text_editor_capi.h");
    println!("cargo:rerun-if-changed=third-party/ImGuiColorTextEdit/text_editor_capi.cpp");

    // Resolve ImGui include path from dear-imgui-sys (exported via links = "dear-imgui")
    let imgui_include = env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH")
        .map(PathBuf::from)
        .expect("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH not set — dear-imgui-sys must be a dependency");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let te_dir = manifest_dir.join("third-party/ImGuiColorTextEdit");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(&imgui_include)
        .include(&te_dir)
        .define("IMGUI_USE_WCHAR32", None)
        .file(te_dir.join("TextEditor.cpp"))
        .file(te_dir.join("text_editor_capi.cpp"));

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        build.flag("/EHsc");
    }

    build.compile("imgui_text_edit");
}
