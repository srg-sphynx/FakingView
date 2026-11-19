# Inactive platform workflow references

These workflow snapshots are engineering references, outside `.github/workflows/`, and GitHub
Actions does not execute them. They are not supported FakingView release targets.

- `flatpak.yml` and `flatpak-check.yml`: Linux Flatpak packaging experiments.
- `linux-glibc-check.yml`: validation of an older Linux binary deployment baseline.
- `windows-installer-check.yml`: Windows installer packaging checks.
- `macos-dmg-check.yml`: disk-image packaging checks with additional Python tools.

The active workflows are CI, supply-chain audits and the explicit FakingView macOS arm64 release.
See [publishing](../../github-publishing.md). Paths and assumptions inside these archived files
reflect their previous location and must be reviewed before reuse. Source support on a platform
does not establish that an application package has been built, signed or tested on that platform.
