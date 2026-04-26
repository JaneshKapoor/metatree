//! Integration smoke tests: build the CLI binary as a library wouldn't expose
//! the modules, so we keep these focused on per-module unit tests located in
//! src/*. This file is a placeholder so `cargo test` enumerates a tests target.

#[test]
fn smoke() {
    assert_eq!(2 + 2, 4);
}
