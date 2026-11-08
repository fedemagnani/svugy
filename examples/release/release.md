---
title: How svugy ships
subtitle: from a merged PR to curl | sh
styles:
  job: (color=#3478f6)
  store: (shape=cylinder, color=#d5dcea)
---

```description
Nothing is published before [the maintainer approves](#approval). [dist](#dist) creates the GitHub release only once the installers are built, so [svugy.dev](#site) never points at a release without one.
```

## Components
- maintainer: Maintainer (caption=merges · approves, color=#9945ff)
- github: GitHub (caption=fedemagnani/svugy)

  ```description
  [release-plz](#releaseplz) proposes and publishes versions; [dist](#dist) builds and hosts the binaries. Both run as GitHub Actions.
  ```

  - ci: CI (job, caption=fmt · clippy · tests)
  - releaseplz: release-plz (job, caption=versions · changelogs)

    ```description
    Every push to `main` opens or updates a release PR. Merging it asks for [approval](#approval), then publishes.
    ```

  - approval: Approval (shape=slab, color=#f0a020, caption=release environment)

    ```description
    The `release` environment holds the publishing secrets and waits for the maintainer to approve each run.
    ```

  - dist: dist (job, caption=5 targets · attestations)
  - releases: Releases (store, caption=installers · archives)
- cratesio: crates.io (store, caption=5 crates)
- site: svugy.dev (shape=disc, caption=Cloudflare redirects)
- terminal: Your terminal (color=transparent, line=dashed, caption=curl … | sh)

## Edges
- maintainer -> ci: merges a change (traveler=document)
- maintainer -> approval: approves the release (traveler=key)
- approval -> releaseplz: unlocks publishing
- releaseplz -> cratesio: publishes the crates (traveler=parcel)
- releaseplz -> dist: pushes the svugy-cli tag
- dist -> releases: uploads the installers (traveler=parcel)
- terminal -> site: GET /install.sh
- site -> releases: 302 to the latest installer

  ```description
  `releases/latest/download/svugy-cli-installer.sh` always names the newest release's installer.
  ```
