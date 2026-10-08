# mlocate

The locate database (`/var/lib/mlocate/mlocate.db`, written by `updatedb` for `locate`), for forensics: every directory the last run indexed, with its modification time then and the names in it. A record of what was on a Linux system as of that run, deleted files included. One dependency, its sibling `sootmark-common` (times).

```toml
[dependencies]
sootmark-mlocate = "0.1"
```

```rust
let db = mlocate::read(&std::fs::read("/var/lib/mlocate/mlocate.db")?);
for dir in &db.directories {
    for entry in &dir.entries {
        println!("{:?} {}/{}", dir.modified, dir.path, entry.name);
    }
}
```

## What you get

- `read(bytes)`: the root `updatedb` started from, whether `locate` checks visibility, the configuration (`prunefs`, `prunenames`, `prunepaths`, `prune_bind_mounts` and their values), and each directory: its offset, path, modification time when indexed (UTC, to the nanosecond as written, kept to 100 ns) and entries (name, whether a folder).
- `is_mlocate(head)`: the `\0mlocate` signature.
- Damage goes to `problems`, never a panic: a directory cut short ends the reading with a problem, what came before is kept.

## How it's checked

- plaso's test database (Apache-2.0, `tests/fixtures/plaso/`, see its NOTICE): every directory plaso's `locate_database` parser reads, read the same (path, time to the microsecond as plaso rounds it, entries; `tests/oracle/`).
- Property tests: damaged and arbitrary databases give directories, problems or nothing, never a panic.

## Not yet

- plocate's database (`plocate.db`, zstd-compressed posting lists), which newer distributions use instead.

## Licence

MIT or Apache-2.0, at your option.
