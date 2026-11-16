//! Local APIC initialization and periodic timer.
//!
//! Strategy:
//! 1. Remap and mask the legacy 8259 PIC (vectors 0x20–0x2F).
//! 2. Detect x2APIC support (CPUID.1:ECX[21]).
//!    - If available: enable x2APIC via IA32_APIC_BASE MSR — no MMIO needed.
//!    - Otherwise: fall back to xAPIC MMIO, mapping via HHDM offset.
//! 3. Enable the Local APIC (Spurious Interrupt Vector Register).
//! 4. Calibrate the APIC timer against PIT channel 2 (~10 ms window).
//! 5. Program the APIC timer in periodic mode at ~100 Hz (vector 0x20).

use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ── Global tick counter ───────────────────────────────────────────────────────
pub static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

// ── Mode flag ─────────────────────────────────────────────────────────────────
static X2APIC_MODE: AtomicBool = AtomicBool::new(false);

// ── xAPIC register offsets (from MMIO base) ──────────────────────────────────
const APIC_EOI: u32 = 0x0B0;
const APIC_SPURIOUS: u32 = 0x0F0;
const APIC_TIMER_LVT: u32 = 0x320;
const APIC_TIMER_INIT: u32 = 0x380;
const APIC_TIMER_CURRENT: u32 = 0x390;
const APIC_TIMER_DIVIDE: u32 = 0x3E0;

// ── APIC flags ────────────────────────────────────────────────────────────────
const APIC_TIMER_PERIODIC: u32 = 1 << 17;
const APIC_SPURIOUS_ENABLE: u32 = 1 << 8;

// ── xAPIC virtual MMIO base (used only in xAPIC fallback) ────────────────────
static APIC_VIRT_BASE: AtomicU64 = AtomicU64::new(0);

// ── Low-level I/O ─────────────────────────────────────────────────────────────

#[inline(always)]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags));
    v
}

#[inline(always)]
unsafe fn io_wait() { outb(0x80, 0); }

// ── MSR helpers ───────────────────────────────────────────────────────────────

#[inline(always)]
unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32; let hi: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
    ((hi as u64) << 32) | lo as u64
}

#[inline(always)]
unsafe fn wrmsr(msr: u32, val: u64) {
    asm!("wrmsr", in("ecx") msr, in("eax") val as u32, in("edx") (val >> 32) as u32, options(nomem, nostack, preserves_flags));
}

// ── CPUID helper ──────────────────────────────────────────────────────────────

unsafe fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    let eax: u32; let ebx: u32; let ecx: u32; let edx: u32;
    // rbx is LLVM-reserved; we must save/restore it manually around cpuid.
    asm!(
        "mov {tmp:r}, rbx",
        "cpuid",
        "mov {ebx:e}, ebx",
        "mov rbx, {tmp:r}",
        inout("eax") leaf => eax,
        out("ecx") ecx,
        out("edx") edx,
        ebx = out(reg) ebx,
        tmp = out(reg) _,
        options(nomem, nostack, preserves_flags),
    );
    (eax, ebx, ecx, edx)
}

// ── Unified APIC register access ──────────────────────────────────────────────

unsafe fn apic_write(offset: u32, val: u32) {
    if X2APIC_MODE.load(Ordering::Relaxed) {
        wrmsr(0x800 + (offset >> 4), val as u64);
    } else {
        let base = APIC_VIRT_BASE.load(Ordering::Relaxed);
        core::ptr::write_volatile((base + offset as u64) as *mut u32, val);
    }
}

unsafe fn apic_read(offset: u32) -> u32 {
    if X2APIC_MODE.load(Ordering::Relaxed) {
        rdmsr(0x800 + (offset >> 4)) as u32
    } else {
        let base = APIC_VIRT_BASE.load(Ordering::Relaxed);
        core::ptr::read_volatile((base + offset as u64) as *const u32)
    }
}

unsafe fn apic_send_eoi() {
    if X2APIC_MODE.load(Ordering::Relaxed) {
        wrmsr(0x80B, 0); // x2APIC EOI MSR
    } else {
        let base = APIC_VIRT_BASE.load(Ordering::Relaxed);
        core::ptr::write_volatile((base + APIC_EOI as u64) as *mut u32, 0);
    }
}

// ── 8259 PIC: remap to 0x20–0x2F, then mask all ──────────────────────────────

unsafe fn remap_and_mask_pic() {
    outb(0x20, 0x11); io_wait(); outb(0xA0, 0x11); io_wait();
    outb(0x21, 0x20); io_wait(); outb(0xA1, 0x28); io_wait();
    outb(0x21, 4);    io_wait(); outb(0xA1, 2);    io_wait();
    outb(0x21, 0x01); io_wait(); outb(0xA1, 0x01); io_wait();
    // Mask all IRQs on master PIC EXCEPT IRQ 1 (PS/2 keyboard = bit 1 cleared)
    outb(0x21, 0xFD); io_wait();
    // Mask all IRQs on slave PIC
    outb(0xA1, 0xFF); io_wait();
}

// ── PIT channel 2 delay (~10 ms) ─────────────────────────────────────────────
// PIT runs at 1.193182 MHz; 11932 ticks ≈ 10 ms.

unsafe fn pit_delay_10ms() {
    const TICKS: u16 = 11932;
    outb(0x61, inb(0x61) & 0xFC);     // disable channel-2 gate
    outb(0x43, 0xB2);                  // channel 2, mode 0, lo/hi
    outb(0x42, (TICKS & 0xFF) as u8); io_wait();
    outb(0x42, (TICKS >> 8) as u8);   io_wait();
    outb(0x61, (inb(0x61) & 0xFE) | 0x01); // enable gate
    while inb(0x61) & 0x20 == 0 {}   // poll OUT pin (bit 5)
}

// ── Public init ───────────────────────────────────────────────────────────────

/// Initialize the Local APIC and arm the periodic timer at ~100 Hz.
/// `hhdm_offset` is only used for the xAPIC MMIO fallback path.
pub fn init_apic(hhdm_offset: u64) {
    unsafe {
        // 1. Silence legacy PIC
        remap_and_mask_pic();
        crate::serial_println!("[ZenoOS] Legacy 8259 PIC remapped (0x20–0x2F) and masked.");

        // 2. Probe x2APIC (CPUID leaf 1, ECX bit 21)
        let (_, _, ecx, _) = cpuid(1);
        let has_x2apic = (ecx >> 21) & 1 != 0;

        let apic_base_msr = rdmsr(0x1B);

        if has_x2apic {
            // Enable x2APIC (bit 10) + global enable (bit 11)
            wrmsr(0x1B, apic_base_msr | (1 << 10) | (1 << 11));
            X2APIC_MODE.store(true, Ordering::Relaxed);
            crate::serial_println!("[ZenoOS] x2APIC enabled (MSR-based, no MMIO).");
        } else {
            // xAPIC: global enable (bit 11), map via HHDM
            wrmsr(0x1B, apic_base_msr | (1 << 11));
            let phys = apic_base_msr & 0xFFFF_FFFF_FFFF_F000;
            let virt = hhdm_offset + phys;
            APIC_VIRT_BASE.store(virt, Ordering::Relaxed);
            crate::serial_println!("[ZenoOS] xAPIC MMIO @ phys={:#010x}  virt={:#018x}.", phys, virt);
        }

        // 3. Enable Local APIC via Spurious Interrupt Vector Register
        apic_write(APIC_SPURIOUS, APIC_SPURIOUS_ENABLE | 0xFF);
        crate::serial_println!("[ZenoOS] Local APIC enabled.");

        // 4. Calibrate APIC timer (divide-by-16, count down for ~10 ms)
        apic_write(APIC_TIMER_DIVIDE, 0x3);
        apic_write(APIC_TIMER_INIT, 0xFFFF_FFFF);
        pit_delay_10ms();
        let after = apic_read(APIC_TIMER_CURRENT);
        let ticks_10ms = 0xFFFF_FFFFu32.saturating_sub(after);
        crate::serial_println!("[ZenoOS] APIC timer calibrated: {} ticks / 10 ms.", ticks_10ms);

        // 5. Arm periodic timer at ~100 Hz (every 10 ms), vector 0x20
        apic_write(APIC_TIMER_LVT, APIC_TIMER_PERIODIC | 0x20);
        apic_write(APIC_TIMER_DIVIDE, 0x3);
        apic_write(APIC_TIMER_INIT, ticks_10ms.max(1));
        crate::serial_println!("[ZenoOS] APIC timer armed: ~100 Hz, vector 0x20.");
    }
}

// ── APIC timer ISR (called from assembly stub `apic_timer_stub`) ──────────────

#[no_mangle]
pub extern "C" fn apic_timer_handler() {
    let tick = TICK_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    if tick % 100 == 0 {
        crate::serial_println!("[TICK] {} ms elapsed ({} ticks)", tick * 10, tick);
    }
    unsafe { apic_send_eoi(); }
}
