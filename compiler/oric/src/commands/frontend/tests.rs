use super::read_file_error_message;

#[test]
fn missing_source_message_names_cause_and_fix() {
    let error = std::io::Error::from(std::io::ErrorKind::NotFound);
    let message = read_file_error_message("missing.ori", &error);

    assert!(message.contains("cannot find source file 'missing.ori'"));
    assert!(message.contains("Check the path and try again"));
    assert!(message.contains("run 'ori help' for command usage"));
}
