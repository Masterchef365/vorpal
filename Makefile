.PHONY: all

all:
	@echo "Building vorpal-wasm-builtins..."
	@(cd vorpal-wasm-builtins/ && cargo b -r)

	@echo "Building vorpal-image..."
	@(cd vorpal-image/ && cargo b -r)

	@echo "Building fluidsim..."
	@(cd fluidsim/ && cargo b -r)

	@echo "Running the final command..."
	@cargo r -r

