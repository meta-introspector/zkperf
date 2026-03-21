.PHONY: build run test clean

build:
	nix develop --command cargo build --release

run:
	nix develop --command cargo run --release -- "$(Q)" $(N)

test:
	nix develop --command cargo run --release -- "moltis ai assistant" 3

clean:
	nix develop --command cargo clean

lock:
	nix develop --command cargo generate-lockfile
