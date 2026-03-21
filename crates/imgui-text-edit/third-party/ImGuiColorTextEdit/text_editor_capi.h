#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef void* TextEditorHandle;

// Lifecycle
TextEditorHandle TextEditor_Create(void);
void TextEditor_Destroy(TextEditorHandle handle);

// Content
void TextEditor_SetText(TextEditorHandle handle, const char* text);
// Returns a pointer to an internal buffer; valid until the next call to TextEditor_GetText.
const char* TextEditor_GetText(TextEditorHandle handle);
const char* TextEditor_GetSelectedText(TextEditorHandle handle);
int TextEditor_GetTotalLines(TextEditorHandle handle);
int TextEditor_IsTextChanged(TextEditorHandle handle);

// Rendering
void TextEditor_Render(TextEditorHandle handle, const char* title, float size_x, float size_y, int border);

// Language definitions
void TextEditor_SetLanguageCPlusPlus(TextEditorHandle handle);
void TextEditor_SetLanguageC(TextEditorHandle handle);
void TextEditor_SetLanguageGLSL(TextEditorHandle handle);
void TextEditor_SetLanguageHLSL(TextEditorHandle handle);
void TextEditor_SetLanguageLua(TextEditorHandle handle);
void TextEditor_SetLanguageSQL(TextEditorHandle handle);

// Palette
void TextEditor_SetPaletteDark(TextEditorHandle handle);
void TextEditor_SetPaletteLight(TextEditorHandle handle);
void TextEditor_SetPaletteRetroBlue(TextEditorHandle handle);

// Options
void TextEditor_SetReadOnly(TextEditorHandle handle, int value);
int TextEditor_IsReadOnly(TextEditorHandle handle);
void TextEditor_SetShowWhitespaces(TextEditorHandle handle, int value);
void TextEditor_SetTabSize(TextEditorHandle handle, int size);
int TextEditor_GetTabSize(TextEditorHandle handle);

// Cursor
int TextEditor_GetCursorLine(TextEditorHandle handle);
int TextEditor_GetCursorColumn(TextEditorHandle handle);

// Edit operations
void TextEditor_InsertText(TextEditorHandle handle, const char* text);
void TextEditor_Copy(TextEditorHandle handle);
void TextEditor_Cut(TextEditorHandle handle);
void TextEditor_Paste(TextEditorHandle handle);
void TextEditor_Delete(TextEditorHandle handle);
void TextEditor_SelectAll(TextEditorHandle handle);
int TextEditor_HasSelection(TextEditorHandle handle);

// Undo/Redo
int TextEditor_CanUndo(TextEditorHandle handle);
int TextEditor_CanRedo(TextEditorHandle handle);
void TextEditor_Undo(TextEditorHandle handle);
void TextEditor_Redo(TextEditorHandle handle);

#ifdef __cplusplus
}
#endif
