//! LuaRocks path discovery.
//!
//! Finds installed LuaRocks module paths for Lua 5.4 on the current system.

use std::path::PathBuf;
use std::process::Command;

/// Discover LuaRocks Lua and C module paths.
///
/// Returns `(lua_path_additions, cpath_additions)` as semicolon-separated
/// Lua path patterns (e.g. `/path/?.lua;/path/?/init.lua`).
pub fn discover_paths() -> (String, String) {
    // Try `luarocks path` command first — most reliable
    if let Some((lp, cp)) = try_luarocks_command() {
        return (lp, cp);
    }

    // Fallback: check well-known directories
    let mut lua_paths = Vec::new();
    let mut c_paths = Vec::new();

    for base in candidate_dirs() {
        let lua_dir = base.join("share/lua/5.4");
        if lua_dir.is_dir() {
            lua_paths.push(format!("{}/?.lua", lua_dir.display()));
            lua_paths.push(format!("{}/?/init.lua", lua_dir.display()));
        }

        let lib_dir = base.join("lib/lua/5.4");
        if lib_dir.is_dir() {
            let ext = if cfg!(target_os = "macos") {
                "so"
            } else if cfg!(target_os = "windows") {
                "dll"
            } else {
                "so"
            };
            c_paths.push(format!("{}/?.{ext}", lib_dir.display()));
        }
    }

    (lua_paths.join(";"), c_paths.join(";"))
}

/// Try running `luarocks path --lua-version 5.4` to get paths.
fn try_luarocks_command() -> Option<(String, String)> {
    let output = Command::new("luarocks")
        .args(["path", "--lua-version", "5.4"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lua_path = String::new();
    let mut c_path = String::new();

    for line in stdout.lines() {
        // Format: export LUA_PATH='...;'
        if let Some(val) = line
            .strip_prefix("export LUA_PATH='")
            .and_then(|s| s.strip_suffix('\''))
        {
            lua_path = val.to_string();
        }
        if let Some(val) = line
            .strip_prefix("export LUA_CPATH='")
            .and_then(|s| s.strip_suffix('\''))
        {
            c_path = val.to_string();
        }
    }

    if lua_path.is_empty() && c_path.is_empty() {
        return None;
    }

    log::debug!("LuaRocks LUA_PATH: {lua_path}");
    log::debug!("LuaRocks LUA_CPATH: {c_path}");
    Some((lua_path, c_path))
}

/// Well-known directories where LuaRocks installs modules.
fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User-local LuaRocks
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".luarocks"));
    }

    // Homebrew (Apple Silicon)
    dirs.push(PathBuf::from("/opt/homebrew"));
    // Homebrew (Intel Mac) / Linux
    dirs.push(PathBuf::from("/usr/local"));
    // System
    dirs.push(PathBuf::from("/usr"));

    dirs
}
