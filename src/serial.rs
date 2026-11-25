use core::arch::asm;

const COM1: u16 = 0x3f8;

fn out(port: u16, value: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)); }
}
fn input(port: u16) -> u8 {
    let value: u8;
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags)); }
    value
}
pub fn init() {
    out(COM1 + 1, 0);
    out(COM1 + 3, 0x80);
    out(COM1, 1); // 115200 baud
    out(COM1 + 1, 0);
    out(COM1 + 3, 3); // 8N1
    out(COM1 + 2, 0xc7);
    out(COM1 + 4, 0x0b);
}
fn byte(b: u8) {
    // Bounded wait: a missing UART must not block boot forever.
    for _ in 0..100_000 {
        if input(COM1 + 5) & 0x20 != 0 {
            out(COM1, b);
            return;
        }
        core::hint::spin_loop();
    }
}
pub fn write(s: &str) {
    for b in s.bytes() { byte(b); }
}
pub fn line(s: &str) { write(s); write("\r\n"); }
pub fn hex(n: u64) {
    write("0x");
    for shift in (0..16).rev() {
        let v = ((n >> (shift * 4)) & 15) as u8;
        byte(if v < 10 { b'0' + v } else { b'a' + v - 10 });
    }
}
pub fn metric(label: &str, value: u64) { write(label); hex(value); write("\r\n"); }
