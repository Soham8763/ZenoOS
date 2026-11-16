//! Interrupt Descriptor Table (IDT)
//!
//! Provides handlers for all 32 CPU exceptions (vectors 0–31) and
//! the keyboard IRQ (vector 0x21 after PIC remapping).
//!
//! Stack layout when an exception fires (after hardware push + our stub):
//!   [RSP+40] SS
//!   [RSP+32] RSP (before interrupt)
//!   [RSP+24] RFLAGS
//!   [RSP+16] CS
//!   [RSP+ 8] RIP
//!   [RSP+ 0] error code (real or synthetic 0)
//!
//! We use a single C-ABI handler (`exception_common`) that receives a pointer
//! to this frame plus the vector number, so every exception gets a uniform,
//! formatted crash dump.

use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

// ── IDT entry layout (16 bytes, §6.14.1 Intel SDM Vol. 3A) ───────────────────

/// A single IDT gate descriptor.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct IdtEntry {
    offset_lo: u16,  // handler bits 15:0
    selector: u16,   // code segment selector
    ist: u8,         // interrupt stack table index (0 = no IST)
    type_attr: u8,   // gate type + DPL + present flag
    offset_mid: u16, // handler bits 31:16
    offset_hi: u32,  // handler bits 63:32
    _zero: u32,      // must be zero
}

const GATE_INTERRUPT: u8 = 0x8E; // present, DPL 0, 64-bit interrupt gate
const GATE_TRAP: u8 = 0x8F;      // present, DPL 0, 64-bit trap gate (don't mask IF)
const GATE_INT_DPL3: u8 = 0xEE;  // present, DPL 3, 64-bit interrupt gate (for int3 from user)

impl IdtEntry {
    const fn missing() -> Self {
        IdtEntry {
            offset_lo: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_hi: 0,
            _zero: 0,
        }
    }

    /// Set this gate to call `handler` on the main kernel stack.
    fn set_handler(&mut self, handler: u64, attr: u8) {
        self.offset_lo = handler as u16;
        self.selector = super::gdt::KERNEL_CODE_SEL;
        self.ist = 0;
        self.type_attr = attr;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_hi = (handler >> 32) as u32;
        self._zero = 0;
    }

    /// Like set_handler but switches to IST stack `n` (1-indexed).
    fn set_handler_ist(&mut self, handler: u64, attr: u8, ist: u8) {
        self.set_handler(handler, attr);
        self.ist = ist;
    }
}

// ── Static IDT (256 entries × 16 bytes = 4 KiB) ──────────────────────────────

static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];

#[repr(C, packed)]
struct IdtDescriptor {
    limit: u16,
    base: u64,
}

static mut IDTR: IdtDescriptor = IdtDescriptor { limit: 0, base: 0 };

// ── Exception frame passed to our common handler ──────────────────────────────

/// Mirrors the stack layout described at the top of this file.
#[repr(C)]
pub struct ExceptionFrame {
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

// ── Exception name table (vectors 0–31) ──────────────────────────────────────

static EXCEPTION_NAMES: [&str; 32] = [
    "#DE Divide-by-Zero",
    "#DB Debug",
    "NMI Non-Maskable Interrupt",
    "#BP Breakpoint",
    "#OF Overflow",
    "#BR Bound Range Exceeded",
    "#UD Invalid Opcode",
    "#NM Device Not Available",
    "#DF Double Fault",
    "Coprocessor Segment Overrun",
    "#TS Invalid TSS",
    "#NP Segment Not Present",
    "#SS Stack Segment Fault",
    "#GP General Protection Fault",
    "#PF Page Fault",
    "Reserved (15)",
    "#MF x87 Floating-Point",
    "#AC Alignment Check",
    "#MC Machine Check",
    "#XM SIMD Floating-Point",
    "#VE Virtualization Exception",
    "#CP Control Protection",
    "Reserved (22)",
    "Reserved (23)",
    "Reserved (24)",
    "Reserved (25)",
    "Reserved (26)",
    "Reserved (27)",
    "#HV Hypervisor Injection",
    "#VC VMM Communication",
    "#SX Security Exception",
    "Reserved (31)",
];

// ── Common exception handler (called from assembly stubs) ─────────────────────

/// Called by every exception stub with:
///   `vector` = exception number
///   `frame`  = pointer to the ExceptionFrame on the interrupted stack
#[no_mangle]
pub extern "C" fn exception_common(vector: u64, frame: *const ExceptionFrame) {
    let frame = unsafe { &*frame };
    let name = if (vector as usize) < 32 {
        EXCEPTION_NAMES[vector as usize]
    } else {
        "Unknown"
    };

    // Breakpoint (#BP) is non-fatal — just log and return.
    if vector == 3 {
        crate::serial_println!(
            "[ZenoOS] #BP Breakpoint @ RIP={:#018x} — continuing.",
            frame.rip
        );
        crate::kprintln!(
            "[#BP] Breakpoint @ {:#018x} — continuing.",
            frame.rip
        );
        return;
    }

    // All other exceptions are fatal — print a crash dump and halt.
    crate::serial_println!("\n╔══════════════════════════════════════════════╗");
    crate::serial_println!("║           KERNEL EXCEPTION — HALTING         ║");
    crate::serial_println!("╠══════════════════════════════════════════════╣");
    crate::serial_println!("║  Vec  : {:#04x}  {}", vector, name);
    crate::serial_println!("║  ErrC : {:#018x}", frame.error_code);
    crate::serial_println!("║  RIP  : {:#018x}", frame.rip);
    crate::serial_println!("║  CS   : {:#06x}", frame.cs);
    crate::serial_println!("║  RSP  : {:#018x}", frame.rsp);
    crate::serial_println!("║  SS   : {:#06x}", frame.ss);
    crate::serial_println!("║  RFLG : {:#018x}", frame.rflags);
    crate::serial_println!("╚══════════════════════════════════════════════╝");

    crate::kprintln!("\n[EXCEPTION] {} (vec={:#x})", name, vector);
    crate::kprintln!("  RIP={:#018x}  RSP={:#018x}", frame.rip, frame.rsp);
    crate::kprintln!("  ErrCode={:#018x}  RFLAGS={:#018x}", frame.error_code, frame.rflags);
    crate::kprintln!("  System halted.");

    crate::hcf();
}

// ── Keyboard IRQ handler ──────────────────────────────────────────────────────

/// Simple US-QWERTY scancode-set-1 to ASCII table (make codes only).
static SCANCODE_TABLE: [u8; 58] = [
    0,   27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0',
    b'-', b'=', 8,   9,  b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i',
    b'o', b'p', b'[', b']', b'\n', 0,  b'a', b's', b'd', b'f', b'g', b'h',
    b'j', b'k', b'l', b';', b'\'', b'`', 0,  b'\\', b'z', b'x', b'c', b'v',
    b'b', b'n', b'm', b',', b'.', b'/', 0,  b'*', 0,  b' ',
];

static SHIFT_HELD: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn keyboard_handler() {
    // Read scancode from PS/2 data port
    let scancode: u8 = unsafe {
        let val: u8;
        asm!("in al, dx", out("al") val, in("dx") 0x60u16, options(nomem, nostack, preserves_flags));
        val
    };

    // Track shift key state (make=0x2A/0x36, break=0xAA/0xB6)
    match scancode {
        0x2A | 0x36 => { SHIFT_HELD.store(true, Ordering::Relaxed); }
        0xAA | 0xB6 => { SHIFT_HELD.store(false, Ordering::Relaxed); }
        code if code < 0x80 => {
            // Make code (key press)
            let idx = code as usize;
            if idx < SCANCODE_TABLE.len() {
                let ch = SCANCODE_TABLE[idx];
                if ch != 0 {
                    let ch = if SHIFT_HELD.load(Ordering::Relaxed) {
                        (ch as char).to_ascii_uppercase() as u8
                    } else {
                        ch
                    };
                    // Print to both serial and framebuffer
                    crate::serial_println!("[KBD] key: {}", ch as char);
                    crate::kprint!("{}", ch as char);
                }
            }
        }
        _ => {} // break codes — ignore
    }

    // Send EOI to master PIC (even though the PIC is masked, QEMU still
    // expects an EOI for vector 0x21 when PIC is in legacy mode during early boot)
    unsafe {
        asm!(
            "out dx, al",
            in("dx") 0x20u16,
            in("al") 0x20u8,
            options(nomem, nostack, preserves_flags),
        );
    }
}

// ── Assembly stubs ────────────────────────────────────────────────────────────
//
// For exceptions WITHOUT an error code, we push a synthetic 0 so the frame
// is uniform. For those WITH an error code the CPU already pushed it.
//
// After establishing a uniform frame we call exception_common(vector, &frame).
//
// The keyboard IRQ stub just saves/restores registers and calls keyboard_handler.

core::arch::global_asm!(
    // ── Helper macro-like sequences ───────────────────────────────────────────
    // Push vector, call common handler, then iretq.
    // Defined as a local macro via the .macro directive.

    // Exceptions WITHOUT error code: push 0, then vector
    ".macro isr_no_err vec",
    "   .global isr_\\vec",
    "   isr_\\vec:",
    "   push 0",           // synthetic error code
    "   push \\vec",       // vector number
    "   jmp exception_stub",
    ".endm",

    // Exceptions WITH error code: CPU already pushed it; just push vector
    ".macro isr_err vec",
    "   .global isr_\\vec",
    "   isr_\\vec:",
    "   push \\vec",       // vector number
    "   jmp exception_stub",
    ".endm",

    // ── Instantiate all 32 CPU exception stubs ────────────────────────────────
    "isr_no_err 0",   // #DE
    "isr_no_err 1",   // #DB
    "isr_no_err 2",   // NMI
    "isr_no_err 3",   // #BP
    "isr_no_err 4",   // #OF
    "isr_no_err 5",   // #BR
    "isr_no_err 6",   // #UD
    "isr_no_err 7",   // #NM
    "isr_err    8",   // #DF  (error code = 0, but CPU still pushes it)
    "isr_no_err 9",   // Coprocessor Segment Overrun
    "isr_err    10",  // #TS
    "isr_err    11",  // #NP
    "isr_err    12",  // #SS
    "isr_err    13",  // #GP
    "isr_err    14",  // #PF
    "isr_no_err 15",
    "isr_no_err 16",  // #MF
    "isr_err    17",  // #AC
    "isr_no_err 18",  // #MC
    "isr_no_err 19",  // #XM
    "isr_no_err 20",  // #VE
    "isr_err    21",  // #CP
    "isr_no_err 22",
    "isr_no_err 23",
    "isr_no_err 24",
    "isr_no_err 25",
    "isr_no_err 26",
    "isr_no_err 27",
    "isr_no_err 28",
    "isr_no_err 29",
    "isr_err    30",  // #SX
    "isr_no_err 31",

    // ── Common exception dispatcher ───────────────────────────────────────────
    // Stack at entry:
    //   [RSP+ 0] = vector  (we just pushed)
    //   [RSP+ 8] = error code (real or synthetic)
    //   [RSP+16] = RIP
    //   [RSP+24] = CS
    //   [RSP+32] = RFLAGS
    //   [RSP+40] = RSP (pre-interrupt)
    //   [RSP+48] = SS
    "exception_stub:",
    "   pop rdi",              // arg 1: vector
    "   mov rsi, rsp",          // arg 2: &ExceptionFrame
    "   push rbx",              // save callee-saved rbx
    "   mov rbx, rsp",          // record current rsp
    "   and rsp, -16",          // 16-byte ABI stack alignment
    "   call exception_common",
    "   mov rsp, rbx",          // restore unaligned rsp
    "   pop rbx",               // restore rbx
    "   add rsp, 8",            // discard error code so stack top is RIP
    "   iretq",

    // ── APIC Timer stub (vector 0x20) ─────────────────────────────────────────
    ".global apic_timer_stub",
    "apic_timer_stub:",
    "   push rbp",
    "   mov rbp, rsp",
    "   push rax",
    "   push rcx",
    "   push rdx",
    "   push rsi",
    "   push rdi",
    "   push r8",
    "   push r9",
    "   push r10",
    "   push r11",
    "   call apic_timer_handler",
    "   pop r11",
    "   pop r10",
    "   pop r9",
    "   pop r8",
    "   pop rdi",
    "   pop rsi",
    "   pop rdx",
    "   pop rcx",
    "   pop rax",
    "   pop rbp",
    "   iretq",

    // ── Keyboard IRQ stub (vector 0x21) ───────────────────────────────────────
    ".global keyboard_stub",
    "keyboard_stub:",
    "   push rbp",
    "   mov rbp, rsp",
    "   push rax",
    "   push rcx",
    "   push rdx",
    "   push rsi",
    "   push rdi",
    "   push r8",
    "   push r9",
    "   push r10",
    "   push r11",
    "   call keyboard_handler",
    "   pop r11",
    "   pop r10",
    "   pop r9",
    "   pop r8",
    "   pop rdi",
    "   pop rsi",
    "   pop rdx",
    "   pop rcx",
    "   pop rax",
    "   pop rbp",
    "   iretq",
);

// Forward-declare the assembly stubs so we can take their addresses.
extern "C" {
    fn isr_0();  fn isr_1();  fn isr_2();  fn isr_3();
    fn isr_4();  fn isr_5();  fn isr_6();  fn isr_7();
    fn isr_8();  fn isr_9();  fn isr_10(); fn isr_11();
    fn isr_12(); fn isr_13(); fn isr_14(); fn isr_15();
    fn isr_16(); fn isr_17(); fn isr_18(); fn isr_19();
    fn isr_20(); fn isr_21(); fn isr_22(); fn isr_23();
    fn isr_24(); fn isr_25(); fn isr_26(); fn isr_27();
    fn isr_28(); fn isr_29(); fn isr_30(); fn isr_31();
    fn apic_timer_stub();
    fn keyboard_stub();
}

// ── Public init ───────────────────────────────────────────────────────────────

/// Populate the IDT and call `lidt`. Must be called after `init_gdt`.
pub fn init_idt() {
    unsafe {
        // CPU exceptions (vectors 0–31)
        let stubs: [unsafe extern "C" fn(); 32] = [
            isr_0,  isr_1,  isr_2,  isr_3,
            isr_4,  isr_5,  isr_6,  isr_7,
            isr_8,  isr_9,  isr_10, isr_11,
            isr_12, isr_13, isr_14, isr_15,
            isr_16, isr_17, isr_18, isr_19,
            isr_20, isr_21, isr_22, isr_23,
            isr_24, isr_25, isr_26, isr_27,
            isr_28, isr_29, isr_30, isr_31,
        ];

        for (vec, &stub) in stubs.iter().enumerate() {
            let attr = match vec {
                3 => GATE_INT_DPL3, // #BP: allow from user mode too
                8 => {
                    // #DF: use IST1 (double-fault dedicated stack)
                    IDT[vec].set_handler_ist(stub as *const () as u64, GATE_INTERRUPT, 1);
                    continue;
                }
                _ => GATE_INTERRUPT,
            };
            IDT[vec].set_handler(stub as *const () as u64, attr);
        }

        // APIC timer — vector 0x20
        IDT[0x20].set_handler(apic_timer_stub as *const () as u64, GATE_INTERRUPT);

        // PS/2 keyboard — vector 0x21
        IDT[0x21].set_handler(keyboard_stub as *const () as u64, GATE_INTERRUPT);

        // Load IDTR
        IDTR.limit = (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16;
        IDTR.base = IDT.as_ptr() as u64;

        asm!(
            "lidt [{idtr}]",
            idtr = in(reg) &IDTR as *const _,
            options(nostack),
        );
    }

    crate::serial_println!("[ZenoOS] IDT loaded (256 entries, all 32 CPU exceptions covered).");
}
