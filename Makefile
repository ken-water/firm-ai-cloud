.PHONY: deps-up deps-down api-run api-check web-install web-dev web-build release-check release-publish release-publish-dry

VERSION ?=
REMOTE ?= origin
LATEST ?= 1

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

release-check:
ifeq ($(strip $(VERSION)),)
	bash scripts/release-sync-check.sh --remote "$(REMOTE)"
else
	bash scripts/release-sync-check.sh --version "$(VERSION)" --remote "$(REMOTE)"
endif

release-publish:
	@if [ -z "$(VERSION)" ]; then \
		echo "ERROR: VERSION is required. Example: make release-publish VERSION=0.1.4"; \
		exit 1; \
	fi
	@if [ "$(LATEST)" = "0" ]; then \
		bash scripts/release-publish.sh --version "$(VERSION)" --remote "$(REMOTE)" --no-latest; \
	else \
		bash scripts/release-publish.sh --version "$(VERSION)" --remote "$(REMOTE)"; \
	fi

release-publish-dry:
	@if [ -z "$(VERSION)" ]; then \
		echo "ERROR: VERSION is required. Example: make release-publish-dry VERSION=0.1.4"; \
		exit 1; \
	fi
	@if [ "$(LATEST)" = "0" ]; then \
		bash scripts/release-publish.sh --version "$(VERSION)" --remote "$(REMOTE)" --no-latest --dry-run; \
	else \
		bash scripts/release-publish.sh --version "$(VERSION)" --remote "$(REMOTE)" --dry-run; \
	fi
