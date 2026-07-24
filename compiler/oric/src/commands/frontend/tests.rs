use super::read_file_error_message;

#[test]
fn missing_source_message_names_cause_and_fix() {
    let error = std::io::Error::from(std::io::ErrorKind::NotFound);
    let message = read_file_error_message("missing.ori", &error);

    assert!(message.contains("cannot find source file 'missing.ori'"));
    assert!(message.contains("Check the path and try again"));
    assert!(message.contains("run 'ori help' for command usage"));
}

#[test]
fn missing_source_message_flags_misplaced_flag_argument() {
    let error = std::io::Error::from(std::io::ErrorKind::NotFound);
    let message = read_file_error_message("--release", &error);

    assert!(message.contains("'--release' looks like a flag"));
    assert!(message.contains("ori <command> <file> [options]"));
    assert!(!message.contains("cannot find source file"));
}
