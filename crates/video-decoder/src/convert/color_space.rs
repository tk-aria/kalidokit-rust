/// Color conversion matrix coefficients for YUV → RGB conversion.
#[derive(Debug, Clone, Copy)]
pub struct ColorMatrix {
    /// Y offset (typically 0.0 for full-range, 16/255 for limited)
    pub y_offset: f32,
    /// UV→R coefficient for V
    pub rv: f32,
    /// UV→G coefficient for U
    pub gu: f32,
    /// UV→G coefficient for V
    pub gv: f32,
    /// UV→B coefficient for U
    pub bu: f32,
}

/// Returns the BT.709 (HD video) color conversion matrix.
pub fn bt709() -> ColorMatrix {
    ColorMatrix {
        y_offset: 0.0,
        rv: 1.5748,
        gu: -0.1873,
        gv: -0.4681,
        bu: 1.8556,
    }
}

/// Returns the BT.601 (SD video) color conversion matrix.
pub fn bt601() -> ColorMatrix {
    ColorMatrix {
        y_offset: 0.0,
        rv: 1.402,
        gu: -0.344136,
        gv: -0.714136,
        bu: 1.772,
    }
}
