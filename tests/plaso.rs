//! plaso's locate database (Apache-2.0, `tests/fixtures/plaso/`, see its
//! NOTICE): every directory plaso's `locate_database` parser reads, read
//! the same (`tests/oracle/plaso.tsv`, written by `tests/oracle/gen.py`).

use proptest::prelude::*;

fn database() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/plaso/mlocate.db"
    ))
    .unwrap()
}

#[test]
fn every_directory_as_plaso_reads_it() {
    let data = database();
    assert!(mlocate::is_mlocate(&data));
    let db = mlocate::read(&data);
    assert_eq!(db.problems, Vec::<String>::new());
    assert_eq!(db.root, "/home/user/temp");
    assert!(db
        .configuration
        .iter()
        .any(|(name, values)| name == "prunefs" && values.iter().any(|v| v == "NFS")));
    let mut got: Vec<String> = db
        .directories
        .iter()
        .map(|d| {
            let names: Vec<&str> = d.entries.iter().map(|e| e.name.as_str()).collect();
            // plaso rounds to the microsecond.
            let micros = (d.modified.unwrap().ticks().unwrap() + 5) / 10;
            format!("{}\t{micros}\t{}", d.path, names.join("|"))
        })
        .collect();
    got.sort();
    let expected: Vec<&str> = include_str!("oracle/plaso.tsv").lines().collect();
    assert_eq!(got, expected);
    let temp = db
        .directories
        .iter()
        .find(|d| d.path == "/home/user/temp")
        .unwrap();
    assert!(temp.entries.iter().all(|e| e.folder));
}

proptest! {
    #[test]
    fn damaged_databases_read_without_panic(
        flips in proptest::collection::vec((any::<usize>(), any::<u8>()), 0..32),
        cut in any::<usize>(),
    ) {
        let mut data = database();
        for (at, byte) in flips {
            let at = at % data.len();
            data[at] = byte;
        }
        data.truncate(cut % (data.len() + 1));
        let _ = mlocate::read(&data);
    }

    #[test]
    fn any_bytes_read_without_panic(data in proptest::collection::vec(any::<u8>(), 0..512)) {
        let mut bytes = b"\0mlocate".to_vec();
        bytes.extend(data);
        let _ = mlocate::read(&bytes);
    }
}
