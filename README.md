# RUN9

An independent classic Macintosh compatibility runtime, using Mac OS 9 as a
reference environment. Hosted first-level Marathon gameplay/audio, selected
TeachText/SimpleText workflows, and a freestanding ARM64 shared-core proof are
verified milestones. Full OS 9 equivalence and physical Raspberry Pi 500 drivers
remain future work.

## Layout

- `compat-core/`: shared services, renderer, and CPU integration.
- `runtime/`: hosted classic application loader, managers, window/input/audio.
- `native/`: freestanding ARM64 shared-execution experiment.
- `probes/`: project-authored native reference probes and owned fixtures.
- `tools/`, `script/`: builders, comparisons, launchers, and audits.
- `vendor/`: dependencies with their original licenses and provenance.
- `docs/fixture-review.json`: individual imported-fixture decisions.

`work/`, `outputs/`, and `.local/` contain local assets, reference environments,
archives, captures, and caches. They are gitignored. Original applications,
OS 9/ROM/media, captured game output, and the original experiment history are
not part of this repository's public Git history.

## Licensing

Project-authored Rust code is **MIT OR Apache-2.0**. Bundled fonts retain OFL1.1;
other dependencies retain their licenses. See `LICENSES.txt` and the package
THIRD_PARTY notices.

The hosted runtime temporarily depends on `vendor/run9-resource-codec`, which
is **GPL-3.0-or-later**. Distributing the combined runtime carries GPL obligations.
Moving that unchanged codec into its own crate does not remove those obligations.
Codec replacement or another GPL solution is explicitly deferred until after the
migration has been confirmed working. The complete runtime is not currently a
permissive-only distribution.

## Development

```
CARGO_INCREMENTAL=0 cargo test --manifest-path compat-core/Cargo.toml
CARGO_INCREMENTAL=0 cargo test --manifest-path vendor/run9-resource-codec/Cargo.toml
CARGO_INCREMENTAL=0 cargo test --manifest-path runtime/Cargo.toml
```

Original-application and OS 9 reference validation also require the local assets
and cross-toolchain. Migration/cutover status and the required verification gate
are documented in `docs/migration-plan.txt`. No original directory may be deleted
on the strength of unit tests alone.
