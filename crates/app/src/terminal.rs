//! ImGui terminal emulator using portable-pty + vte ANSI parser.
//!
//! Spawns a real shell (via PTY) and renders its output inside an ImGui window
//! with ANSI color support.

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

/// Maximum number of lines kept in the scrollback buffer.
const MAX_SCROLLBACK: usize = 5000;

/// ANSI color index (0-15 standard colors).
#[derive(Debug, Clone, Copy)]
struct AnsiColor {
    r: f32,
    g: f32,
    b: f32,
}

impl AnsiColor {
    const fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
        }
    }
}

/// Standard ANSI 16-color palette (dark theme).
const ANSI_COLORS: [AnsiColor; 16] = [
    AnsiColor::new(0x1d, 0x1f, 0x21), // 0  Black
    AnsiColor::new(0xcc, 0x66, 0x66), // 1  Red
    AnsiColor::new(0xb5, 0xbd, 0x68), // 2  Green
    AnsiColor::new(0xf0, 0xc6, 0x74), // 3  Yellow
    AnsiColor::new(0x81, 0xa2, 0xbe), // 4  Blue
    AnsiColor::new(0xb2, 0x94, 0xbb), // 5  Magenta
    AnsiColor::new(0x8a, 0xbe, 0xb7), // 6  Cyan
    AnsiColor::new(0xc5, 0xc8, 0xc6), // 7  White
    AnsiColor::new(0x96, 0x98, 0x96), // 8  Bright Black
    AnsiColor::new(0xcc, 0x66, 0x66), // 9  Bright Red
    AnsiColor::new(0xb5, 0xbd, 0x68), // 10 Bright Green
    AnsiColor::new(0xf0, 0xc6, 0x74), // 11 Bright Yellow
    AnsiColor::new(0x81, 0xa2, 0xbe), // 12 Bright Blue
    AnsiColor::new(0xb2, 0x94, 0xbb), // 13 Bright Magenta
    AnsiColor::new(0x8a, 0xbe, 0xb7), // 14 Bright Cyan
    AnsiColor::new(0xff, 0xff, 0xff), // 15 Bright White
];

/// A colored character in the terminal buffer.
#[derive(Debug, Clone, Copy)]
struct Cell {
    ch: char,
    fg: u8, // ANSI color index (0-15), 255 = default
    bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: 255,
            bold: false,
        }
    }
}

/// Terminal buffer state, updated by the VTE parser.
#[derive(Debug)]
pub struct TerminalBuffer {
    lines: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    current_fg: u8,
    current_bold: bool,
    cols: usize,
    rows: usize,
    /// Input text buffer for the ImGui input field.
    pub input_buf: String,
}

impl TerminalBuffer {
    fn new(cols: usize, rows: usize) -> Self {
        Self {
            lines: vec![vec![Cell::default(); cols]],
            cursor_row: 0,
            cursor_col: 0,
            current_fg: 255,
            current_bold: false,
            cols,
            rows,
            input_buf: String::new(),
        }
    }

    fn ensure_row(&mut self, row: usize) {
        while self.lines.len() <= row {
            self.lines.push(vec![Cell::default(); self.cols]);
        }
    }

    /// Render terminal contents via ImGui.
    pub fn render_imgui(&self, ui: &dear_imgui_rs::Ui) {
        let default_color = [0.85f32, 0.85, 0.85, 1.0];

        // Show the last `rows` lines (or fewer)
        let total = self.lines.len();
        let start = total.saturating_sub(self.rows);

        for line in &self.lines[start..] {
            // Build colored spans
            let mut spans: Vec<([f32; 4], String)> = Vec::new();
            let mut current_color = default_color;
            let mut current_text = String::new();

            for cell in line {
                let color = if cell.fg == 255 {
                    default_color
                } else {
                    let idx = cell.fg as usize % 16;
                    let c = ANSI_COLORS[idx];
                    [c.r, c.g, c.b, 1.0]
                };

                if color != current_color && !current_text.is_empty() {
                    spans.push((current_color, std::mem::take(&mut current_text)));
                    current_color = color;
                }
                if color != current_color {
                    current_color = color;
                }
                current_text.push(cell.ch);
            }
            if !current_text.is_empty() {
                spans.push((current_color, current_text));
            }

            // Render spans on the same line using SameLine
            let mut first = true;
            for (color, text) in &spans {
                if !first {
                    ui.same_line();
                }
                first = false;
                // Trim trailing spaces for the last span
                let trimmed = text.trim_end();
                if !trimmed.is_empty() {
                    ui.text_colored(*color, trimmed);
                } else if first {
                    ui.text("");
                }
            }
            if first {
                // Empty line
                ui.text("");
            }
        }
    }
}

impl vte::Perform for TerminalBuffer {
    fn print(&mut self, c: char) {
        self.ensure_row(self.cursor_row);
        if self.cursor_col < self.cols {
            let row = self.cursor_row;
            let col = self.cursor_col;
            if col >= self.lines[row].len() {
                self.lines[row].resize(col + 1, Cell::default());
            }
            self.lines[row][col] = Cell {
                ch: c,
                fg: self.current_fg,
                bold: self.current_bold,
            };
            self.cursor_col += 1;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0a => {
                self.cursor_row += 1;
                self.cursor_col = 0;
                self.ensure_row(self.cursor_row);
                // Trim scrollback
                if self.lines.len() > MAX_SCROLLBACK {
                    let excess = self.lines.len() - MAX_SCROLLBACK;
                    self.lines.drain(0..excess);
                    self.cursor_row = self.cursor_row.saturating_sub(excess);
                }
            }
            b'\r' | 0x0d => {
                self.cursor_col = 0;
            }
            b'\t' => {
                let next_tab = (self.cursor_col / 8 + 1) * 8;
                self.cursor_col = next_tab.min(self.cols - 1);
            }
            0x08 => {
                // Backspace
                self.cursor_col = self.cursor_col.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let params: Vec<u16> = params.iter().flat_map(|sub| sub.iter().copied()).collect();
        match action {
            'm' => {
                // SGR — Select Graphic Rendition
                if params.is_empty() {
                    self.current_fg = 255;
                    self.current_bold = false;
                    return;
                }
                let mut i = 0;
                while i < params.len() {
                    match params[i] {
                        0 => {
                            self.current_fg = 255;
                            self.current_bold = false;
                        }
                        1 => self.current_bold = true,
                        22 => self.current_bold = false,
                        30..=37 => self.current_fg = (params[i] - 30) as u8,
                        90..=97 => self.current_fg = (params[i] - 90 + 8) as u8,
                        39 => self.current_fg = 255,
                        _ => {}
                    }
                    i += 1;
                }
            }
            'A' => {
                // Cursor Up
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            'B' => {
                // Cursor Down
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row += n;
                self.ensure_row(self.cursor_row);
            }
            'C' => {
                // Cursor Forward
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.cols - 1);
            }
            'D' => {
                // Cursor Back
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            'H' | 'f' => {
                // Cursor Position
                let row = params.first().copied().unwrap_or(1).max(1) as usize - 1;
                let col = params.get(1).copied().unwrap_or(1).max(1) as usize - 1;
                self.cursor_row = row;
                self.cursor_col = col;
                self.ensure_row(self.cursor_row);
            }
            'J' => {
                // Erase in Display
                let mode = params.first().copied().unwrap_or(0);
                match mode {
                    2 | 3 => {
                        self.lines.clear();
                        self.lines.push(vec![Cell::default(); self.cols]);
                        self.cursor_row = 0;
                        self.cursor_col = 0;
                    }
                    _ => {}
                }
            }
            'K' => {
                // Erase in Line
                self.ensure_row(self.cursor_row);
                let mode = params.first().copied().unwrap_or(0);
                let row = &mut self.lines[self.cursor_row];
                match mode {
                    0 => {
                        for i in self.cursor_col..row.len() {
                            row[i] = Cell::default();
                        }
                    }
                    1 => {
                        for i in 0..=self.cursor_col.min(row.len().saturating_sub(1)) {
                            row[i] = Cell::default();
                        }
                    }
                    2 => {
                        for cell in row.iter_mut() {
                            *cell = Cell::default();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _c: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

/// An interactive terminal emulator that can be rendered in ImGui.
pub struct ImGuiTerminal {
    buffer: Arc<Mutex<TerminalBuffer>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    _reader_thread: std::thread::JoinHandle<()>,
}

impl ImGuiTerminal {
    /// Spawn a new terminal with the user's default shell.
    pub fn new(cols: u16, rows: u16) -> Result<Self, anyhow::Error> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| anyhow::anyhow!("PTY open failed: {e}"))?;

        // Spawn shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");
        pair.slave
            .spawn_command(cmd)
            .map_err(|e| anyhow::anyhow!("Shell spawn failed: {e}"))?;

        let buffer = Arc::new(Mutex::new(TerminalBuffer::new(cols as usize, rows as usize)));
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pair.master.take_writer().unwrap()));

        // Reader thread: reads PTY output and feeds to VTE parser
        let reader_buf = Arc::clone(&buffer);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let reader_thread = std::thread::Builder::new()
            .name("terminal-reader".into())
            .spawn(move || {
                let mut parser = vte::Parser::new();
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut term = reader_buf.lock().unwrap();
                            for byte in &buf[..n] {
                                parser.advance(&mut *term, *byte);
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
            .unwrap();

        Ok(Self {
            buffer,
            writer,
            _reader_thread: reader_thread,
        })
    }

    /// Send raw bytes to the terminal (keyboard input).
    pub fn send(&self, data: &[u8]) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(data);
            let _ = w.flush();
        }
    }

    /// Send a string to the terminal.
    pub fn send_str(&self, s: &str) {
        self.send(s.as_bytes());
    }

    /// Render the terminal in the current ImGui window.
    ///
    /// Returns true if the user pressed Enter (command submitted).
    pub fn render(&self, ui: &dear_imgui_rs::Ui) -> bool {
        let buf = self.buffer.lock().unwrap();
        buf.render_imgui(ui);

        // Auto-scroll to bottom
        if ui.scroll_y() >= ui.scroll_max_y() - 10.0 {
            ui.set_scroll_here_y(1.0);
        }

        false
    }

    /// Render with an input line at the bottom.
    /// Returns true if the user submitted a command.
    pub fn render_with_input(&self, ui: &dear_imgui_rs::Ui) -> bool {
        // Terminal output
        {
            let buf = self.buffer.lock().unwrap();
            buf.render_imgui(ui);
        }

        // Auto-scroll
        if ui.scroll_y() >= ui.scroll_max_y() - 10.0 {
            ui.set_scroll_here_y(1.0);
        }

        // Input line
        ui.separator();
        let mut submitted = false;
        let mut input = self.buffer.lock().unwrap().input_buf.clone();
        ui.set_next_item_width(-1.0);
        if ui
            .input_text("##term_input", &mut input)
            .enter_returns_true(true)
            .build()
        {
            // Send command + newline
            self.send_str(&input);
            self.send(b"\n");
            input.clear();
            submitted = true;
        }
        self.buffer.lock().unwrap().input_buf = input;
        submitted
    }
}
