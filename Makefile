.PHONY: all build check test install clean

all: build

build:
	cargo build --release

check:
	cargo check

test:
	cargo test

install:
	./install.sh

clean:
	cargo clean
