use mimalloc_redirect::MiMalloc;
use std::io::{Result as IoResult, Write};

// Try setting `MIMALLOC_VERBOSE=1` to confirm MiMalloc is working.
// We deliberately don't `#[global_allocator]` here to test the application-wide redirection.
// Instead, we force-link MiMalloc with a call to `MiMalloc::print_stats()`.
//
// On Windows assuming no `+crt-static` (which you shouldn't do anyway), you should see this output:
//     mimalloc: malloc is redirected.
// This means MiMalloc has succeeded intercepting every single `malloc`/`free`/etc calls.
fn main() {
    struct Mock;
    impl Write for Mock {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            Ok(buf.len())
        }

        #[inline]
        fn flush(&mut self) -> IoResult<()> {
            Ok(())
        }
    }

    // Don't actually print anything, we only care about the final output of MiMalloc.
    // If the values are not all 0s, that should indicate MiMalloc was actually used.
    MiMalloc::print_stats_to(&mut Mock);

    let mut message = String::new();
    message.push_str("Hello");
    message.push_str(", ");
    message.push_str("world!");

    println!("{message}");
}
