use super::EhModel;

#[test]
fn itanium_on_linux() {
    assert_eq!(
        EhModel::from_triple("x86_64-unknown-linux-gnu"),
        EhModel::Itanium
    );
}

#[test]
fn itanium_on_macos() {
    assert_eq!(
        EhModel::from_triple("aarch64-apple-darwin"),
        EhModel::Itanium
    );
}

#[test]
fn seh_on_windows_msvc() {
    assert_eq!(EhModel::from_triple("x86_64-pc-windows-msvc"), EhModel::Seh);
}

#[test]
fn itanium_on_windows_gnu() {
    assert_eq!(
        EhModel::from_triple("x86_64-pc-windows-gnu"),
        EhModel::Itanium
    );
}

#[test]
fn itanium_on_wasm() {
    assert_eq!(
        EhModel::from_triple("wasm32-unknown-wasi"),
        EhModel::Itanium
    );
}

#[test]
fn personality_name_itanium() {
    assert_eq!(EhModel::Itanium.personality_name(), "ori_eh_personality");
}

#[test]
fn personality_name_seh() {
    assert_eq!(EhModel::Seh.personality_name(), "__CxxFrameHandler3");
}
