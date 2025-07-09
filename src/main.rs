use mdbutil::mach;

fn main() {
    let buf: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
    assert_eq!(mach::mach_read_from_4(&buf), 1);
}
