# ZenoOS - Product Requirements Document (PRD) & Engineering Specification

## 1. Executive Summary & Vision

**ZenoOS** is an ultra-lightweight, memory-safe, 64-bit operating system engineered from bare metal using **Rust (`#![no_std]`)**.
The project aims to serve as a high-performance, future-proof software foundation capable of running on repurposed physical hardware (e.g., an x86_64 laptop acting as a persistent personal server) with architectural pathways to smart appliances, tablets, and embedded security hardware.

Unlike traditional bloated operating systems, ZenoOS strips away legacy baggage, booting in **under 50 milliseconds**, enforcing **zero-cost memory safety**, and natively exposing structured Inter-Process Communication (IPC) interfaces designed for autonomous AI agents and ethical security auditing.

---

## 2. Core Principles & Non-Functional Requirements

### 2.1 Design Philosophy: "No Hadbadi" (Disciplined Engineering)
* **Modular Layering:** No layer depends on a layer above it. (Hardware -> Memory -> Interrupts -> Scheduler -> Networking -> Userspace).
* **Strict Verification Gates:** Every phase must pass deterministic hardware or emulator verification tests before the next phase begins.
* **Deterministic Behavior:** Panic handlers report exact stack traces and CPU register states; no silent fails.

### 2.2 Non-Functional Metrics
* **Boot Time:** < 50ms from firmware handover to the first userspace task.
* **Kernel Memory Footprint:** < 16 MB baseline RAM usage.
* **Disk Space Constraints (Development Host):**
  * Cargo build cache restricted to < 300 MB through tailored profiles (`debug = 1`, `incremental = false`).
  * Final bootable ISO size < 15 MB.
* **Reliability:** Zero kernel panics resulting from memory safety bugs (guaranteed by Rust compiler invariants).

---

## 3. System Architecture & Component Breakdown

```
+---------------------------------------------------------------+
|                      Userspace (Ring 3)                       |
|   +-----------------------+     +-------------------------+   |
|   |  Jarvis Daemon (AI)   |     | HTTP Server / Utilities |   |
|   +-----------+-----------+     +------------+------------+   |
+---------------|------------------------------|----------------+
|               | Custom Capability Syscalls   |                |
+---------------v------------------------------v----------------+
|                      Kernel Space (Ring 0)                    |
|  +---------------------------------------------------------+  |
|  |                 Task Scheduler (Round-Robin)            |  |
|  +---------------------------------------------------------+  |
|  |           Network Stack (smoltcp + NIC Drivers)         |  |
|  +---------------------------------------------------------+  |
|  |      Memory Subsystem (Paging, Frame Allocator, Heap)   |  |
|  +---------------------------------------------------------+  |
|  |         Interrupts & Exceptions (IDT, GDT, APIC)        |  |
|  +---------------------------------------------------------+  |
|  |             HAL & Boot Protocol Layer (Limine)          |  |
+---------------------------------------------------------------+
|                       Bare Metal Hardware                     |
+---------------------------------------------------------------+
```

### 3.1 Bootloader Subsystem (Limine)
* **Protocol:** Limine Boot Protocol v6+.
* **Boot Modes:** UEFI (64-bit) and legacy BIOS compatibility out-of-the-box.
* **Handoff State:**
  * 64-bit Long Mode enabled.
  * Paging enabled with **Higher Half Direct Map (HHDM)**: Physical address `P` is mapped to virtual address `P + HHDM_OFFSET`.
  * Memory map provided with classified zones (Usable, Reserved, ACPI, Bootloader Reclaimable).
  * Linear graphical framebuffer populated with width, height, pitch, and BPP (32-bit RGBA).

### 3.2 Memory Management Subsystem
1. **Physical Frame Allocator:**
   * Tracks 4 KB physical pages using a memory-efficient Bitmap Allocator.
   * Allocates contiguous physical memory for DMA (Direct Memory Access) buffers required by network cards.
2. **Virtual Memory Manager (VMM):**
   * Implements 4-level x86_64 paging (PML4, PDPT, PD, PT).
   * Maps kernel text/data into higher-half address space with NX (No-Execute) and Write-Protect bits enforced.
3. **Kernel Heap Allocator:**
   * Global allocator implementing Rust's `core::alloc::GlobalAlloc`.
   * Enables dynamic data structures (`Vec`, `String`, `Box`, `BTreeMap`).

### 3.3 Interrupt & Timing Subsystem
* **GDT (Global Descriptor Table):** Configures Ring 0 kernel code/data segments and Ring 3 user code/data segments, plus a 64-bit TSS (Task State Segment) with dedicated IST (Interrupt Stack Table) for Double Faults.
* **IDT (Interrupt Descriptor Table):** 256 interrupt gates handling:
  * CPU Exceptions (Divide error, Page fault, Double fault, General protection fault).
  * Hardware IRQs routed through the I/O APIC and Local APIC (legacy PIC is masked/disabled).
* **APIC Timer:** High-precision periodic timer for task preemption.

### 3.4 Concurrency & Deadlock Prevention
* **No Runtime Deadlock Detection Overhead:** Deadlock prevention is designed into the codebase:
  1. **Lock Hierarchy:** Locks must always be acquired in a strictly documented order.
  2. **Interrupt-Disabling Spinlocks:** Any spinlock used inside or shared with interrupt handlers automatically disables interrupts while held (`without_interrupts`).
  3. **Atomic Primitives:** Frequent use of atomic ring buffers (SPSC queues) for lock-free communication between drivers and worker threads.

### 3.5 Networking Subsystem
* **Bus Architecture:** PCI bus scan to enumerate attached hardware devices.
* **Target Network Controllers:**
  * *Emulation/Standard:* Intel e1000 (82540EM) and Realtek RTL8139.
  * *Bare-metal Laptop:* Realtek RTL8111/8168/8411 PCI Express Gigabit Ethernet controller (standard on most older laptops).
* **TCP/IP Stack:** Integration of `smoltcp` in `#![no_std]` mode, providing ARP, IPv4, TCP, and UDP sockets.

### 3.6 Autonomous AI & Security Interface ("Jarvis")
* **Architecture:** The AI does **not** live in kernel space (preserving crash-safety and determinism). It runs as a privileged user-level system daemon (`zenod`).
* **System Call Interface:**
  * `sys_net_listen(port)`: Bind server ports for remote interaction.
  * `sys_audit_log(event)`: Capture security metrics, dropped packets, and unauthorized access attempts.
  * `sys_packet_inject(raw_frame)`: Capability-gated raw packet transmission for network auditing and penetration testing.
* **Remote Management:** Minimal HTTP/TLS or authenticated lightweight RPC listener allowing remote control from anywhere over the internet.

---

## 4. Phase-Wise Implementation Roadmap

### Phase 1: Bare-Metal Kernel Bootstrap
* **Objective:** Establish the Rust development pipeline, compile for `x86_64-unknown-none`, and successfully boot a Limine ISO in QEMU.
* **Deliverables:**
  * `.cargo/config.toml` targeting `x86_64-unknown-none`.
  * `Cargo.toml` with low-disk-space optimizations.
  * Kernel entry point `_start()` in Rust receiving Limine bootloader responses.
  * Basic linear Framebuffer terminal writer (printing text to the screen).
  * Serial port driver (`COM1` at `0x3F8`) for instant terminal debugging.
* **Verification Gate:** QEMU boots `zenoos.iso`, displays color text on both the graphical window and the serial console, and enters a clean idle loop (`hlt`).

### Phase 2: Descriptors, Exceptions & APIC Timer
* **Objective:** Ensure the CPU handles CPU faults gracefully and calibrate the hardware timer for multitasking.
* **Deliverables:**
  * GDT initialization with 64-bit kernel and user segment descriptors.
  * TSS setup with an IST stack to catch stack overflows and double faults.
  * IDT setup covering all 32 x86 CPU exceptions with formatted crash dumps.
  * Legacy 8259 PIC disabled; Local APIC mapped and calibrated.
* **Verification Gate:** Deliberately triggering a breakpoint (`int3`) and page fault recovers cleanly; APIC timer fires regular ticks to serial output.

### Phase 3: Memory Subsystem (Physical, Virtual, Heap)
* **Objective:** Provide robust dynamic memory management (`alloc`).
* **Deliverables:**
  * Physical memory parsing from Limine memory map.
  * Bitmap-based physical page frame allocator.
  * Page table manipulation functions (mapping, unmapping, flags update).
  * Kernel heap allocator (`linked_list_allocator` or slab) initializing a 4 MB initial heap.
* **Verification Gate:** Allocating dynamic structures (`Vec<u8>`, `Box<[u64]>`, `String`) works without panics, and out-of-memory states are gracefully trapped.

### Phase 4: Preemptive Multitasking & Synchronization
* **Objective:** Execute concurrent kernel tasks with preemptive scheduling.
* **Deliverables:**
  * Task Control Block (TCB) structure containing saved register states and separate stacks.
  * Assembly context switch routine (`switch_context(prev_tcb, next_tcb)`).
  * Preemptive Round-Robin scheduler driven by APIC timer interrupts.
  * Interrupt-safe `Spinlock<T>` and cooperative `yield_now()`.
* **Verification Gate:** Two independent kernel worker tasks increment counters and print interleaving messages to the console smoothly without locking up.

### Phase 5: PCI Enumeration, Drivers & Networking
* **Objective:** Connect the OS to local networks and serve data.
* **Deliverables:**
  * PCI configuration space reader/writer enumerating bus 0..256.
  * Network Interface Card (NIC) driver: Intel e1000 and Realtek RTL8139.
  * Buffer descriptor queues for TX (transmit) and RX (receive) using DMA.
  * `smoltcp` integration into kernel event loop.
  * Embedded HTTP / Echo server listening on port 80 / 8080.
* **Verification Gate:** Pinging ZenoOS from the host macOS terminal succeeds (`ping 192.168...`), and `curl http://<ip>:8080` returns an HTML server status page.

### Phase 6: Ring 3 Userspace, Security Capabilities & Jarvis AI Daemon
* **Objective:** Separate kernel from user tasks and deploy the remote management AI service.
* **Deliverables:**
  * Hardware MSR configuration for fast system calls (`syscall` / `sysret`).
  * Process isolation with unique PML4 address spaces for userspace binaries.
  * Capability security tokens granting permission to bind ports or inspect network frames.
  * Integration with local LLM endpoint (Ollama daemon on the network or laptop).
* **Verification Gate:** Userspace process runs in Ring 3; attempting unauthorized kernel access triggers General Protection Fault; Jarvis service receives and executes remote authenticated commands.

---

## 5. Development & Deployment Workflow

### 5.1 Host Setup (macOS)
* Cross-compiles natively using standard Rust `cargo` without requiring a full Linux VM.
* Builds ISOs using `xorriso` and Limine deploy binaries.
* Tests in milliseconds using Homebrew's `qemu-system-x86_64`.

### 5.2 Real Hardware Target (Old Laptop)
* **Storage:** USB flash drive formatted with Limine UEFI/BIOS dual-boot image.
* **Installation:** Boot laptop via USB (F12/BIOS boot menu) into ZenoOS.
* **Connectivity:** Standard RJ45 Ethernet cable connected to home router for fixed DHCP IP address.
* **Maintenance:** Headless operation—console output forwarded to serial or network socket.

