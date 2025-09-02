use std::fmt::Debug;

// Reference: sql/handler.h
pub const XIDDATASIZE: u32 = MYSQL_XIDDATASIZE;
//  struct st_mysql_xid is binary compatible with the XID structure as
//  in the X/Open CAE Specification, Distributed Transaction Processing:
//  The XA Specification, X/Open Company Ltd., 1991.
//  http://www.opengroup.org/bookstore/catalog/c193.htm
//
//  @see XID in sql/handler.h
//
//  Reference: include/mysql/plugin.h
pub const MYSQL_XIDDATASIZE: u32 = 128;

/// WSREP XID info structure. Present in the trx_sys_t or trx_rseg_t header.
#[allow(non_camel_case_types)]
#[derive(Clone)]
pub struct wsrep_xid_t {
    pub format: u32,
    pub gtrid_len: u32,
    pub bqual_len: u32,
    pub xid_data: [u8; XIDDATASIZE as usize],
}

impl Debug for wsrep_xid_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("trx_sys_wsrep_xid_t")
            .field("format", &self.format)
            .field("gtrid_len", &self.gtrid_len)
            .field("bqual_len", &self.bqual_len)
            .field(
                "xid_data",
                &self
                    .xid_data
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<String>>()
                    .join(""),
            )
            .finish()
    }
}
