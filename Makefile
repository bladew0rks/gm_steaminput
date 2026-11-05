# Builds the module for Windows (GMod runs under Proton) and installs it into lua/bin.
# Override the install location with: make GMOD_DIR=/path/to/GarrysMod

GMOD_DIR ?= $(HOME)/.local/share/Steam/steamapps/common/GarrysMod
BIN_DIR  := $(GMOD_DIR)/garrysmod/lua/bin

TARGET   := x86_64-pc-windows-gnu
PROFILE  ?= release
# Steam Input is client-side, so this is a gmcl_ module. _win64 matches the x86-64 branch.
MODULE   := gmcl_steaminput_win64.dll

ifeq ($(PROFILE),release)
CARGO_FLAGS := --release
endif

BUILT := target/$(TARGET)/$(PROFILE)/steaminput.dll

.PHONY: all build install debug clean uninstall

all: install

build:
	cargo build $(CARGO_FLAGS)

# Copy to a temp file then rename, so a running game holding the old DLL isn't corrupted.
install: build
	@test -d "$(BIN_DIR)" || mkdir -p "$(BIN_DIR)"
	cp "$(BUILT)" "$(BIN_DIR)/.$(MODULE).tmp"
	mv -f "$(BIN_DIR)/.$(MODULE).tmp" "$(BIN_DIR)/$(MODULE)"
	@echo "Installed $(BIN_DIR)/$(MODULE)"

debug:
	$(MAKE) install PROFILE=debug

uninstall:
	rm -f "$(BIN_DIR)/$(MODULE)"

clean:
	cargo clean
