#[cfg(test)]
mod tests {
    use rch_common::patterns::classify_command;

    #[test]
    fn test_ampersand_separator_should_be_rejected() {
        // "cargo build & echo done" implies backgrounding or chaining.
        // Current implementation might miss this if it only checks ends_with('&')
        let result = classify_command("cargo build & echo done");
        
        // We expect this to be NOT compilation (rejected due to structure)
        // If it returns true, it's a bug.
        assert!(!result.is_compilation, "Should reject 'cargo build & echo done'");
        assert!(result.reason.contains("background") || result.reason.contains("chained"), 
            "Reason should be background or chained, got: {}", result.reason);
    }

    #[test]
    fn test_ampersand_separator_with_spaces() {
        let result = classify_command("cargo build   &   echo done");
        assert!(!result.is_compilation, "Should reject 'cargo build   &   echo done'");
    }
}
