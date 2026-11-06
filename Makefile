GMOD_DIR ?= $(HOME)/.local/share/Steam/steamapps/common/GarrysMod
BIN_DIR  := $(GMOD_DIR)/garrysmod/lua/bin
EXAMPLE_DIR := $(GMOD_DIR)/garrysmod/addons/steaminput_example

TARGET   := x86_64-pc-windows-gnu
PROFILE  ?= release
MODULE   := gmcl_steaminput_win64.dll

ifeq ($(PROFILE),release)
CARGO_FLAGS := --release
endif

BUILT := target/$(TARGET)/$(PROFILE)/steaminput.dll

.PHONY: all build install example debug clean uninstall

all: install

build:
	cargo build $(CARGO_FLAGS)

install: build example
	@test -d "$(BIN_DIR)" || mkdir -p "$(BIN_DIR)"
	cp "$(BUILT)" "$(BIN_DIR)/.$(MODULE).tmp"
	mv -f "$(BIN_DIR)/.$(MODULE).tmp" "$(BIN_DIR)/$(MODULE)"
	@echo "Installed $(BIN_DIR)/$(MODULE)"

example:
	@if [ -e "$(EXAMPLE_DIR)" ] && [ ! -L "$(EXAMPLE_DIR)" ]; then echo "$(EXAMPLE_DIR) exists and is not a symlink"; exit 1; fi
	ln -sfn "$(CURDIR)/example" "$(EXAMPLE_DIR)"

debug:
	$(MAKE) install PROFILE=debug

uninstall:
	rm -f "$(BIN_DIR)/$(MODULE)"
	@if [ -L "$(EXAMPLE_DIR)" ]; then rm "$(EXAMPLE_DIR)"; fi

clean:
	cargo clean
