MariaDB experimental utilities for testing and development purposes.
===

Redo log parser for 11.8.x:

```
$ scripts/mariadb-install-db --datadir ./data --innodb-log-file-size=10M
$ bin/mariadbd --datadir ./data --innodb_fast_shutdown=0 --innodb-log-file-size=10M

$ mycli -S /tmp/mysql.sock
> CREATE TABLE a (id int not null auto_increment primary key, t TEXT);
> SET max_recursive_iterations = 1000000;
> INSERT INTO a (t)
  WITH RECURSIVE fill(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM fill WHERE n < 60500
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
    checkpoints: [
        RedoHeaderCheckpoint {
            checkpoint_lsn: 2847229,
            end_lsn: 2847229,
            checksum: 3046192467,
        },
        RedoHeaderCheckpoint {
            checkpoint_lsn: 2847328,
            end_lsn: 2847328,
            checksum: 3415854794,
        },
    ],
    checkpoint_lsn: Some(
        2847328,
    ),
    checkpoint_no: Some(
        0,
    ),
    end_lsn: 2847328,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}
Mtr {
    len: 10,
    space_id: 0,
    page_no: 0,
    op: 240,
    checksum: 2504227498,
    file_checkpoint_lsn: Some(
        2847328,
    ),
}
Checkpoint LSN/1: RedoHeaderCheckpoint { checkpoint_lsn: 2847229, end_lsn: 2847229, checksum: 3046192467 }
Checkpoint LSN/2: RedoHeaderCheckpoint { checkpoint_lsn: 2847328, end_lsn: 2847328, checksum: 3415854794 }
File checkpoint LSN: 2847328
New MTR: Mtr {
    len: 10,
    space_id: 0,
    page_no: 0,
    op: 240,
    checksum: 2504227498,
    file_checkpoint_lsn: Some(
        2847328,
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
    0x2b,
    0x72,
    0x60,
    0x1,
    0x95,
    0x43,
    0x7a,
    0xaa,
    0x0,
] at pos: 2847328 (0x2b7260)
Target header block: 12288
Size: 100663296, Capacity: 0x5ffd000
RedoHeader {
    version: 1349024115,
    first_lsn: 12288,
    creator: "MariaDB 11.6.2",
    crc: 224651864,
}
RedoCheckpointCoordinate {
    checkpoints: [
        RedoHeaderCheckpoint {
            checkpoint_lsn: 2847328,
            end_lsn: 2847328,
            checksum: 3415854794,
        },
        RedoHeaderCheckpoint {
            checkpoint_lsn: 2847328,
            end_lsn: 2847328,
            checksum: 3415854794,
        },
    ],
    checkpoint_lsn: Some(
        2847328,
    ),
    checkpoint_no: Some(
        0,
    ),
    end_lsn: 2847328,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}
Mtr {
    len: 10,
    space_id: 0,
    page_no: 0,
    op: 240,
    checksum: 2504227498,
    file_checkpoint_lsn: Some(
        2847328,
    ),
}
Target checkpoint LSN/1: RedoHeaderCheckpoint { checkpoint_lsn: 2847328, end_lsn: 2847328, checksum: 3415854794 }
Target checkpoint LSN/2: RedoHeaderCheckpoint { checkpoint_lsn: 2847328, end_lsn: 2847328, checksum: 3415854794 }
Target file checkpoint LSN: 2847328

$ cp ./data/ib_logfile0.new ./data/ib_logfile0

# and now we can start mariadbd with the new redo log file

$ mariadbd --datadir ./data --innodb_fast_shutdown=0
2025-07-22 17:51:49 0 [Warning] Setting lower_case_table_names=2 because file system for ./data/ is case insensitive
2025-07-22 17:51:49 0 [Note] Starting MariaDB 11.6.2-MariaDB-debug source revision d8dad8c3b54cd09fefce7bc3b9749f427eed9709 server_uid jrmwW5r3Tn164Vhvku7bB+z6nV4= as process 13591
2025-07-22 17:51:49 0 [Note] InnoDB: !!!!!!!! UNIV_DEBUG switched on !!!!!!!!!
2025-07-22 17:51:49 0 [Note] InnoDB: Compressed tables use zlib 1.3.1
2025-07-22 17:51:49 0 [Note] InnoDB: Number of transaction pools: 1
2025-07-22 17:51:49 0 [Note] InnoDB: Using generic crc32 instructions
2025-07-22 17:51:49 0 [Note] InnoDB: Initializing buffer pool, total size = 128.000MiB, chunk size = 2.000MiB
2025-07-22 17:51:49 0 [Note] InnoDB: Completed initialization of buffer pool
2025-07-22 17:51:49 0 [Note] InnoDB: End of log at LSN=2847344
2025-07-22 17:51:49 0 [Note] InnoDB: Opened 3 undo tablespaces
2025-07-22 17:51:49 0 [Note] InnoDB: 128 rollback segments in 3 undo tablespaces are active.
2025-07-22 17:51:49 0 [Note] InnoDB: Setting file './ibtmp1' size to 12.000MiB. Physically writing the file full; Please wait ...
2025-07-22 17:51:49 0 [Note] InnoDB: File './ibtmp1' size is now 12.000MiB.
2025-07-22 17:51:49 0 [Note] InnoDB: log sequence number 2847344; transaction id 26
2025-07-22 17:51:49 0 [Note] InnoDB: Loading buffer pool(s) from ./data/ib_buffer_pool
2025-07-22 17:51:49 0 [Note] Plugin 'FEEDBACK' is disabled.
2025-07-22 17:51:49 0 [Note] Plugin 'wsrep-provider' is disabled.
2025-07-22 17:51:49 0 [Note] InnoDB: Buffer pool(s) load completed at 250722 17:51:49
2025-07-22 17:51:50 0 [Note] Server socket created on IP: '::'.
2025-07-22 17:51:50 0 [Note] Server socket created on IP: '0.0.0.0'.
2025-07-22 17:51:50 0 [Note] mariadbd: Event Scheduler: Loaded 0 events
2025-07-22 17:51:50 0 [Note] mariadbd: ready for connections
```

