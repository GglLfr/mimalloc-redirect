use mimalloc_redirect::MiMalloc;

// Try setting `MIMALLOC_VERBOSE=1` to confirm MiMalloc is working.
// We deliberately don't use `#[global_allocator]` here to test the application-wide redirection.
// Instead, we force-link MiMalloc with a call to `MiMalloc::get_version()`.
//
// On Windows assuming no `+crt-static` (which you shouldn't do anyway), you should see this output:
//     mimalloc: malloc is redirected.
// This means MiMalloc has succeeded intercepting every single `malloc`/`free`/etc calls.
fn main() {
    println!("Using MiMalloc {}.", MiMalloc::get_version());

    let mut message = String::new();
    message.push_str("Hello");
    message.push_str(", ");
    message.push_str("world!");

    println!("{message}");
}
