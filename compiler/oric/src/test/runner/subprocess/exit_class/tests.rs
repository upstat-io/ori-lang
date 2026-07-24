//! Pins for worker exit-status classification.

use super::{classify_exit, windows_abort_class};

#[cfg(unix)]
fn signal_status(signal: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(signal)
}

#[cfg(unix)]
fn exit_code_status(code: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code << 8)
}

#[cfg(unix)]
#[test]
fn test_classify_exit_names_shared_posix_signals() {
    assert_eq!(classify_exit(signal_status(1)), "signal 1 (SIGHUP)");
    assert_eq!(classify_exit(signal_status(2)), "signal 2 (SIGINT)");
    assert_eq!(classify_exit(signal_status(3)), "signal 3 (SIGQUIT)");
    assert_eq!(classify_exit(signal_status(6)), "signal 6 (SIGABRT)");
    assert_eq!(classify_exit(signal_status(11)), "signal 11 (SIGSEGV)");
    assert_eq!(classify_exit(signal_status(14)), "signal 14 (SIGALRM)");
    assert_eq!(classify_exit(signal_status(15)), "signal 15 (SIGTERM)");
    assert_eq!(classify_exit(signal_status(24)), "signal 24 (SIGXCPU)");
    assert_eq!(classify_exit(signal_status(25)), "signal 25 (SIGXFSZ)");
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn test_classify_exit_names_linux_divergent_signals() {
    assert_eq!(classify_exit(signal_status(7)), "signal 7 (SIGBUS)");
    assert_eq!(classify_exit(signal_status(10)), "signal 10 (SIGUSR1)");
    assert_eq!(classify_exit(signal_status(12)), "signal 12 (SIGUSR2)");
    assert_eq!(classify_exit(signal_status(17)), "signal 17 (SIGCHLD)");
    assert_eq!(classify_exit(signal_status(19)), "signal 19 (SIGSTOP)");
    assert_eq!(classify_exit(signal_status(20)), "signal 20 (SIGTSTP)");
    assert_eq!(classify_exit(signal_status(31)), "signal 31 (SIGSYS)");
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
#[test]
fn test_classify_exit_names_bsd_divergent_signals() {
    assert_eq!(classify_exit(signal_status(10)), "signal 10 (SIGBUS)");
    assert_eq!(classify_exit(signal_status(20)), "signal 20 (SIGCHLD)");
    assert_eq!(classify_exit(signal_status(30)), "signal 30 (SIGUSR1)");
    assert_eq!(classify_exit(signal_status(31)), "signal 31 (SIGUSR2)");
}

#[cfg(unix)]
#[test]
fn test_classify_exit_unmapped_signal_renders_numeric_form() {
    // 26 (SIGVTALRM) is outside the mapped set on every supported platform.
    let rendered = classify_exit(signal_status(26));
    assert_eq!(rendered, "signal 26");
    assert!(
        !rendered.contains("unknown signal"),
        "bare 'unknown signal' must never render: {rendered}"
    );
}

#[cfg(unix)]
#[test]
fn test_classify_exit_keeps_exit_code_classes() {
    assert_eq!(
        classify_exit(exit_code_status(134)),
        "exit code 134 (SIGABRT-class abort)"
    );
    assert_eq!(
        classify_exit(exit_code_status(101)),
        "exit code 101 (Rust panic)"
    );
    assert_eq!(classify_exit(exit_code_status(7)), "exit code 7");
}

#[cfg(unix)]
#[test]
fn test_classify_exit_code_3_stays_plain_exit_on_unix() {
    // Exit code 3 is abort-class on windows ONLY; the unix table must not
    // absorb it.
    assert_eq!(classify_exit(exit_code_status(3)), "exit code 3");
}

#[test]
fn test_windows_abort_class_maps_crt_abort_and_fastfail() {
    let crt_abort = windows_abort_class(3);
    assert!(
        crt_abort.is_some_and(|c| c.contains("SIGABRT-class abort")),
        "windows exit code 3 must classify as abort-class: {crt_abort:?}"
    );
    // 0xC0000409 (STATUS_STACK_BUFFER_OVERRUN) as i32.
    let fastfail = windows_abort_class(-1_073_740_791);
    assert!(
        fastfail.is_some_and(|c| c.contains("STATUS_STACK_BUFFER_OVERRUN")),
        "windows 0xC0000409 must classify as abort-class: {fastfail:?}"
    );
}

#[test]
fn test_windows_abort_class_rejects_non_abort_codes() {
    assert_eq!(windows_abort_class(0), None);
    assert_eq!(windows_abort_class(1), None);
    assert_eq!(
        windows_abort_class(101),
        None,
        "Rust panic stays in the shared table"
    );
    assert_eq!(
        windows_abort_class(134),
        None,
        "unix abort code stays in the shared table"
    );
}
