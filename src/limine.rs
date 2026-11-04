//! Limine Boot Protocol 64-bit definitions (no_std, zero external dependencies)
#![allow(dead_code)]

pub const LIMINE_COMMON_MAGIC: [u64; 2] = [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b];

#[repr(C)]
pub struct BaseRevision {
    pub id: [u64; 2],
    pub revision: u64,
}

impl BaseRevision {
    pub const fn new(revision: u64) -> Self {
        Self {
            id: [0xf9562b2d5c95a6c8, 0x6a7b384944536bdc],
            revision,
        }
    }

    pub fn is_supported(&self) -> bool {
        unsafe { core::ptr::read_volatile(&self.revision) == 0 }
    }
}

// Start & End markers for Limine request scanning
#[repr(C)]
pub struct RequestsStartMarker {
    pub id: [u64; 4],
}

impl RequestsStartMarker {
    pub const fn new() -> Self {
        Self {
            id: [
                0xf6b8f4b39de7d1ae,
                0xfab91a6940fcb9cf,
                0x785c6ed015d3e316,
                0x181e920a7852b9d9,
            ],
        }
    }
}

#[repr(C)]
pub struct RequestsEndMarker {
    pub id: [u64; 2],
}

impl RequestsEndMarker {
    pub const fn new() -> Self {
        Self {
            id: [0xadc0e0531bb10d03, 0x9572709f31764c62],
        }
    }
}

// Framebuffer Request
#[repr(C)]
pub struct FramebufferRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *const FramebufferResponse,
}

unsafe impl Sync for FramebufferRequest {}

impl FramebufferRequest {
    pub const fn new() -> Self {
        Self {
            id: [
                LIMINE_COMMON_MAGIC[0],
                LIMINE_COMMON_MAGIC[1],
                0x9d5827dcd881dd75,
                0xa3148604f6fab11b,
            ],
            revision: 0,
            response: core::ptr::null(),
        }
    }

    pub fn response(&self) -> Option<&'static FramebufferResponse> {
        let resp_ptr = unsafe { core::ptr::read_volatile(&self.response) };
        if resp_ptr.is_null() {
            None
        } else {
            unsafe { Some(&*resp_ptr) }
        }
    }
}

#[repr(C)]
pub struct FramebufferResponse {
    pub revision: u64,
    pub framebuffer_count: u64,
    pub framebuffers: *const *const Framebuffer,
}

impl FramebufferResponse {
    pub fn framebuffers(&self) -> &'static [&'static Framebuffer] {
        let count = unsafe { core::ptr::read_volatile(&self.framebuffer_count) };
        let fbs = unsafe { core::ptr::read_volatile(&self.framebuffers) };
        if count == 0 || fbs.is_null() {
            &[]
        } else {
            unsafe {
                let slice = core::slice::from_raw_parts(fbs, count as usize);
                core::mem::transmute(slice)
            }
        }
    }
}

#[repr(C)]
pub struct Framebuffer {
    pub address: *mut u8,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
    pub unused: [u8; 7],
    pub edid_size: u64,
    pub edid: *const u8,
}

// Higher Half Direct Map (HHDM) Request
#[repr(C)]
pub struct HhdmRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *const HhdmResponse,
}

unsafe impl Sync for HhdmRequest {}

impl HhdmRequest {
    pub const fn new() -> Self {
        Self {
            id: [
                LIMINE_COMMON_MAGIC[0],
                LIMINE_COMMON_MAGIC[1],
                0x48dcf1cb8ad2b852,
                0x63984e959a98244b,
            ],
            revision: 0,
            response: core::ptr::null(),
        }
    }

    pub fn response(&self) -> Option<&'static HhdmResponse> {
        let resp_ptr = unsafe { core::ptr::read_volatile(&self.response) };
        if resp_ptr.is_null() {
            None
        } else {
            unsafe { Some(&*resp_ptr) }
        }
    }
}

#[repr(C)]
pub struct HhdmResponse {
    pub revision: u64,
    pub offset: u64,
}

// Memory Map Request
#[repr(C)]
pub struct MemmapRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *const MemmapResponse,
}

unsafe impl Sync for MemmapRequest {}

impl MemmapRequest {
    pub const fn new() -> Self {
        Self {
            id: [
                LIMINE_COMMON_MAGIC[0],
                LIMINE_COMMON_MAGIC[1],
                0x67cf3d9d378a806f,
                0xe304acdfc50c3c62,
            ],
            revision: 0,
            response: core::ptr::null(),
        }
    }

    pub fn response(&self) -> Option<&'static MemmapResponse> {
        let resp_ptr = unsafe { core::ptr::read_volatile(&self.response) };
        if resp_ptr.is_null() {
            None
        } else {
            unsafe { Some(&*resp_ptr) }
        }
    }
}

#[repr(C)]
pub struct MemmapResponse {
    pub revision: u64,
    pub entry_count: u64,
    pub entries: *const *const MemmapEntry,
}

impl MemmapResponse {
    pub fn entries(&self) -> &'static [&'static MemmapEntry] {
        let count = unsafe { core::ptr::read_volatile(&self.entry_count) };
        let entries = unsafe { core::ptr::read_volatile(&self.entries) };
        if count == 0 || entries.is_null() {
            &[]
        } else {
            unsafe {
                let slice = core::slice::from_raw_parts(entries, count as usize);
                core::mem::transmute(slice)
            }
        }
    }
}

#[repr(C)]
pub struct MemmapEntry {
    pub base: u64,
    pub length: u64,
    pub entry_type: u64,
}

pub const LIMINE_MEMMAP_USABLE: u64 = 0;
pub const LIMINE_MEMMAP_RESERVED: u64 = 1;
pub const LIMINE_MEMMAP_ACPI_RECLAIMABLE: u64 = 2;
pub const LIMINE_MEMMAP_ACPI_NVS: u64 = 3;
pub const LIMINE_MEMMAP_BAD_MEMORY: u64 = 4;
pub const LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE: u64 = 5;
pub const LIMINE_MEMMAP_KERNEL_AND_MODULES: u64 = 6;
pub const LIMINE_MEMMAP_FRAMEBUFFER: u64 = 7;
