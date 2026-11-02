# RUN9

An independent classic Macintosh compatibility runtime, using Mac OS 9 as a
reference environment. Hosted first-level Marathon gameplay/audio, selected
TeachText/SimpleText workflows, and a freestanding ARM64 shared-core proof are
verified milestones. Full OS 9 equivalence and physical Raspberry Pi 500 drivers
remain future work.

## Layout

- `compat-core/`: shared services, renderer, and CPU integration.
- `runtime/`: hosted classic application loader, managers, window/input/audio.
- `resource-codec/`: independently authored, bounded dcmp0/dcmp3 resource decoder.
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

The hosted runtime uses the **MIT OR Apache-2.0** codec in `resource-codec/`.
Its independent authoring inputs, retained MIT notices, and compatibility scope
are documented in `resource-codec/PROVENANCE.txt` and
`docs/resource-codec-format.md`. The old GPL codec remains in historical commits
and a private, separately executed comparison oracle; it is absent from the
current runtime dependency graph. Separate GPL tools such as QEMU and the offline
HFS utility retain their licenses. Bundled fonts remain OFL-1.1.
Verification counts, evidence pins, native adaptations, and limits are recorded
in `docs/resource-codec-validation.json`.

## Development

```
CARGO_INCREMENTAL=0 cargo test --manifest-path compat-core/Cargo.toml
CARGO_INCREMENTAL=0 cargo test --manifest-path resource-codec/Cargo.toml
CARGO_INCREMENTAL=0 cargo test --manifest-path runtime/Cargo.toml
```

Original-application and OS 9 reference validation also require the local assets
and cross-toolchain. The completed cutover checks are documented in
`docs/migration-status.txt`; the migration plan is in `docs/migration-plan.txt`.
Private migration reports are in `.local/migration/final-result.json`.
Historical completion audits retain the original revision's source pins and
evidence; they are not rewritten to manufacture a pass against the new layout.
