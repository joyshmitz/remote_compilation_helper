//! Main binary that uses the build.rs generated code.
//!
//! This demonstrates the include! pattern for generated modules.

// Include the generated code from OUT_DIR
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

fn main() {
    println!("=== Build.rs Fixture Test ===");
    println!("Version: {}", version_info());
    println!("Build time: {}", build_timestamp());
    println!("Greeting: {}", generated_greeting());
    println!("build.rs ran: {}", BUILD_RS_RAN);
    println!("=== Test Complete ===");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info_not_empty() {
        assert!(!version_info().is_empty());
    }

    #[test]
    fn test_build_timestamp_not_empty() {
        assert!(!build_timestamp().is_empty());
    }

    #[test]
    fn test_generated_greeting_matches() {
        assert_eq!(generated_greeting(), "Hello from build.rs generated code!");
    }

    #[test]
    fn test_build_rs_ran_true() {
        assert!(BUILD_RS_RAN);
    }
}

