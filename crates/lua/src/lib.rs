//! Lua 5.4 runtime with LuaRocks package discovery.
//!
//! Wraps `mlua::Lua` and configures `package.path` / `package.cpath`
//! so that LuaRocks-installed modules are available via `require()`.

mod luarocks;

pub use mlua;

use anyhow::Result;
use mlua::Lua;

/// Lua 5.4 runtime with LuaRocks paths configured.
pub struct LuaRuntime {
    lua: Lua,
}

impl LuaRuntime {
    /// Create a new Lua 5.4 runtime.
    ///
    /// Automatically discovers and appends LuaRocks paths so that
    /// `require("some_rock")` works for installed packages.
    pub fn new() -> Result<Self> {
        let lua = Lua::new();

        // Configure LuaRocks paths
        let (lua_path, c_path) = luarocks::discover_paths();
        if !lua_path.is_empty() || !c_path.is_empty() {
            let code = format!(
                r#"
                package.path  = package.path  .. ";{lua_path}"
                package.cpath = package.cpath .. ";{c_path}"
                "#
            );
            lua.load(&code).exec()?;
            log::info!("LuaRocks paths configured");
        }

        Ok(Self { lua })
    }

    /// Access the inner `mlua::Lua`.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Register a Rust function as a Lua global.
    pub fn register_fn<F, A, R>(&self, name: &str, func: F) -> Result<()>
    where
        F: Fn(&Lua, A) -> mlua::Result<R> + Send + 'static,
        A: mlua::FromLuaMulti,
        R: mlua::IntoLuaMulti,
    {
        let f = self.lua.create_function(func)?;
        self.lua.globals().set(name, f)?;
        Ok(())
    }

    /// Set a global value.
    pub fn set_global<V: mlua::IntoLua>(&self, name: &str, value: V) -> Result<()> {
        self.lua.globals().set(name, value)?;
        Ok(())
    }

    /// Get a global value.
    pub fn get_global<V: mlua::FromLua>(&self, name: &str) -> Result<V> {
        let v = self.lua.globals().get(name)?;
        Ok(v)
    }

    /// Load and execute a Lua script file.
    pub fn exec_file(&self, path: &std::path::Path) -> Result<()> {
        let code = std::fs::read_to_string(path)?;
        self.lua.load(&code).set_name(path.to_string_lossy()).exec()?;
        Ok(())
    }

    /// Execute a Lua string.
    pub fn exec(&self, code: &str) -> Result<()> {
        self.lua.load(code).exec()?;
        Ok(())
    }

    /// Call a named global Lua function.
    pub fn call_global<A, R>(&self, name: &str, args: A) -> Result<R>
    where
        A: mlua::IntoLuaMulti,
        R: mlua::FromLuaMulti,
    {
        let func: mlua::Function = self.lua.globals().get(name)?;
        let result = func.call(args)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_runtime() {
        let rt = LuaRuntime::new().unwrap();
        rt.exec("x = 1 + 2").unwrap();
        let x: i64 = rt.get_global("x").unwrap();
        assert_eq!(x, 3);
    }

    #[test]
    fn register_and_call_fn() {
        let rt = LuaRuntime::new().unwrap();
        rt.register_fn("add", |_, (a, b): (f64, f64)| Ok(a + b))
            .unwrap();
        rt.exec("result = add(10, 20)").unwrap();
        let result: f64 = rt.get_global("result").unwrap();
        assert_eq!(result, 30.0);
    }

    #[test]
    fn require_builtin() {
        let rt = LuaRuntime::new().unwrap();
        rt.exec(r#"local m = require("math"); pi = m.pi"#).unwrap();
        let pi: f64 = rt.get_global("pi").unwrap();
        assert!((pi - std::f64::consts::PI).abs() < 1e-10);
    }
}
