#include "TextEditor.h"
#include "text_editor_capi.h"
#include <string>

// Thread-local scratch buffers for returning strings to C.
static thread_local std::string g_text_buf;
static thread_local std::string g_sel_buf;

extern "C" {

TextEditorHandle TextEditor_Create(void) {
    return static_cast<TextEditorHandle>(new TextEditor());
}

void TextEditor_Destroy(TextEditorHandle handle) {
    delete static_cast<TextEditor*>(handle);
}

void TextEditor_SetText(TextEditorHandle handle, const char* text) {
    static_cast<TextEditor*>(handle)->SetText(text ? text : "");
}

const char* TextEditor_GetText(TextEditorHandle handle) {
    g_text_buf = static_cast<TextEditor*>(handle)->GetText();
    return g_text_buf.c_str();
}

const char* TextEditor_GetSelectedText(TextEditorHandle handle) {
    g_sel_buf = static_cast<TextEditor*>(handle)->GetSelectedText();
    return g_sel_buf.c_str();
}

int TextEditor_GetTotalLines(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->GetTotalLines();
}

int TextEditor_IsTextChanged(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->IsTextChanged() ? 1 : 0;
}

void TextEditor_Render(TextEditorHandle handle, const char* title, float size_x, float size_y, int border) {
    static_cast<TextEditor*>(handle)->Render(title, ImVec2(size_x, size_y), border != 0);
}

void TextEditor_SetLanguageCPlusPlus(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetLanguageDefinition(TextEditor::LanguageDefinition::CPlusPlus());
}

void TextEditor_SetLanguageC(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetLanguageDefinition(TextEditor::LanguageDefinition::C());
}

void TextEditor_SetLanguageGLSL(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetLanguageDefinition(TextEditor::LanguageDefinition::GLSL());
}

void TextEditor_SetLanguageHLSL(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetLanguageDefinition(TextEditor::LanguageDefinition::HLSL());
}

void TextEditor_SetLanguageLua(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetLanguageDefinition(TextEditor::LanguageDefinition::Lua());
}

void TextEditor_SetLanguageSQL(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetLanguageDefinition(TextEditor::LanguageDefinition::SQL());
}

void TextEditor_SetPaletteDark(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetPalette(TextEditor::GetDarkPalette());
}

void TextEditor_SetPaletteLight(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetPalette(TextEditor::GetLightPalette());
}

void TextEditor_SetPaletteRetroBlue(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SetPalette(TextEditor::GetRetroBluePalette());
}

void TextEditor_SetReadOnly(TextEditorHandle handle, int value) {
    static_cast<TextEditor*>(handle)->SetReadOnly(value != 0);
}

int TextEditor_IsReadOnly(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->IsReadOnly() ? 1 : 0;
}

void TextEditor_SetShowWhitespaces(TextEditorHandle handle, int value) {
    static_cast<TextEditor*>(handle)->SetShowWhitespaces(value != 0);
}

void TextEditor_SetTabSize(TextEditorHandle handle, int size) {
    static_cast<TextEditor*>(handle)->SetTabSize(size);
}

int TextEditor_GetTabSize(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->GetTabSize();
}

int TextEditor_GetCursorLine(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->GetCursorPosition().mLine;
}

int TextEditor_GetCursorColumn(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->GetCursorPosition().mColumn;
}

void TextEditor_InsertText(TextEditorHandle handle, const char* text) {
    static_cast<TextEditor*>(handle)->InsertText(text ? text : "");
}

void TextEditor_Copy(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->Copy();
}

void TextEditor_Cut(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->Cut();
}

void TextEditor_Paste(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->Paste();
}

void TextEditor_Delete(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->Delete();
}

void TextEditor_SelectAll(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->SelectAll();
}

int TextEditor_HasSelection(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->HasSelection() ? 1 : 0;
}

int TextEditor_CanUndo(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->CanUndo() ? 1 : 0;
}

int TextEditor_CanRedo(TextEditorHandle handle) {
    return static_cast<TextEditor*>(handle)->CanRedo() ? 1 : 0;
}

void TextEditor_Undo(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->Undo();
}

void TextEditor_Redo(TextEditorHandle handle) {
    static_cast<TextEditor*>(handle)->Redo();
}

} // extern "C"
