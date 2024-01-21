#![feature(macro_metavar_expr)]

mod io {
    use std::fs::File;
    use std::io::{stdin, BufRead, BufReader};

    #[cfg(windows)]
    pub fn inner() -> impl BufRead {
        use std::os::windows::prelude::{AsRawHandle, FromRawHandle};
        unsafe {
            let stdin = File::from_raw_handle(stdin().as_raw_handle());
            BufReader::new(stdin)
        }
    }

    #[cfg(unix)]
    pub fn inner() -> impl BufRead {
        use std::os::unix::prelude::{AsRawFd, FromRawFd};
        unsafe {
            let stdin = File::from_raw_fd(stdin().as_raw_fd());
            BufReader::new(stdin)
        }
    }

    pub struct Scanner<R> {
        reader: R,
        buf_str: Vec<u8>,
        buf_iter: std::str::SplitAsciiWhitespace<'static>,
    }

    impl<R: BufRead> Scanner<R> {
        pub fn new(reader: R) -> Self {
            Self {
                reader,
                buf_str: Vec::new(),
                buf_iter: "".split_ascii_whitespace(),
            }
        }
        pub fn next<T: std::str::FromStr>(&mut self) -> T {
            loop {
                if let Some(token) = self.buf_iter.next() {
                    return token.parse().ok().expect("Failed parse");
                }
                unsafe {
                    self.buf_str.set_len(0);
                }
                self.reader
                    .read_until(b'\n', &mut self.buf_str)
                    .expect("Failed read");
                self.buf_iter = unsafe {
                    let slice = std::str::from_utf8_unchecked(&self.buf_str);
                    std::mem::transmute(slice.split_ascii_whitespace())
                }
            }
        }
    }
}
macro_rules! io_prelude {
    () => {
        let scanner = inner();
        let mut scanner = Scanner::new(scanner);
        macro_rules! input {
            ($$($ident:ident : $type:tt),+ ) => {
                $$(let $ident = scanner.next::<$type>();)+
            };
        }
    }
}

use crate::io::inner;
use crate::io::Scanner;
fn main() {
    io_prelude!();
    input! { a: usize, b: i32 }
    // let c = scanner.next::<i64>(); ERROR!
    println!("{a} {b}");
}
