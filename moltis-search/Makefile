.PHONY: build run test clean lock

build:
	nix develop --command cargo build --release

run:
	nix develop --command cargo run --release -- "$(Q)" witnesses

test:
	nix develop --command cargo run --release -- "moltis ai assistant" witnesses

clean:
	nix develop --command cargo clean
	rm -rf witnesses

lock:
	nix develop --command cargo generate-lockfile
