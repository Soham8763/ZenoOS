#![no_std]
#![no_main]

mod font;
mod limine;
mod serial;
mod sync;
mod writer;

use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;
use limine::*;
use serial::SerialPort;
use sync::Spinlock;
use writer::FramebufferConsole;

// Limine protocol markers and requests
#[used]
#[link_section = ".requests_start_marker"]
static START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[link_section = ".requests"]
static BASE_REVISION: BaseRevision = BaseRevision::new(3);

#[used]
#[link_section = ".requests"]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[link_section = ".requests"]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[link_section = ".requests"]
static MEMMAP_REQUEST: MemmapRequest = MemmapRequest::new();

#[used]
#[link_section = ".requests_end_marker"]
static END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

static CONSOLE: Spinlock<FramebufferConsole> = Spinlock::new(FramebufferConsole::empty());

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        $crate::print_fmt(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("\n"));
    ($($arg:tt)*) => {{
        $crate::print_fmt(format_args!($($arg)*));
        $crate::kprint!("\n");
    }};
}

pub fn print_fmt(args: core::fmt::Arguments) {
    serial::_print(args);
    let mut console = CONSOLE.lock();
    let _ = console.write_fmt(args);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    SerialPort::init();
    serial_println!("[ZenoOS] Serial port initialized successfully.");

    // Check Limine base revision handshake
    if !BASE_REVISION.is_supported() {
        serial_println!(
            "[ZenoOS WARN] Base revision handshake returned {}. Proceeding with detected requests...",
            BASE_REVISION.revision
        );
    } else {
        serial_println!("[ZenoOS] Limine protocol handshake confirmed (Base Revision 3).");
    }

    // Initialize Framebuffer Console if available
    let mut fb_width = 0;
    let mut fb_height = 0;
    let mut fb_bpp = 0;

    if let Some(fb_response) = FRAMEBUFFER_REQUEST.response() {
        let fbs = fb_response.framebuffers();
        if let Some(&fb) = fbs.first() {
            {
                let mut console = CONSOLE.lock();
                console.init(fb);
            }
            fb_width = fb.width;
            fb_height = fb.height;
            fb_bpp = fb.bpp;
            serial_println!(
                "[ZenoOS] Framebuffer acquired: {}x{} @ {} bpp",
                fb.width,
                fb.height,
                fb.bpp
            );
        }
    }

    // Print welcome banner
    kprintln!("============================================================");
    kprintln!("                 ZENO-OS :: BARE-METAL KERNEL                ");
    kprintln!("         64-Bit Modular OS Core (Rust no_std / Limine)      ");
    kprintln!("============================================================");
    kprintln!();

    if fb_width > 0 {
        kprintln!("[+] Display:  {}x{} ({} bpp linear framebuffer)", fb_width, fb_height, fb_bpp);
    } else {
        kprintln!("[-] Display:  No linear framebuffer provided (headless mode)");
    }

    // Inspect HHDM (Higher Half Direct Map)
    if let Some(hhdm) = HHDM_REQUEST.response() {
        kprintln!("[+] HHDM:     Direct physical mapping at offset {:#018x}", hhdm.offset);
    } else {
        kprintln!("[-] HHDM:     Not provided by bootloader");
    }

    // Inspect Memory Map
    if let Some(memmap) = MEMMAP_REQUEST.response() {
        let entries = memmap.entries();
        let mut total_usable_bytes: u64 = 0;
        let mut total_ram_bytes: u64 = 0;

        for &entry in entries {
            total_ram_bytes += entry.length;
            if entry.entry_type == LIMINE_MEMMAP_USABLE {
                total_usable_bytes += entry.length;
            }
        }

        kprintln!(
            "[+] Memory:   {} MB usable RAM (out of {} MB total physical RAM, {} entries)",
            total_usable_bytes / (1024 * 1024),
            total_ram_bytes / (1024 * 1024),
            entries.len()
        );
    } else {
        kprintln!("[-] Memory:   Memory map not provided by bootloader");
    }

    kprintln!();
    kprintln!("[*] System State: 64-bit Long Mode active. Halting CPU idle loop.");
    kprintln!("============================================================");

    hcf();
}

#[inline(always)]
pub fn hcf() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("\n[KERNEL PANIC] {}", info);
    kprintln!("\n[KERNEL PANIC] {}", info);
    hcf();
}
