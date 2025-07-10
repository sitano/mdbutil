MariaDB experimental utilities for testing and development purposes.
===

Redo log reader for 10.8.x:

```
$ cargo run -- --log-group-path data

RedoHeader {
    version: 1349024115,
    first_lsn: 12288,
    creator: "MariaDB 11.8.2",
    crc: 103712569,
}

RedoCheckpointCoordinate {
    checkpoint_lsn: Some(
        95487,
    ),
    checkpoint_no: Some(
        4096,
    ),
    end_lsn: 95487,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}
```
