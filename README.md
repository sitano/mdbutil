MariaDB experimental utilities for testing and development purposes.
===

On Undo Log structure [link](https://sitano.github.io/mariadb/innodb/undolog/recovery/2025/08/08/notes-on-mariadb-undo-log/).

Read tablespace:

```
$ cargo run read-tablespace --file-path ./ibdata1 --undo-log-dir ./
Opened tablespace file: ./ibdata1 with size: 12582912 bytes, page size: 16384 bytes, num pages: 768, flags: FULL_CRC32|PAGE_SSIZE=5|POST_ANTELOPE|RAW=0x00000015
Tablespace(space_id=0, flags=0x15, page_size=16384, order=0)
PageBuf { space_id: 0, page_no: 0, prev_page: None, next_page: None, page_lsn: 44158, page_type: FspHdr, checksum: 1379061894 }
FSP header: fsp_header_t {
    space_id: 0,
    not_used: 0,
    space_pages: 768,
    free_limit: 320,
    flags: 21,
    free_frag_pages: 50,
    free_extens: flst_base_node_t { len: 2, first: fil_addr_t { page: 0, boffset: 278 }, last: fil_addr_t { page: 0, boffset: 318 } },
    free_frag: flst_base_node_t { len: 1, first: fil_addr_t { page: 0, boffset: 158 }, last: fil_addr_t { page: 0, boffset: 158 } },
    full_frag: flst_base_node_t { len: 0 },
    seg_id: 26,
    seg_inodes_full: flst_base_node_t { len: 0 },
    seg_inodes_free: flst_base_node_t { len: 1, first: fil_addr_t { page: 2, boffset: 38 }, last: fil_addr_t { page: 2, boffset: 38 } },
}
PageBuf { space_id: 0, page_no: 5, prev_page: None, next_page: None, page_lsn: 40054, page_type: TrxSys, checksum: 162806086 }
TRX_SYS header: trx_sys_t {
    id_store: 0,
    fseg_header: fseg_header_t {
        space: 0,
        page_no: 2,
        offset: 242,
    },
    rsegs: [
        (space_id: 0, page_no: 6),
        (space_id: 1, page_no: 3),
        (space_id: 2, page_no: 3),
        (space_id: 3, page_no: 3),
        ...
        (space_id: 1, page_no: 44),
        (space_id: 2, page_no: 44),
        (space_id: 3, page_no: 44),
    ],
    wsrep_xid: None,
    mysql_log: None,
    doublewrite: trx_sys_doublewrite_t {
        fseg: fseg_header_t {
            space: 0,
            page_no: 2,
            offset: 2738,
        },
        magic: 536853855,
        block1: 64,
        block2: 128,
        magic_repeat: 536853855,
        block1_repeat: 64,
        block2_repeat: 128,
    },
}
RSEG page: PageBuf { space_id: 0, page_no: 6, prev_page: None, next_page: None, page_lsn: 269027095, page_type: Sys, checksum: 3066088388 }
RSEG page: PageBuf { space_id: 1, page_no: 3, prev_page: None, next_page: None, page_lsn: 66000, page_type: Sys, checksum: 2212095732 }
trx_rseg_t { max_trx_id: 12 }
...
RSEG page: PageBuf { space_id: 1, page_no: 6, prev_page: None, next_page: None, page_lsn: 76648, page_type: Sys, checksum: 2692835315 }
trx_rseg_t {
    format: 0,
    history_size: 0,
    history: flst_base_node_t { len: 0 },
    fseg_header: fseg_header_t { space: 1, page_no: 2, offset: 626 },
    undo_slots: [],
    max_trx_id: 44,
    mysql_log: Some(
        mysql_log_t {
            log_offset: 7441,
            log_name: "/.../binlog.000001",
        },
    ),
    wsrep_xid: None,
}
...
```

On Redo Log structure [link](https://sitano.github.io/mariadb/innodb/redolog/recovery/2025/07/07/notes-on-mariadb-redo-log/).

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
$ cargo run -- read-redo --log-file-path data/ib_logfile0

Header block: 12288
Size: 100663296, Capacity: 100651008
RedoHeader {
    version: 1349024115,
    first_lsn: 12288,
    creator: "WorkatoDB Controller",
    crc: 509551572,
}
RedoCheckpointCoordinate {
    checkpoints: [
        RedoHeaderCheckpoint {
            checkpoint_lsn: 84875,
            end_lsn: 84875,
            checksum: 2243572435,
        },
        RedoHeaderCheckpoint {
            checkpoint_lsn: 84793,
            end_lsn: 84793,
            checksum: 3358478536,
        },
    ],
    checkpoint_lsn: Some(
        84875,
    ),
    checkpoint_no: Some(
        1,
    ),
    end_lsn: 84875,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}
1: MTR Chain count=1, len=16, lsn=84875
  1: [84875..84886) Mtr { space_id: 0, page_no: 0, op: FileCheckpoint } at (84875+11)
Checkpoint LSN/1: RedoHeaderCheckpoint { checkpoint_lsn: 84875, end_lsn: 84875, checksum: 2243572435 }
Checkpoint LSN/2: RedoHeaderCheckpoint { checkpoint_lsn: 84793, end_lsn: 84793, checksum: 3358478536 }
File checkpoint chain: Some(MtrChain { lsn: 84875, len: 16, marker: 1, checksum: 3078137627, mtr: [Mtr { lsn: 84875, len: 11, space_id: 0, page_no: 0, op: FileCheckpoint, file_checkpoint_lsn: Some(84875) }] })
File checkpoint LSN: 84875
```

or without the checkpoint:

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
$ pkill -9 mariadbd
$ cargo run -- read-redo --log-file-path data/ib_logfile0

Header block: 12288
Size: 100663296, Capacity: 100651008
RedoHeader {
    version: 1349024115,
    first_lsn: 12288,
    creator: "WorkatoDB Controller",
    crc: 509551572,
}
RedoCheckpointCoordinate {
    checkpoints: [
        RedoHeaderCheckpoint {
            checkpoint_lsn: 83365,
            end_lsn: 83365,
            checksum: 694933498,
        },
        RedoHeaderCheckpoint {
            checkpoint_lsn: 83267,
            end_lsn: 83267,
            checksum: 3695364396,
        },
    ],
    checkpoint_lsn: Some(
        83365,
    ),
    checkpoint_no: Some(
        1,
    ),
    end_lsn: 83365,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}
1: MTR Chain count=1, len=16, lsn=83365
  1: [83365..83376) Mtr { space_id: 0, page_no: 0, op: FileCheckpoint } at (83365+11)
2: MTR Chain count=37, len=289, lsn=83381
  1: [83381..83386) Mtr { space_id: 3, page_no: 0, op: Write } at (83381+5)
  2: [83386..83392) Mtr { space_id: 3, page_no: 2, op: Write } at (83386+6)
  3: [83392..83396) Mtr { space_id: 3, page_no: 2, op: Memset } at (83392+4)
  4: [83396..83400) Mtr { space_id: 3, page_no: 2, op: Memmove } at (83396+4)
...
21: MTR Chain count=13, len=89, lsn=84704
  1: [84704..84714) Mtr { space_id: 3, page_no: 4, op: Write } at (84704+10)
...
  13: [84780..84788) Mtr { space_id: 3, page_no: 0, op: Option } at (84780+8)
Checkpoint LSN/1: RedoHeaderCheckpoint { checkpoint_lsn: 83365, end_lsn: 83365, checksum: 694933498 }
Checkpoint LSN/2: RedoHeaderCheckpoint { checkpoint_lsn: 83267, end_lsn: 83267, checksum: 3695364396 }
File checkpoint chain: Some(MtrChain { lsn: 83365, len: 16, marker: 1, checksum: 3479569512, mtr: [Mtr { lsn: 83365, len: 11, space_id: 0, page_no: 0, op: FileCheckpoint, file_checkpoint_lsn: Some(83365) }] })
File checkpoint LSN: 83365

```

to craft fake redo log file checkpoint use `write`. MariaDB ensures that:

- file checkpoint is the latest entry in redo log by comparing its LSN to the
  end of the file LSN (redo log LSN).
- file checkpoint LSN is not less than the pages LSN in the tablespaces.

```
$ cargo run -- write-redo --log-file-path data/ib_logfile0 --size 100663296 --lsn 8336

Writing file checkpoint: [fa, 0, 0, 0, 0, 0, 0, 0, 1, 45, a6, 1, dc, 36, f7, 9c, 0] at pos: 83366 (0x145a6)
Target header block: 12288
Size: 100663296, Capacity: 0x5ffd000
RedoHeader {
    version: 1349024115,
    first_lsn: 12288,
    creator: "test_creator",
    crc: 2774233419,
}
RedoCheckpointCoordinate {
    checkpoints: [
        RedoHeaderCheckpoint {
            checkpoint_lsn: 83366,
            end_lsn: 83366,
            checksum: 3290552678,
        },
        RedoHeaderCheckpoint {
            checkpoint_lsn: 83366,
            end_lsn: 83366,
            checksum: 3290552678,
        },
    ],
    checkpoint_lsn: Some(
        83366,
    ),
    checkpoint_no: Some(
        0,
    ),
    end_lsn: 83366,
    encrypted: false,
    version: 1349024115,
    start_after_restore: false,
}
  [83366..83377) Mtr { space_id: 0, page_no: 0, op: FileCheckpoint } at (83366+11)
Target checkpoint LSN/1: RedoHeaderCheckpoint { checkpoint_lsn: 83366, end_lsn: 83366, checksum: 3290552678 }
Target checkpoint LSN/2: RedoHeaderCheckpoint { checkpoint_lsn: 83366, end_lsn: 83366, checksum: 3290552678 }
Target file checkpoint LSN: 83366

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
2025-07-22 17:51:49 0 [Note] InnoDB: End of log at LSN=83366
2025-07-22 17:51:49 0 [Note] InnoDB: Opened 3 undo tablespaces
2025-07-22 17:51:49 0 [Note] InnoDB: 128 rollback segments in 3 undo tablespaces are active.
2025-07-22 17:51:49 0 [Note] InnoDB: Setting file './ibtmp1' size to 12.000MiB. Physically writing the file full; Please wait ...
2025-07-22 17:51:49 0 [Note] InnoDB: File './ibtmp1' size is now 12.000MiB.
2025-07-22 17:51:49 0 [Note] InnoDB: log sequence number 83366+16; transaction id 26
2025-07-22 17:51:49 0 [Note] InnoDB: Loading buffer pool(s) from ./data/ib_buffer_pool
2025-07-22 17:51:49 0 [Note] Plugin 'FEEDBACK' is disabled.
2025-07-22 17:51:49 0 [Note] Plugin 'wsrep-provider' is disabled.
2025-07-22 17:51:49 0 [Note] InnoDB: Buffer pool(s) load completed at 250722 17:51:49
2025-07-22 17:51:50 0 [Note] Server socket created on IP: '::'.
2025-07-22 17:51:50 0 [Note] Server socket created on IP: '0.0.0.0'.
2025-07-22 17:51:50 0 [Note] mariadbd: Event Scheduler: Loaded 0 events
2025-07-22 17:51:50 0 [Note] mariadbd: ready for connections
```

