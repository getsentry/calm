use std::io;
use std::io::Write;


pub fn clear_term() {
    write!(&mut io::stderr(), "\x1b[2J\x1b[H").ok();
}
