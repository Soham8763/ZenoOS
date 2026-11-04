# ZenoOS - Master Engineering Documentary & Living Context Anchor

> **Instructions for AI Agents:**
> Read this file first before taking any action. This document is a living chronological documentary and architectural memory of **ZenoOS**.
> Whenever any feature, bug fix, or milestone is completed, append a new documentary log entry with the exact date, time, phase number, and technical details.

---

## 1. Project Identity & Architectural Manifesto

* **Project Name:** ZenoOS
* **Core Philosophy:** Ultra-lightweight, memory-safe, future-proof, 64-bit operating system engineered from bare metal in **Rust (`#![no_std]`)**.
* **Target Deployment:** Runs directly on bare-metal x86_64 hardware (initially a repurposed laptop acting as a headless/network-accessible server) with long-term architecture designed for embedded security hardware, smart appliances, and tablets.
* **Boot Architecture:** Modern **Limine Boot Protocol v6+** supporting both UEFI (64-bit) and legacy BIOS.
* **Security & Concurrency:** Compile-time memory safety, strict lock hierarchies, interrupt-safe spinlocks (zero runtime deadlock detection overhead).
* **AI Architecture:** Autonomous AI daemon ("Jarvis") operating in **Userspace (Ring 3)** over structured capability-gated IPC channels (never in kernel space).
* **Storage Constraint:** Tailored Cargo profile (`debug = 1`, `incremental = false`) keeping total build cache strictly under **300 MB** (current build cache: **< 100 KB**, binary size: **13 KB**).

---

## 2. Chronological Engineering Documentary & Logbook

### 📅 Entry 1: 2026-11-04 01:30 IST — Project Audit & Legacy Analysis
* **Phase:** Phase 0 (Pre-study & Codebase Audit)
* **Starting State:** 
  * Repository had 3 commits: `printing hello world`, `first commit`, and `Reading from a memory`.
  * Implemented an experimental 16-bit x86 Real-Mode bootloader (`src/bootloader/boot.asm`) and stub kernel (`src/kernel/main.asm`) following the Nanobyte tutorial.
  * Executed via BIOS interrupt `int 13h` reading 512-byte sectors from a 1.44MB FAT12 floppy disk image.
* **Findings & Architectural Dead-End:**
  * 16-bit Real Mode addresses a maximum of 1 MB of RAM (640 KB conventional memory), making it physically incapable of running modern workloads, network stacks, or local AI/LLM models.
  * Modern laptop hardware uses 64-bit UEFI firmware; 16-bit floppy BIOS code cannot boot on real modern machines without legacy CSM (which is now omitted in modern chipsets).
  * 100% assembly codebase is tightly locked to legacy x86 and cannot be ported to ARM64 or RISC-V devices.
* **The "Larp" Concept:**
  * Coined the project lore around **"Zeno's Bootloader Paradox"** (Achilles and the Tortoise): an OS that computes 1980s magnetic cylinder-head-sector track geometry where each cluster hop halves the remaining distance to userspace.

---

### 📅 Entry 2: 2026-11-04 02:20 - 03:20 IST — Architectural Pivot & Specification
* **Phase:** Phase 0 (Requirements & Blueprint Formulation)
* **Vision Stated:** Universal base for laptops, tablets, smart devices, ethical security penetration testing, and integration with a local Ollama LLM.
* **Strategic Decisions Approved:**
  1. **Pivot to Rust (`#![no_std]`):** Guarantees memory safety and eliminates ~70% of historical kernel CVEs (buffer overflows, use-after-free, dangling pointers).
  2. **Adopt Limine Bootloader:** Discard custom 16-bit floppy bootloader. Limine drops the kernel directly into 64-bit Long Mode with Higher-Half Direct Map (HHDM), linear framebuffer, and memory map on both UEFI and BIOS.
  3. **Userspace AI Daemon Architecture:** Keep the kernel microscopic and deterministic. Run the AI agent in Ring 3 as a system daemon communicating via capability syscalls.
* **Deliverables Created:**
  * [`PRD.md`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/PRD.md): 6-phase engineering specification with strict verification checkpoints ("No Hadbadi").
  * [`memory.md`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/memory.md): Master context anchor for token conservation across AI sessions.

---

### 📅 Entry 3: 2026-11-04 03:30 - 03:45 IST — macOS Host Development Setup
* **Phase:** Phase 1 Setup (Toolchain & Cross-Compilation)
* **Environment:** macOS (Apple Silicon aarch64 host targeting `x86_64-unknown-none`).
* **Hurdle Encountered:**
  * Running the official `rustup` installer failed with `Permission denied (os error 13)` on `/Users/soham/.bash_profile`.
* **Root Cause & Fix:**
  * `/Users/soham/.bash_profile` was owned by `root` from an earlier `sudo` command.
  * Resolved by restoring file ownership: `sudo chown $(whoami) ~/.bash_profile`.
* **Toolchain Successfully Installed:**
  * Rust Compiler & Cargo (`cargo 1.99.1`, `rustc 1.99.1`).
  * Target `x86_64-unknown-none` and component `llvm-tools-preview`.
  * Verified QEMU (`qemu-system-x86_64 10.0.0`) and `xorriso 1.5.8`.

---

### 📅 Entry 4: 2026-11-04 03:48 - 03:52 IST — Phase 1 Codebase Construction
* **Phase:** Phase 1 (64-Bit Kernel Bootstrap)
* **Actions Taken:**
  * Archived legacy 16-bit assembly files into [`legacy/`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/legacy).
  * Configured `.cargo/config.toml` targeting `x86_64-unknown-none` with kernel code model.
  * Configured `Cargo.toml` with low-disk-space optimizations (`debug = 1`, `incremental = false`).
  * Implemented [`linker.ld`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/linker.ld): Higher-half 64-bit ELF linker script at `0xffffffff80000000`.
  * Implemented [`src/limine.rs`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/src/limine.rs): Standalone Limine boot protocol request/response structures in pure Rust with **zero external crate dependencies**.
  * Implemented [`src/serial.rs`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/src/serial.rs): 16550 UART serial driver (`COM1: 0x3F8`) streaming logs to host terminal.
  * Implemented [`src/font.rs`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/src/font.rs) & [`src/writer.rs`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/src/writer.rs): 32-bit linear framebuffer console renderer with embedded 8x8 font.
  * Implemented [`src/sync.rs`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/src/sync.rs): Interrupt-safe atomic `Spinlock<T>`.
  * Implemented [`src/main.rs`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/src/main.rs): `_start()` kernel entry point and panic handler.
  * Implemented [`Makefile`](file:///Users/soham/Desktop/Developer/Code/ZenoOS/Makefile): Automated ISO build using `xorriso` and `limine bios-install`.
* **Build Metric:** Compiled kernel binary size: **13 KB**; `target/` cache: **84 KB**.

---

### 📅 Entry 5: 2026-11-04 03:55 - 04:10 IST — Debugging the Bootloader Handshake
* **Phase:** Phase 1 Verification & Hardening
* **Symptom:**
  * First QEMU test halted with `[ZenoOS FATAL] Limine base revision unsupported!` and showed a blank/black screen.
* **Deep-Dive Investigation & Root Causes:**
  1. **Compiler Constant Folding:** Because `static BASE_REVISION = BaseRevision::new(3)` was a standard Rust static, `rustc` saw `revision = 3` at compile-time and constant-folded `BASE_REVISION.is_supported()` directly into `false`. When Limine wrote `0` into that memory at runtime, the CPU never re-read the updated memory value.
  2. **Uncached Request Responses:** `FRAMEBUFFER_REQUEST.response`, `HHDM_REQUEST.response`, and `MEMMAP_REQUEST.response` suffered the same constant-folding, causing the kernel to treat them as `None`.
  3. **Print Macro Clashing:** `serial_println!` and `kprintln!` were wrapping arguments inside nested `format_args!`, causing parameter register clobbering.
  4. **Linker Section Bounding:** The request markers and `.requests` section required explicit alignment and preservation directives in `linker.ld` so Limine could locate them between `requests_start_marker` and `requests_end_marker`.
* **Engineered Fixes:**
  * Implemented `core::ptr::read_volatile` across all Limine accessor methods (`is_supported()`, `response()`, `entries()`, `framebuffers()`).
  * Updated request UUID magic numbers to strictly adhere to the Limine v8/v12 specification.
  * Rewrote `serial_println!` and `kprintln!` to avoid nested `format_args!` calls.
  * Bound request sections in `linker.ld` using `ALIGN(8)` and `KEEP()`.

---

### 📅 Entry 6: 2026-11-04 04:10 - 04:20 IST — Phase 1 Fully Verified & Operational
* **Phase:** Phase 1 Sign-Off
* **Test Command:** `make run`
* **Verified Runtime Output:**
  ```text
  [ZenoOS] Serial port initialized successfully.
  [ZenoOS] Limine protocol handshake confirmed (Base Revision 3).
  [ZenoOS] Framebuffer acquired: 1280x800 @ 32 bpp
  ============================================================
                   ZENO-OS :: BARE-METAL KERNEL                
           64-Bit Modular OS Core (Rust no_std / Limine)      
  ============================================================

  [+] Display:  1280x800 (32 bpp linear framebuffer)
  [+] HHDM:     Direct physical mapping at offset 0xffff800000000000
  [+] Memory:   510 MB usable RAM (out of 12803 MB total physical RAM, 14 entries)

  [*] System State: 64-bit Long Mode active. Halting CPU idle loop.
  ============================================================
  ```
* **Milestone Achievements:**
  * Limine boot menu displays cleanly with 3-second countdown timer (`timeout: 3` in `limine.conf`).
  * Seamless transition from bootloader to kernel.
  * Linear Framebuffer pixel rendering working at **1280x800 @ 32 bpp**.
  * Higher-Half Direct Map (HHDM) mapped to `0xffff800000000000`.
  * Physical memory mapped: **510 MB usable RAM** discovered.
  * CPU placed in low-power idle sleep (`hlt`) with 0% runaway host CPU utilization.
* **Phase 1 Status:** **100% COMPLETE & VERIFIED.**

---

## 3. Phase-Wise Progress Tracker

- [x] **Phase 0: Pre-study & Architectural Pivot** *(Completed: 2026-11-04)*
  - [x] Evaluated initial 16-bit real-mode FAT12 assembly bootloader.
  - [x] Approved pivot to 64-bit Rust (`no_std`) + Limine bootloader.
  - [x] Formulated PRD and Master Memory context.

- [x] **Phase 1: 64-bit Kernel Bootstrap & Limine Integration** *(Completed: 2026-11-04)*
  - [x] Initialize Cargo project with `x86_64-unknown-none` target and low-storage profile.
  - [x] Configure pure `no_std` `limine` protocol requests (Base Revision, Framebuffer, HHDM, Memmap).
  - [x] Implement `_start()` kernel entry point in Rust (`src/main.rs`).
  - [x] Implement UART 16550 Serial logging (`src/serial.rs`) and Framebuffer console writer (`src/writer.rs`, `src/font.rs`).
  - [x] Implement interrupt-safe `Spinlock<T>` (`src/sync.rs`).
  - [x] Create automated build script (`Makefile`) producing a hybrid UEFI/BIOS ISO.
  - [x] Verified full boot in QEMU: 1280x800 @ 32bpp framebuffer, HHDM mapping, and memory map enumeration.

- [ ] **Phase 2: Core Hardware & Interrupt Subsystem** *(UP NEXT)*
  - [ ] Global Descriptor Table (GDT) & Task State Segment (TSS) for double fault stack isolation.
  - [ ] Interrupt Descriptor Table (IDT) for all 32 x86 CPU exceptions.
  - [ ] Exception handlers with formatted crash dumps (Divide by Zero, Page Fault, GPF).
  - [ ] Disable legacy 8259 PIC; map and initialize Local APIC.
  - [ ] APIC Timer calibration for periodic system ticks.
  - [ ] PS/2 Keyboard interrupt driver for interactive console typing.

- [ ] **Phase 3: Physical & Virtual Memory Management**
  - [ ] Physical Frame Allocator (Bitmap Allocator from Limine memory map).
  - [ ] Virtual Memory Paging manager using Limine HHDM.
  - [ ] Kernel Heap Allocator (`linked_list_allocator`) enabling `alloc` (`Box`, `Vec`, `String`).

- [ ] **Phase 4: Preemptive Multitasking & Context Switching**
  - [ ] Process/Thread control block (TCB).
  - [ ] Assembly context switcher (`switch_context`).
  - [ ] Preemptive Round-Robin scheduler driven by APIC timer.
  - [ ] Synchronization primitives (Mutexes, Channels).

- [ ] **Phase 5: PCI Enumeration, Drivers & Networking**
  - [ ] PCI Bus scanner to identify Ethernet NIC (Intel e1000 / Realtek RTL8139).
  - [ ] Network card driver with DMA descriptor rings.
  - [ ] Integration of `smoltcp` in `#![no_std]` mode.
  - [ ] Embedded HTTP / Echo server listening on LAN.

- [ ] **Phase 6: Userspace, Capabilities & Jarvis Daemon Interface**
  - [ ] User mode transition (Ring 3) via `syscall` / `sysret`.
  - [ ] Capability-based security model.
  - [ ] IPC socket for privileged system services.
  - [ ] Interface to local Ollama / LLM agent.

---

## 4. Current Directory Structure
```text
ZenoOS/
├── .cargo/
│   └── config.toml          # Target: x86_64-unknown-none + kernel code model
├── legacy/                  # Archived 16-bit FAT12 assembly bootloader
│   ├── bootloader/boot.asm
│   ├── kernel/main.asm
│   ├── bochs_config
│   ├── debug.sh
│   ├── run.sh
│   └── test.img
├── src/
│   ├── font.rs              # Embedded 8x8 ASCII glyph bitmap font
│   ├── limine.rs            # Pure Rust Limine protocol definitions
│   ├── main.rs              # 64-bit kernel entry point (_start) & panic handler
│   ├── serial.rs            # 16550 UART serial driver (COM1: 0x3F8)
│   ├── sync.rs              # Atomic interrupt-safe Spinlock<T>
│   └── writer.rs            # Graphical framebuffer console writer (1280x800)
├── Cargo.toml               # Rust dependencies and low-storage profile (< 100 KB target)
├── Makefile                 # Automated build & ISO generator
├── PRD.md                   # Complete Product Requirements Document
├── limine.conf              # Bootloader configuration (timeout & kernel path)
├── linker.ld                # Higher-half 64-bit ELF linker script
└── memory.md                # Living documentary & context anchor (this file)
```

---

## 5. Instructions for Future AI Agents
1. **Never guess past architectural decisions** — Refer to the logbook above.
2. **Never add external dependencies without checking storage impact** — Keep build artifacts minimal.
3. **Always verify code via QEMU before marking any step complete.**
4. **Append a new timestamped log entry to Section 2 whenever you complete a task or phase.**
