MariaDB experimental utilities for testing and development purposes.
===

Redo log parser for 10.8.x:

```
$ scripts/mariadb-install-db --datadir ./data
$ bin/mariadbd --datadir ./data
$ mycli -S /tmp/mysql.sock
> CREATE TABLE a (id int not null auto_increment primary key, t TEXT);
> SET max_recursive_iterations = 20000;
> INSERT INTO a (t)
  WITH RECURSIVE fill(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM fill WHERE n < 16384
  )
  SELECT RPAD(CONCAT(FLOOR(RAND()*1000000)), 64, 'x') FROM fill;
$ pkill mariadbd
$ cargo run -- --log-group-path data

Header block: 12288
Size: 100663296, Capacity: 0x5ffd000
RedoHeader {
    version: 1349024115,
    first_lsn: 12288,
    creator: "MariaDB 11.6.2",
    crc: 224651864,
}
RedoCheckpointCoordinate {
    checkpoint_lsn: Some(
        56893,
    ),
    checkpoint_no: Some(
        8192,
    ),
    end_lsn: 56893,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}
Mtr {
    len: 10,
    space_id: 0,
    page_no: 0,
    op: 240,
    checksum: 530797207,
    file_checkpoint_lsn: Some(
        56893,
    ),
}
File checkpoint LSN: 56893
```

to craft fake redo log file checkpoint use `--write`. MariaDB ensures that:

- file checkpoint is the latest entry in redo log by comparing its LSN to the
  end of the file LSN (redo log LSN).
- file checkpoint LSN is not less than the pages LSN in the tablespaces.

> TODO: forge correct file checkpoint position.

```
$ cargo run -- --log-group-path data --write

Header block: 12288
Size: 100663296, Capacity: 0x5ffd000
RedoHeader {
    version: 1349024115,
    first_lsn: 12288,
    creator: "MariaDB 11.6.2",
    crc: 224651864,
}
RedoCheckpointCoordinate {
    checkpoint_lsn: Some(
        45048,
    ),
    checkpoint_no: Some(
        4096,
    ),
    end_lsn: 45048,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}
Mtr {
    len: 10,
    space_id: 0,
    page_no: 0,
    op: 240,
    checksum: 530797207,
    file_checkpoint_lsn: Some(
        56893,
    ),
}
File checkpoint LSN: 56893
File copied successfully from ./data4/ib_logfile0 to ./data4/ib_logfile0.copy
New MTR: Mtr {
    len: 10,
    space_id: 0,
    page_no: 0,
    op: 240,
    checksum: 530797207,
    file_checkpoint_lsn: Some(
        56893,
    ),
}
Writing file checkpoint: [
    0xfa,
    0x0,
    0x0,
    0x0,
    0x0,
    0x0,
    0x0,
    0x0,
    0x0,
    0xde,
    0x3d,
    0x1,
    0x1f,
    0xa3,
    0x52,
    0x97,
    0x0,
] at pos: 45048 (0xaff8)
```

