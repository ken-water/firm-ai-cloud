# Offline Docker Prerequisites (Optional)

This directory is optional and used only for fully air-gapped hosts that do not have Docker installed.

## How it works

- `scripts/install-offline.sh` will automatically execute `docker/install-docker-offline.sh` if:
  - Docker CLI is not found, and
  - the installer script exists and is executable.

## Package layout

Place your offline Docker packages under:

- `docker/packages/`

Supported by the sample installer:

- Debian/Ubuntu: `.deb` packages
- RHEL/CentOS/Fedora: `.rpm` packages

## Notes

- You must prepare complete dependency packages in advance.
- The sample script uses local package installation only and does not access external networks.
