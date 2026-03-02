# CloudOps One Developer Quickstart

Version: v1.0  
Date: 2026-03-02

## 1. Repository Layout

- `services/api`: Rust Axum API service
- `libs/common`: shared Rust models/config
- `apps/web-console`: React + Vite frontend shell
- `deploy`: local dependency stack definitions
- `scripts`: install, upgrade, offline packaging, and dev helpers

## 2. Start Dependencies

If Docker is already installed:

```bash
bash scripts/install.sh --skip-docker-install --mirror cn
```

## 3. Run Backend API

```bash
cargo run -p api
```

Health check:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/api/v1/ping
```

## 4. Run Frontend

```bash
cd apps/web-console
npm install
npm run dev
```

## 5. One-Command Dev Entry

```bash
bash scripts/dev-up.sh
```

This command:

1. Ensures dependency stack is up.
2. Starts `services/api`.

## 6. Stop Dependencies

```bash
bash scripts/dev-down.sh
```
