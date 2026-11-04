CARGO ?= $(HOME)/.cargo/bin/cargo
KERNEL := target/x86_64-unknown-none/release/zenoos
ISO := zenoos.iso
ISO_DIR := build/iso_root

# Detect Limine assets from Homebrew or local clone
LIMINE_SHARE ?= $(shell brew --prefix limine 2>/dev/null)/share/limine
LIMINE_BIN ?= $(shell which limine 2>/dev/null)

.PHONY: all build iso run clean

all: iso

build:
	$(CARGO) build --release

iso: build
	@mkdir -p $(ISO_DIR)/boot $(ISO_DIR)/boot/limine $(ISO_DIR)/EFI/BOOT
	@cp $(KERNEL) $(ISO_DIR)/boot/zenoos
	@cp limine.conf $(ISO_DIR)/boot/limine/limine.conf
	@cp limine.conf $(ISO_DIR)/limine.conf
	@# Copy Limine bootloader files
	@if [ -d "$(LIMINE_SHARE)" ]; then \
		cp $(LIMINE_SHARE)/limine-bios.sys $(ISO_DIR)/boot/limine/; \
		cp $(LIMINE_SHARE)/limine-bios-cd.bin $(ISO_DIR)/boot/limine/; \
		cp $(LIMINE_SHARE)/limine-uefi-cd.bin $(ISO_DIR)/boot/limine/; \
		cp $(LIMINE_SHARE)/BOOTX64.EFI $(ISO_DIR)/EFI/BOOT/; \
	elif [ -d "limine-bin" ]; then \
		cp limine-bin/limine-bios.sys $(ISO_DIR)/boot/limine/; \
		cp limine-bin/limine-bios-cd.bin $(ISO_DIR)/boot/limine/; \
		cp limine-bin/limine-uefi-cd.bin $(ISO_DIR)/boot/limine/; \
		cp limine-bin/BOOTX64.EFI $(ISO_DIR)/EFI/BOOT/; \
	else \
		echo "[-] Limine assets not found! Please run: brew install limine"; \
		exit 1; \
	fi
	@# Create hybrid ISO using xorriso
	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		$(ISO_DIR) -o $(ISO)
	@# Install BIOS bootloader to ISO
	@if [ -x "$(LIMINE_BIN)" ]; then \
		$(LIMINE_BIN) bios-install $(ISO); \
	elif [ -x "limine-bin/limine" ]; then \
		./limine-bin/limine bios-install $(ISO); \
	fi
	@echo "[+] Successfully built $(ISO)!"

run: iso
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio -m 512M

clean:
	$(CARGO) clean
	rm -rf build $(ISO)
