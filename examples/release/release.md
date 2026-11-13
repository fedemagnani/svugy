---
styles:
  job: (color=#3478f6)
  store: (shape=cylinder, color=#d5dcea)
---

## Components
- maintainer: Maintainer (caption=merges · approves, color=#9945ff)
- github: GitHub (caption=fedemagnani/svugy)

  ```description
  [release-plz](#releaseplz) proposes and publishes versions; [dist](#dist) builds and hosts the binaries. Both run as GitHub Actions.
  ```

  - ci: CI (job, caption=fmt · clippy · tests)
  - releaseplz: release-plz (job, caption=versions · changelogs)

    ````description
    Every push to `main` opens or updates a release PR. Merging it asks for [approval](#approval), then publishes. From `release-plz.toml`:
    ```toml
    [workspace]
    release_always = false
    git_release_enable = false

    [[package]]
    name = "svugy-cli"
    version_group = "svugy"
    ```
    ````

  - approval: Approval (shape=slab, color=#f0a020, caption=release environment)

    ```description
    The `release` environment holds the publishing secrets and waits for the maintainer to approve each run.
    ```

  - dist: dist (job, caption=5 targets · attestations)
  - releases: Releases (store, caption=installers · archives)
- cratesio: crates.io (store, caption=5 crates)
- site: svugy.dev (shape=disc, caption=Cloudflare redirects)
- terminal: Your terminal (color=transparent, line=dashed, caption=curl … | sh)

## Workflows
### merge: A change lands on main (subtitle=and release-plz proposes a version)
Merging a change publishes nothing: it only updates the release PR.

#### check: CI checks the change
[CI](#ci) runs fmt, clippy and the tests on the PR and again on `main`.
- maintainer -> ci: merges a change (traveler=document)

#### propose: release-plz updates the release PR
The same push to `main` makes [release-plz](#releaseplz) bump the versions and changelogs in one PR.
- maintainer -> releaseplz: pushes to main
- releaseplz -> maintainer: opens the release PR (traveler=document)

### publish: A release is published (subtitle=from the merged release PR to crates.io and GitHub)
Nothing is published before [the maintainer approves](#publish/approve).

#### approve: The maintainer approves the run
Merging the release PR starts a run that waits on [the release environment](#approval), which holds the publishing secrets.
- maintainer -> approval: approves the release (traveler=key)
- approval -> releaseplz: unlocks publishing

#### crates: release-plz publishes the crates
[release-plz](#releaseplz) uploads every bumped crate to [crates.io](#cratesio).
- releaseplz -> cratesio: publishes the crates (traveler=parcel)

#### binaries: dist builds the installers
The `svugy-cli` tag starts [dist](#dist). It creates the GitHub release only once all five targets are built, so [the latest release](#releases) always has installers.
- releaseplz -> dist: pushes the svugy-cli tag
- dist -> releases: uploads the installers (traveler=parcel)

### install: A user installs svugy (subtitle=curl … | sh)
[svugy.dev](#site) holds no files: it redirects to [the latest GitHub release](#releases).

#### script: svugy.dev redirects to the installer
The whole install is one line:
```sh
curl -fsSL https://svugy.dev/install.sh | sh   # macOS, Linux
```
- terminal -> site: GET /install.sh
- site -> releases: 302 to the latest installer

  ```description
  `releases/latest/download/svugy-cli-installer.sh` always names the newest release's installer.
  ```

#### binary: The installer downloads the binary
The script picks the archive for your platform from [the same release](#releases) and unpacks `svugy` into `$CARGO_HOME/bin`.
- terminal -> releases: downloads the archive (traveler=parcel)
