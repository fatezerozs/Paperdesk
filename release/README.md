# Release artifacts

This directory documents the portable release layout. Generated binaries are intentionally excluded from Git; Windows installers and Docker image archives are attached to the matching GitHub Release.

## Build locally

- Windows installer and portable folder: `scripts/package-windows.ps1`
- Docker image archive and deployment Compose file: `scripts/package-docker.ps1`
- Local installation and launcher behavior: [`docs/RELEASE-0.5.md`](../docs/RELEASE-0.5.md)
- Docker deployment guide: [`docs/DEPLOYMENT.md`](../docs/DEPLOYMENT.md)

The portable folder produced by the Windows packaging script contains `paperdesk.exe`, the launcher, runtime OCR assets, licenses, and the relevant documentation. The repository-local build folder is ignored by Git.
