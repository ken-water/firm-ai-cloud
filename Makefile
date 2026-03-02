.PHONY: deps-up deps-down api-run api-check web-install web-dev web-build

deps-up:
	bash scripts/install.sh --skip-docker-install --mirror cn

deps-down:
	bash scripts/uninstall.sh -y

api-run:
	cargo run -p api

api-check:
	cargo check --workspace

web-install:
	cd apps/web-console && npm install

web-dev:
	cd apps/web-console && npm run dev

web-build:
	cd apps/web-console && npm run build
