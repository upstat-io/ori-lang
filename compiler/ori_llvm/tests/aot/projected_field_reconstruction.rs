//! End-to-end projected-field reconstruction ownership pins.

use crate::util::{compile_and_capture_ir, compile_and_run_with_env, extract_function_ir};

const FALLIBLE_IDENTITY_RELAY: &str =
    include_str!("fixtures/projected_field_reconstruction/fallible_identity_relay_unwind.ori");

#[test]
fn fallible_identity_relay_releases_residual_sibling_on_unwind() {
    let ir = compile_and_capture_ir(FALLIBLE_IDENTITY_RELAY);
    let update_ir = extract_function_ir(&ir, "_ori_update");

    assert!(
        !update_ir.contains("ori_list_rc_inc"),
        "the aggregate owner must fund the relayed field without a retain:\n{update_ir}"
    );
    let invoke = update_ir
        .find("invoke fastcc void @_ori_relay")
        .unwrap_or_else(|| {
            panic!("the admitted fallible relay must remain an invoke:\n{update_ir}")
        });
    let landingpad = update_ir[invoke..].find("landingpad").map_or_else(
        || panic!("the relay unwind edge must have a landing pad:\n{update_ir}"),
        |offset| invoke + offset,
    );
    let residual_dec = update_ir[landingpad..]
        .find("call void @ori_rc_dec")
        .map_or_else(
            || panic!("the unwind edge must release the still-owned sibling:\n{update_ir}"),
            |offset| landingpad + offset,
        );
    let resume = update_ir[residual_dec..].find("resume ").map_or_else(
        || panic!("the cleanup edge must resume unwinding:\n{update_ir}"),
        |offset| residual_dec + offset,
    );

    assert!(
        invoke < landingpad && landingpad < residual_dec && residual_dec < resume,
        "the residual sibling release must be ordered on the relay cleanup edge:\n{update_ir}"
    );
    assert_eq!(
        update_ir.matches("call void @ori_rc_dec").count(),
        1,
        "only the residual sibling is released by update's unwind cleanup:\n{update_ir}"
    );

    let (exit_code, stdout, stderr) =
        compile_and_run_with_env(FALLIBLE_IDENTITY_RELAY, &[("ORI_TRACE_RC", "1")]);
    assert_eq!(
        exit_code, 0,
        "the caught relay panic must remain leak- and double-free-clean:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "true");
    assert_eq!(
        stderr.matches("[RC] alloc").count(),
        2,
        "the fixture must allocate exactly the relayed and residual list buffers:\n{stderr}"
    );
    assert_eq!(
        stderr.matches("[RC] free").count(),
        2,
        "both list buffers must free exactly once across the unwind:\n{stderr}"
    );
    assert_eq!(
        stderr.matches("[RC] dec").count(),
        2,
        "the transferred field and residual sibling must each decrement exactly once:\n{stderr}"
    );
    assert!(
        stderr.contains("(live=0)"),
        "the RC trace must end with no live allocations:\n{stderr}"
    );
}
