use mimalloc_redirect::MiMalloc;

// This acts as both force-linkage to MiMalloc *and* override Rust's global allocator.
#[global_allocator]
pub static MI_MALLOC: MiMalloc = MiMalloc;

// Try setting `MIMALLOC_VERBOSE=1` to confirm MiMalloc is working.
// On Windows assuming no `+crt-static` (which you shouldn't do anyway), you should see this output:
//     mimalloc: malloc is redirected.
// This means MiMalloc has succeeded intercepting every single `malloc`/`free`/etc calls.
fn main() {
    let mut message = String::new();
    message.push_str("Hello");
    message.push_str(", ");
    message.push_str("world!");

    println!("{message}");
}
