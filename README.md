# `mimalloc-redirect`

Provides application-wide redirection to [MiMalloc](https://github.com/microsoft/mimalloc), and a dedicated
`#[global-allocator]` provider that acts as both force-linkage to the native libraries and as Rust global allocator.
