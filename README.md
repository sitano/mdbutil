MariaDB experimental utilities for testing and development purposes.
===

Redo log reader for 10.8.x:

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
$ pkill -9 mariadbd
$ cargo run -- --log-group-path data

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
        4096,
    ),
    end_lsn: 56893,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}

or with graceful shutdown:

$ cargo run -- --log-group-path data

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
    checksum: 530797207,
}
Err(
    Error {
        context: "Mtr::parse_next",
        source: Kind(
            NotFound,
        ),
    },
)```
