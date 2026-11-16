//! Architecture-specific code for x86_64.
//! Re-exports the three hardware subsystems so `main.rs` can use them as
//! `arch::x86_64::gdt::init_gdt()`, etc.

pub mod apic;
pub mod gdt;
pub mod idt;

