.PHONY: test test-unit test-integration test-all build clean dev

# Default target
all: test-all

# Run all tests in Docker
test-all:
	@echo "Running all tests in Docker..."
	docker-compose -f docker/docker-compose.test.yml up test --build --exit-code-from test

# Run unit tests only (no dependencies)
test-unit:
	@echo "Running unit tests in Docker..."
	docker-compose -f docker/docker-compose.test.yml run --rm test-unit

# Run integration tests (requires full app stack)
test-integration:
	@echo "Running integration tests in Docker..."
	docker-compose -f docker/docker-compose.test.yml run --rm test

# Run tests with verbose output
test-verbose:
	@echo "Running tests with verbose output..."
	docker-compose -f docker/docker-compose.test.yml run --rm test-debug

# Run tests for a specific crate
test-crate:
	@echo "Usage: make test-crate CRATE=core"
	@if [ -z "$(CRATE)" ]; then echo "Please specify CRATE=name"; exit 1; fi
	docker-compose -f docker/docker-compose.test.yml run --rm test cargo test -p faber-$(CRATE)

# Build the application
build:
	@echo "Building application..."
	docker-compose -f docker/docker-compose.dev.yml build

# Start development environment
dev:
	@echo "Starting development environment..."
	docker-compose -f docker/docker-compose.dev.yml up

# Clean up Docker resources
clean:
	@echo "Cleaning up Docker resources..."
	docker-compose -f docker/docker-compose.dev.yml down
	docker-compose -f docker/docker-compose.test.yml down
	docker system prune -f

# Run clippy for code quality
clippy:
	@echo "Running clippy..."
	docker-compose -f docker/docker-compose.test.yml run --rm test cargo clippy --workspace -- -D warnings

# Format code
fmt:
	@echo "Formatting code..."
	docker-compose -f docker/docker-compose.test.yml run --rm test cargo fmt --all 