MariaDB experimental utilities for testing and development purposes.
===

Redo log reader:

```
$ cargo run -- --log-group-path data
Redo Log Header: RedoHeader {
    version: 1349024115,
    first_lsn: 12288,
    creator: "MariaDB 11.8.2",
    crc: 103712569,
}
```
