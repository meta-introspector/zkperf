.PHONY: build test clean record record-all witness compare report install dev dev-shell fmt check nix-build nix-check

PERF := perf
STRACE := strace
OUT := target
PROOFS := proofs
RECORDINGS := recordings
LANGUAGES := coq haskell lua ocaml python ruby rust

# --- Nix-based builds (preferred) ---

nix-build:
	nix build

nix-check:
	nix flake check

build: nix-build

check:
	nix develop --command cargo check

fmt:
	nix develop --command cargo fmt

test:
	nix develop --command cargo test

clean:
	rm -rf result $(OUT)
	rm -rf $(RECORDINGS)/*.perf.data $(RECORDINGS)/*.strace.log

install: nix-build
	cp -L result/bin/zkperf-witness ~/.local/bin/ 2>/dev/null || cp result/bin/* ~/.local/bin/

# --- Perf Recording ---

$(RECORDINGS):
	mkdir -p $(RECORDINGS)

# Record perf for a single command: make record CMD="curl https://example.com"
record: $(RECORDINGS)
	$(PERF) record -g -o $(RECORDINGS)/session.perf.data -- $(CMD)
	$(PERF) report -i $(RECORDINGS)/session.perf.data --stdio > $(RECORDINGS)/session.perf.txt

# Record perf + strace together
record-full: $(RECORDINGS)
	$(STRACE) -T -tt -o $(RECORDINGS)/session.strace.log -- $(CMD) &
	$(PERF) record -g -o $(RECORDINGS)/session.perf.data -p $$! || true
	$(PERF) report -i $(RECORDINGS)/session.perf.data --stdio > $(RECORDINGS)/session.perf.txt

# Record perf stat counters
record-stat: $(RECORDINGS)
	$(PERF) stat -e cycles,instructions,cache-misses,branch-misses -o $(RECORDINGS)/session.stat.txt -- $(CMD)

# Record all language benchmarks
record-all: $(RECORDINGS)
	@for lang in $(LANGUAGES); do \
		echo "Recording $$lang..."; \
		./scripts/record-language.sh $$lang; \
	done

# --- Witness / Proof Generation ---

$(PROOFS):
	mkdir -p $(PROOFS)

witness: nix-build $(PROOFS)
	result/bin/zkperf-witness

# Compare two perf stages
compare: $(RECORDINGS)
	./scripts/compare-stages.sh $(STAGE0) $(STAGE1)

# Generate summary report
report:
	./scripts/generate-report.sh $(RECORDINGS) > $(PROOFS)/report.json

# --- Dev ---

dev:
	nix develop

dev-shell:
	nix develop --command bash
