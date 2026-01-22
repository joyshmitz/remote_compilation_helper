// RCH E2E Test Fixture - Broken Rust File
//
// This file contains intentional compilation errors for testing
// exit code 1 propagation from direct rustc invocation.

fn main() {
    // Intentional error: type mismatch
    let x: i32 = "not an integer";

    // Intentional error: undefined function
    let y = undefined_function();

    // Intentional error: missing semicolon
    println!("This will not compile")
    let z = 42;

    println!("{} {} {}", x, y, z);
}
