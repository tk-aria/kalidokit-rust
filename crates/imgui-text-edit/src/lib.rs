//! ImGuiColorTextEdit — syntax-highlighting code editor for Dear ImGui.
//!
//! Wraps the C++ ImGuiColorTextEdit library via a thin C FFI layer.
//! Requires the same ImGui context used by `dear-imgui-rs`.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

// ── FFI declarations ──

type TextEditorHandle = *mut c_void;

unsafe extern "C" {
    fn TextEditor_Create() -> TextEditorHandle;
    fn TextEditor_Destroy(handle: TextEditorHandle);

    fn TextEditor_SetText(handle: TextEditorHandle, text: *const c_char);
    fn TextEditor_GetText(handle: TextEditorHandle) -> *const c_char;
    fn TextEditor_GetSelectedText(handle: TextEditorHandle) -> *const c_char;
    fn TextEditor_GetTotalLines(handle: TextEditorHandle) -> c_int;
    fn TextEditor_IsTextChanged(handle: TextEditorHandle) -> c_int;

    fn TextEditor_Render(
        handle: TextEditorHandle,
        title: *const c_char,
        size_x: f32,
        size_y: f32,
        border: c_int,
    );

    fn TextEditor_SetLanguageCPlusPlus(handle: TextEditorHandle);
    fn TextEditor_SetLanguageC(handle: TextEditorHandle);
    fn TextEditor_SetLanguageGLSL(handle: TextEditorHandle);
    fn TextEditor_SetLanguageHLSL(handle: TextEditorHandle);
    fn TextEditor_SetLanguageLua(handle: TextEditorHandle);
    fn TextEditor_SetLanguageSQL(handle: TextEditorHandle);

    fn TextEditor_SetPaletteDark(handle: TextEditorHandle);
    fn TextEditor_SetPaletteLight(handle: TextEditorHandle);
    fn TextEditor_SetPaletteRetroBlue(handle: TextEditorHandle);

    fn TextEditor_SetReadOnly(handle: TextEditorHandle, value: c_int);
    fn TextEditor_IsReadOnly(handle: TextEditorHandle) -> c_int;
    fn TextEditor_SetShowWhitespaces(handle: TextEditorHandle, value: c_int);
    fn TextEditor_SetTabSize(handle: TextEditorHandle, size: c_int);
    fn TextEditor_GetTabSize(handle: TextEditorHandle) -> c_int;

    fn TextEditor_GetCursorLine(handle: TextEditorHandle) -> c_int;
    fn TextEditor_GetCursorColumn(handle: TextEditorHandle) -> c_int;

    fn TextEditor_InsertText(handle: TextEditorHandle, text: *const c_char);
    fn TextEditor_Copy(handle: TextEditorHandle);
    fn TextEditor_Cut(handle: TextEditorHandle);
    fn TextEditor_Paste(handle: TextEditorHandle);
    fn TextEditor_Delete(handle: TextEditorHandle);
    fn TextEditor_SelectAll(handle: TextEditorHandle);
    fn TextEditor_HasSelection(handle: TextEditorHandle) -> c_int;

    fn TextEditor_CanUndo(handle: TextEditorHandle) -> c_int;
    fn TextEditor_CanRedo(handle: TextEditorHandle) -> c_int;
    fn TextEditor_Undo(handle: TextEditorHandle);
    fn TextEditor_Redo(handle: TextEditorHandle);
}

// ── Public API ──

/// Supported language definitions for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    CPlusPlus,
    C,
    GLSL,
    HLSL,
    Lua,
    SQL,
}

/// Color palette presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    Dark,
    Light,
    RetroBlue,
}

/// A syntax-highlighting code editor widget for Dear ImGui.
///
/// This editor supports undo/redo, copy/cut/paste, breakpoints, error markers,
/// and configurable language definitions.
pub struct CodeEditor {
    handle: TextEditorHandle,
}

// The underlying C++ TextEditor is single-threaded (ImGui context bound).
// It's safe to send between threads as long as it's only used on one at a time.
unsafe impl Send for CodeEditor {}

impl CodeEditor {
    /// Create a new code editor with the dark palette.
    pub fn new() -> Self {
        let handle = unsafe { TextEditor_Create() };
        let editor = Self { handle };
        editor.set_palette(Palette::Dark);
        editor
    }

    /// Set the text content.
    pub fn set_text(&self, text: &str) {
        let c = CString::new(text).unwrap_or_default();
        unsafe { TextEditor_SetText(self.handle, c.as_ptr()) }
    }

    /// Get the full text content.
    pub fn get_text(&self) -> String {
        unsafe {
            let ptr = TextEditor_GetText(self.handle);
            if ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    /// Get the currently selected text.
    pub fn get_selected_text(&self) -> String {
        unsafe {
            let ptr = TextEditor_GetSelectedText(self.handle);
            if ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    /// Get total number of lines.
    pub fn total_lines(&self) -> i32 {
        unsafe { TextEditor_GetTotalLines(self.handle) }
    }

    /// Returns true if the text was modified since the last frame.
    pub fn is_text_changed(&self) -> bool {
        unsafe { TextEditor_IsTextChanged(self.handle) != 0 }
    }

    /// Render the editor inside the current ImGui window.
    ///
    /// `title` is the ImGui widget ID. `size` of `[0.0, 0.0]` fills available space.
    pub fn render(&self, title: &str, size: [f32; 2], border: bool) {
        let c = CString::new(title).unwrap_or_default();
        unsafe {
            TextEditor_Render(self.handle, c.as_ptr(), size[0], size[1], border as c_int);
        }
    }

    /// Set the syntax highlighting language.
    pub fn set_language(&self, lang: Language) {
        unsafe {
            match lang {
                Language::CPlusPlus => TextEditor_SetLanguageCPlusPlus(self.handle),
                Language::C => TextEditor_SetLanguageC(self.handle),
                Language::GLSL => TextEditor_SetLanguageGLSL(self.handle),
                Language::HLSL => TextEditor_SetLanguageHLSL(self.handle),
                Language::Lua => TextEditor_SetLanguageLua(self.handle),
                Language::SQL => TextEditor_SetLanguageSQL(self.handle),
            }
        }
    }

    /// Set the color palette.
    pub fn set_palette(&self, palette: Palette) {
        unsafe {
            match palette {
                Palette::Dark => TextEditor_SetPaletteDark(self.handle),
                Palette::Light => TextEditor_SetPaletteLight(self.handle),
                Palette::RetroBlue => TextEditor_SetPaletteRetroBlue(self.handle),
            }
        }
    }

    /// Set read-only mode.
    pub fn set_read_only(&self, value: bool) {
        unsafe { TextEditor_SetReadOnly(self.handle, value as c_int) }
    }

    /// Check if read-only.
    pub fn is_read_only(&self) -> bool {
        unsafe { TextEditor_IsReadOnly(self.handle) != 0 }
    }

    /// Show/hide whitespace characters.
    pub fn set_show_whitespaces(&self, value: bool) {
        unsafe { TextEditor_SetShowWhitespaces(self.handle, value as c_int) }
    }

    /// Set tab size.
    pub fn set_tab_size(&self, size: i32) {
        unsafe { TextEditor_SetTabSize(self.handle, size) }
    }

    /// Get tab size.
    pub fn tab_size(&self) -> i32 {
        unsafe { TextEditor_GetTabSize(self.handle) }
    }

    /// Get cursor line (0-based).
    pub fn cursor_line(&self) -> i32 {
        unsafe { TextEditor_GetCursorLine(self.handle) }
    }

    /// Get cursor column (0-based).
    pub fn cursor_column(&self) -> i32 {
        unsafe { TextEditor_GetCursorColumn(self.handle) }
    }

    /// Insert text at the current cursor position.
    pub fn insert_text(&self, text: &str) {
        let c = CString::new(text).unwrap_or_default();
        unsafe { TextEditor_InsertText(self.handle, c.as_ptr()) }
    }

    pub fn copy(&self) {
        unsafe { TextEditor_Copy(self.handle) }
    }

    pub fn cut(&self) {
        unsafe { TextEditor_Cut(self.handle) }
    }

    pub fn paste(&self) {
        unsafe { TextEditor_Paste(self.handle) }
    }

    pub fn delete(&self) {
        unsafe { TextEditor_Delete(self.handle) }
    }

    pub fn select_all(&self) {
        unsafe { TextEditor_SelectAll(self.handle) }
    }

    pub fn has_selection(&self) -> bool {
        unsafe { TextEditor_HasSelection(self.handle) != 0 }
    }

    pub fn can_undo(&self) -> bool {
        unsafe { TextEditor_CanUndo(self.handle) != 0 }
    }

    pub fn can_redo(&self) -> bool {
        unsafe { TextEditor_CanRedo(self.handle) != 0 }
    }

    pub fn undo(&self) {
        unsafe { TextEditor_Undo(self.handle) }
    }

    pub fn redo(&self) {
        unsafe { TextEditor_Redo(self.handle) }
    }
}

impl Default for CodeEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CodeEditor {
    fn drop(&mut self) {
        unsafe { TextEditor_Destroy(self.handle) }
    }
}
