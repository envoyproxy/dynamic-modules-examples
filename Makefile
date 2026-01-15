CYAN := \033[36m
GREEN := \033[32m
YELLOW := \033[33m
RESET := \033[0m
BOLD := \033[1m

define print_task
	printf "$(BOLD)$(CYAN)[TASK]$(RESET) $(BOLD)%s$(RESET)\n" "$(1)"
endef
define print_subtask
	printf "  $(YELLOW)→$(RESET) %s\n" "$(1)"
endef
define print_success
	printf "  $(GREEN)✓$(RESET) %s\n" "$(1)"
endef

## help: Show this help info.
.PHONY: help
help:
	@echo "Envoy Dynamic Modules.\n"
	@echo "Usage:\n  make \033[36m<Target>\033[0m \n\nTargets:"
	@awk 'BEGIN {FS = ":.*##"; printf ""} /^[a-zA-Z_0-9-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

# This runs all necessary steps to prepare for a commit.
.PHONY: precommit
precommit: ## Run all necessary steps to prepare for a commit.
precommit: precommit-go precommit-rust

.PHONY: precommit-go
precommit-go: ## This runs the linter, formatter, and tidy on the Go codebase.
	@$(call print_task,Tidying Go modules)
	@find . -name "go.mod" \
	| grep go.mod \
	| xargs -I {} bash -c 'dirname {}' \
	| xargs -I {} bash -c 'cd {} && $(call print_subtask,Tidying {}) && go mod tidy -v;'
	@$(call print_success,Tidying completed)
	@$(call print_task,Running linter)
	@$(call print_subtask,Checking ./...)
	@cd go && go tool golangci-lint run --build-tags==cgo ./...
	@$(call print_success,Linting completed)
	@$(call print_task,Formatting code)
	@$(call print_subtask,Running gofmt)
	@cd go && find . -type f -name '*.go' | xargs gofmt -s -w
	@$(call print_subtask,Running gofumpt)
	@cd go && find . -type f -name '*.go' | xargs go tool gofumpt -l -w
	@$(call print_subtask,Running gci)
	@cd go && go tool gci write -s standard -s default -s "prefix(github.com/envoyproxy/dynamic-modules-examples)" `find . -name '*.go'`
	@$(call print_success,Formatting completed)

.PHONY: precommt-rust
precommit-rust: ## This runs the linter, formatter, and tidy on the Rust codebase.
	@$(call print_task,Running Rust precommit steps)
	@$(call print_subtask,Running cargo fmt)
	@cd rust && cargo fmt --all -- --check
	@$(call print_subtask,Running cargo clippy)
	@cd rust && cargo clippy -- -D warnings
	@$(call print_success,Rust precommit steps completed)

# This runs precommit and checks for any differences in the codebase, failing if there are any.
.PHONY: check
check: precommit ## Run all necessary steps to prepare for a commit and check for any differences in the codebase.
	@$(call print_task,Checking for uncommitted changes)
	@if [ ! -z "`git status -s`" ]; then \
		echo "$(BOLD)$(YELLOW)The following differences will fail CI until committed:$(RESET)"; \
		git diff --exit-code; \
		echo "$(BOLD)$(YELLOW)Please ensure you have run 'make precommit' and committed the changes.$(RESET)"; \
		exit 1; \
	fi
	@$(call print_success,No uncommitted changes found)

.PHONY: test
test: test-rust test-rust ## Run all tests for the codebase.
.PHONY: test-go
test-go:## Run the unit tests for the Go codebase. This doesn't run the integration tests like test-* targets.
	@$(call print_task,Running Go tests)
	@cd go && go test -v ./...
	@$(call print_success,Go unit tests completed)
.PHONY: test-rust
test-rust: ## Run the unit tests for the Rust codebase.
	@$(call print_task,Running Rust tests)
	@cd rust && cargo test
	@$(call print_success,Rust unit tests completed)

.PHONY: build
build: build-go build-rust ## Build all dynamic modules.

.PHONY: build-go
build-go: ## Build the Go dynamic module.
	@$(call print_task,Building Go dynamic module)
	@cd go && go build -buildmode=c-shared -o libgo_module.so .
	@$(call print_success,Go dynamic module built at go/libgo_module.so)
	@$(call print_task,Copying Go dynamic module for easier use with Envoy)
	@cp go/libgo_module.so integration/libgo_module.so

.PHONY: build-rust
build-rust: ## Build the Rust dynamic module.
	@$(call print_task,Building Rust dynamic module)
	@cd rust && cargo build
	@$(call print_success,Rust dynamic module built at rust/target/debug/librust_module.so)
	@$(call print_task,Copying Rust dynamic module for easier use with Envoy)
	@cp rust/target/debug/librust_module.dylib integration/librust_module.so || true
	@cp rust/target/debug/librust_module.so integration/librust_module.so || true

.PHONY: integration-test
integration-test: build-go build-rust ## Run the integration tests.
	@$(call print_task,Running integration tests)
	@cd integration && go test -v ./...
	@$(call print_success,Integration tests completed)
