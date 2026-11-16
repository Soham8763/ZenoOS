//! Global Descriptor Table (GDT) + Task State Segment (TSS)
//!
//! Builds a minimal 64-bit GDT with:
//!   Slot 0 — null descriptor (required by x86)
//!   Slot 1 — kernel code segment (DPL 0, 64-bit)
//!   Slot 2 — kernel data segment (DPL 0)
//!   Slot 3 — user code segment  (DPL 3, 64-bit)
//!   Slot 4 — user data segment  (DPL 3)
//!   Slots 5+6 — 16-byte TSS descriptor (two GDT slots)
//!
//! The TSS provides IST1 — a dedicated 4 KiB stack for the double-fault
//! handler, so a stack-overflow can't triple-fault the machine.

use core::arch::asm;
use core::mem;

// ── Segment selector constants ────────────────────────────────────────────────
pub const KERNEL_CODE_SEL: u16 = 1 << 3; // GDT slot 1, RPL 0
pub const KERNEL_DATA_SEL: u16 = 2 << 3; // GDT slot 2, RPL 0
pub const USER_CODE_SEL: u16 = (3 << 3) | 3; // GDT slot 3, RPL 3
pub const USER_DATA_SEL: u16 = (4 << 3) | 3; // GDT slot 4, RPL 3
pub const TSS_SEL: u16 = 5 << 3;             // GDT slots 5+6, RPL 0

// ── Segment descriptor access byte flags ─────────────────────────────────────
const ACCESS_PRESENT: u8 = 1 << 7;
const ACCESS_DPL0: u8 = 0;
const ACCESS_DPL3: u8 = 3 << 5;
const ACCESS_CODE_DATA: u8 = 1 << 4; // descriptor type = code/data
const ACCESS_EXEC: u8 = 1 << 3;      // executable (code segment)
const ACCESS_RW: u8 = 1 << 1;        // readable (code) / writable (data)
const ACCESS_TSS: u8 = 0x09;         // available 64-bit TSS

// Flags nibble: 64-bit mode, 1-byte granularity (limit interpreted in bytes)
const FLAG_LONG: u8 = 1 << 1; // "L" bit  → 64-bit code
const FLAG_GRAN: u8 = 1 << 3; // "G" bit  → page granularity (not used in 64-bit flat mode)

/// Pack an x86-64 segment descriptor into a u64.
/// In 64-bit mode base and limit are ignored for code/data; we still fill them.
const fn make_seg(access: u8, flags: u8) -> u64 {
    // limit = 0xFFFFF (20 bits), base = 0
    let limit_lo: u64 = 0xFFFF;
    let limit_hi: u64 = 0xF;
    let base_lo: u64 = 0;
    let base_mid: u64 = 0;
    let base_hi: u64 = 0;

    limit_lo
        | (base_lo << 16)
        | (base_mid << 32)
        | ((access as u64) << 40)
        | (limit_hi << 48)
        | ((flags as u64) << 52)
        | (base_hi << 56)
}

// ── Task State Segment ────────────────────────────────────────────────────────

/// Minimal 104-byte TSS as specified by the Intel SDM Vol. 3A §7.7.
/// We only care about IST1 (used for the double-fault handler).
#[repr(C, packed)]
pub struct TaskStateSegment {
    _reserved0: u32,
    /// RSP0–RSP2: stack pointers for privilege-level transitions (we don't use these yet)
    pub rsp: [u64; 3],
    _reserved1: u64,
    /// IST1–IST7: interrupt-stack-table entries
    pub ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    /// I/O permission bitmap offset (set past end of TSS → no IOPB)
    pub iopb_offset: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        TaskStateSegment {
            _reserved0: 0,
            rsp: [0; 3],
            _reserved1: 0,
            ist: [0; 7],
            _reserved2: 0,
            _reserved3: 0,
            iopb_offset: mem::size_of::<TaskStateSegment>() as u16,
        }
    }
}

// ── Static storage ────────────────────────────────────────────────────────────

/// 4 KiB stack dedicated to the double-fault handler (IST1).
/// Must be 16-byte aligned per the x86-64 ABI.
#[repr(C, align(16))]
struct Stack([u8; 4096]);

static mut DOUBLE_FAULT_STACK: Stack = Stack([0u8; 4096]);
static mut TSS: TaskStateSegment = TaskStateSegment::new();

/// 7 descriptors: null, kcode, kdata, ucode, udata, tss_lo, tss_hi
static mut GDT: [u64; 7] = [0u64; 7];

/// GDTR value passed to `lgdt` — must be alive for the entire kernel lifetime.
#[repr(C, packed)]
struct GdtDescriptor {
    limit: u16,
    base: u64,
}

static mut GDTR: GdtDescriptor = GdtDescriptor { limit: 0, base: 0 };

// ── Init ──────────────────────────────────────────────────────────────────────

/// Build the GDT, install the TSS, call `lgdt`, and load the TSS with `ltr`.
/// Must be called once early in `_start`, before `init_idt`.
pub fn init_gdt() {
    unsafe {
        // Point IST1 to the TOP of our dedicated double-fault stack.
        // The stack grows down, so we pass the address of the byte just past
        // the array end.
        TSS.ist[0] = {
            let top = DOUBLE_FAULT_STACK.0.as_ptr() as u64 + 4096;
            top
        };

        // ── Fill the GDT ─────────────────────────────────────────────────────
        GDT[0] = 0; // null
        GDT[1] = make_seg(ACCESS_PRESENT | ACCESS_DPL0 | ACCESS_CODE_DATA | ACCESS_EXEC | ACCESS_RW, FLAG_LONG); // kernel code
        GDT[2] = make_seg(ACCESS_PRESENT | ACCESS_DPL0 | ACCESS_CODE_DATA | ACCESS_RW, 0); // kernel data
        GDT[3] = make_seg(ACCESS_PRESENT | ACCESS_DPL3 | ACCESS_CODE_DATA | ACCESS_EXEC | ACCESS_RW, FLAG_LONG); // user code
        GDT[4] = make_seg(ACCESS_PRESENT | ACCESS_DPL3 | ACCESS_CODE_DATA | ACCESS_RW, 0); // user data

        // TSS descriptor: 16-byte "system descriptor" that occupies two GDT slots.
        let tss_base = &TSS as *const _ as u64;
        let tss_limit = (mem::size_of::<TaskStateSegment>() - 1) as u64;

        // Low 8 bytes
        GDT[5] = (tss_limit & 0xFFFF)                     // limit 15:0
            | ((tss_base & 0xFFFFFF) << 16)               // base 23:0
            | ((ACCESS_PRESENT | ACCESS_TSS) as u64) << 40 // access
            | ((tss_limit >> 16) & 0xF) << 48             // limit 19:16
            | ((tss_base >> 24) & 0xFF) << 56;            // base 31:24

        // High 8 bytes: remaining 32 bits of base
        GDT[6] = (tss_base >> 32) & 0xFFFFFFFF;

        // ── Load GDTR ────────────────────────────────────────────────────────
        GDTR.limit = (mem::size_of::<[u64; 7]>() - 1) as u16;
        GDTR.base = GDT.as_ptr() as u64;

        asm!(
            "lgdt [{gdtr}]",
            // Reload CS via a far return trick
            "push {kcs}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",
            // Reload all data segments
            "mov ax, {kds}",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            gdtr = in(reg) &GDTR as *const _,
            kcs = const KERNEL_CODE_SEL as u64,
            kds = const KERNEL_DATA_SEL as u64,
            tmp = lateout(reg) _,
            out("ax") _,
        );

        // ── Load TSS ─────────────────────────────────────────────────────────
        asm!(
            "ltr ax",
            in("ax") TSS_SEL,
            options(nomem, nostack),
        );
    }

    crate::serial_println!("[ZenoOS] GDT loaded (7 entries, TSS installed, IST1 ready).");
}
