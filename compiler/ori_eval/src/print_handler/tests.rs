use super::*;

#[test]
fn buffer_handler_println_captures_with_newline() {
    let handler = BufferPrintHandler::new();
    handler.println("hello");
    assert_eq!(handler.get_output(), "hello\n");
}

#[test]
fn buffer_handler_print_captures_without_newline() {
    let handler = BufferPrintHandler::new();
    handler.print("hello");
    assert_eq!(handler.get_output(), "hello");
}

#[test]
fn buffer_handler_multiple_prints() {
    let handler = BufferPrintHandler::new();
    handler.print("hello");
    handler.print(" ");
    handler.println("world");
    assert_eq!(handler.get_output(), "hello world\n");
}

#[test]
fn buffer_handler_clear_empties_buffer() {
    let handler = BufferPrintHandler::new();
    handler.println("hello");
    assert!(!handler.get_output().is_empty());
    handler.clear();
    assert!(handler.get_output().is_empty());
}

#[test]
fn stdout_handler_get_output_returns_empty() {
    let handler = StdoutPrintHandler;
    assert_eq!(handler.get_output(), "");
}

#[test]
fn stdout_handler_clear_is_noop() {
    let handler = StdoutPrintHandler;
    // Should not panic
    handler.clear();
}

#[test]
fn buffer_handler_factory_creates_working_handler() {
    let handler = buffer_handler();
    handler.println("test");
    assert_eq!(handler.get_output(), "test\n");
}

#[test]
fn silent_handler_discards_output() {
    let handler = silent_handler();
    handler.println("hello");
    handler.print("world");
    assert_eq!(handler.get_output(), "");
}

#[test]
fn silent_handler_clear_is_noop() {
    let handler = silent_handler();
    handler.println("hello");
    handler.clear(); // Should not panic
    assert_eq!(handler.get_output(), "");
}

#[test]
fn buffer_handler_is_thread_safe() {
    use std::thread;

    let handler = buffer_handler();
    let handler2 = handler.clone();

    let t1 = thread::spawn(move || {
        for _ in 0..100 {
            handler2.println("a");
        }
    });

    for _ in 0..100 {
        handler.println("b");
    }

    t1.join().unwrap();

    let output = handler.get_output();
    let line_count = output.lines().count();
    assert_eq!(line_count, 200);
}
