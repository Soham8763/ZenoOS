//! Graphical Framebuffer Console Writer for Limine (32-bit RGBA)

use crate::font::{get_glyph, FONT_HEIGHT, FONT_WIDTH};
use crate::limine::Framebuffer;
use core::fmt;

pub struct FramebufferConsole {
    buffer: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
    cursor_x: usize,
    cursor_y: usize,
    fg_color: u32,
    bg_color: u32,
}

unsafe impl Send for FramebufferConsole {}
unsafe impl Sync for FramebufferConsole {}

impl FramebufferConsole {
    pub const fn empty() -> Self {
        Self {
            buffer: core::ptr::null_mut(),
            width: 0,
            height: 0,
            pitch: 0,
            cursor_x: 0,
            cursor_y: 0,
            fg_color: 0x00E0E0E0, // Light gray/white
            bg_color: 0x000F1016, // Dark slate/navy
        }
    }

    pub fn init(&mut self, fb: &Framebuffer) {
        self.buffer = fb.address;
        self.width = fb.width as usize;
        self.height = fb.height as usize;
        self.pitch = fb.pitch as usize;
        self.cursor_x = 16;
        self.cursor_y = 16;
        self.clear();
    }

    #[inline]
    pub fn is_ready(&self) -> bool {
        !self.buffer.is_null()
    }

    pub fn clear(&mut self) {
        if !self.is_ready() {
            return;
        }
        for y in 0..self.height {
            for x in 0..self.width {
                self.put_pixel(x, y, self.bg_color);
            }
        }
        self.cursor_x = 16;
        self.cursor_y = 16;
    }

    #[inline]
    fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let offset = y * self.pitch + x * 4;
            unsafe {
                let pixel_ptr = self.buffer.add(offset) as *mut u32;
                pixel_ptr.write_volatile(color);
            }
        }
    }

    pub fn draw_char(&mut self, x: usize, y: usize, c: u8, color: u32) {
        let glyph = get_glyph(c);
        for row in 0..FONT_HEIGHT {
            let byte = glyph[row];
            for col in 0..FONT_WIDTH {
                if (byte & (0x80 >> col)) != 0 {
                    self.put_pixel(x + col, y + row, color);
                } else {
                    self.put_pixel(x + col, y + row, self.bg_color);
                }
            }
        }
    }

    pub fn write_char(&mut self, c: u8) {
        if !self.is_ready() {
            return;
        }

        match c {
            b'\n' => {
                self.newline();
            }
            b'\r' => {
                self.cursor_x = 16;
            }
            byte => {
                if self.cursor_x + FONT_WIDTH >= self.width - 16 {
                    self.newline();
                }
                self.draw_char(self.cursor_x, self.cursor_y, byte, self.fg_color);
                self.cursor_x += FONT_WIDTH;
            }
        }
    }

    fn newline(&mut self) {
        self.cursor_x = 16;
        self.cursor_y += FONT_HEIGHT + 2;

        if self.cursor_y + FONT_HEIGHT >= self.height - 16 {
            self.scroll();
        }
    }

    fn scroll(&mut self) {
        let row_height = FONT_HEIGHT + 2;
        let bytes_to_copy = (self.height - 32 - row_height) * self.pitch;
        let src = unsafe { self.buffer.add(16 * self.pitch) };
        let dst = unsafe { self.buffer.add(16 * self.pitch + row_height * self.pitch) };

        // Simple scroll copy
        unsafe {
            core::ptr::copy(dst, src, bytes_to_copy);
        }

        // Clear the new bottom line
        let clear_start_y = self.height - 16 - row_height;
        for y in clear_start_y..(self.height - 16) {
            for x in 16..(self.width - 16) {
                self.put_pixel(x, y, self.bg_color);
            }
        }
        self.cursor_y -= row_height;
    }
}

impl fmt::Write for FramebufferConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            self.write_char(b);
        }
        Ok(())
    }
}

