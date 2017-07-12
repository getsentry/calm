#![macro_use]

macro_rules! fail {
    ($expr:expr) => (
        return Err(::std::convert::From::from($expr));
    );
    ($expr:expr $(, $more:expr)+) => (
        fail!(format!($expr, $($more),*))
    )
}

macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        use std::io::Write;
        let r = writeln!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);
