# zkperf-workflow.mk — Process any Rust project into DA51 CBOR shards
#
# Usage:
#   make -f zkperf-workflow.mk PROJECT=~/git/kagenti-portal all
#   make -f zkperf-workflow.mk PROJECT=~/git/boringtun all
#   make -f zkperf-workflow.mk all-projects
#
# Requires: cargo-zkperf in PATH or ZKPERF_BIN set

ZKPERF_BIN ?= /mnt/data1/time-2026/03-march/19/zkperf/target/debug/cargo-zkperf
PASTEBIN_URL ?= http://127.0.0.1:8090
ZKPERF_SERVICE ?= http://127.0.0.1:9718

# All known projects
PROJECTS := \
	/mnt/data1/time-2026/03-march/19/zkperf \
	/home/mdupont/03-march/23/kagenti-portal \
	/home/mdupont/03-march/24/boringtun \
	/mnt/data1/git/meta-introspector/kagenti-native \
	/mnt/data1/nix/time/2024/12/10/swarms-terraform/services/submodules/zos-server

.PHONY: all audit annotate report shard post witness all-projects clean

# Single project workflow
all: audit report shard witness
	@echo "=== Done: $(PROJECT) ==="

audit:
	@echo "=== Audit: $(PROJECT) ==="
	$(ZKPERF_BIN) audit $(PROJECT)/src

report:
	@echo "=== Report: $(PROJECT) ==="
	$(ZKPERF_BIN) report $(PROJECT)/src > $(PROJECT)/zkperf-report.json
	@echo "Report: $(PROJECT)/zkperf-report.json"

annotate:
	@echo "=== Annotate: $(PROJECT) ==="
	$(ZKPERF_BIN) annotate $(PROJECT)/src

shard:
	@echo "=== Shard: $(PROJECT) ==="
	$(ZKPERF_BIN) shard $(PROJECT)/src

witness:
	@echo "=== Witness: $(PROJECT) ==="
	@curl -sf -X POST $(ZKPERF_SERVICE)/witness \
		-H 'content-type: application/json' \
		-d "{\"sig\":\"shard-pipeline\",\"event\":\"processed\",\"data_hash\":\"$(PROJECT)\",\"size\":0}" \
		>/dev/null 2>&1 && echo "  witnessed" || echo "  (zkperf-service not running)"

post:
	@echo "=== Post shards to pastebin ==="
	@for f in ~/.zkperf/shards/*/manifest.json; do \
		curl -sf -X POST $(PASTEBIN_URL)/paste \
			-H 'content-type: application/json' \
			-d "{\"content\":\"$$(cat $$f)\",\"title\":\"zkperf-shards\"}" \
			>/dev/null 2>&1 && echo "  posted $$f" || echo "  (pastebin not running)"; \
	done

# Process all projects
all-projects:
	@for proj in $(PROJECTS); do \
		echo ""; \
		echo "╔══════════════════════════════════════════╗"; \
		echo "║ Processing: $$proj"; \
		echo "╚══════════════════════════════════════════╝"; \
		$(MAKE) -f $(lastword $(MAKEFILE_LIST)) PROJECT=$$proj all 2>&1; \
	done
	@echo ""
	@echo "=== All projects processed ==="
	@echo "Shards: ~/.zkperf/shards/"
	@ls -d ~/.zkperf/shards/*/ 2>/dev/null

clean:
	rm -rf ~/.zkperf/shards/
