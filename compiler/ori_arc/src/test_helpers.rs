//! Shared test utilities for ARC analysis passes.
//!
//! Consolidates factory functions used across `borrow`, `liveness`, `aims`,
//! and pipeline tests. Only compiled in test builds.

#![expect(
    clippy::disallowed_types,
    reason = "tracing event capture needs thread-safe shared test state"
)]

use ori_ir::{Name, Span};
use ori_types::{Idx, Pool, Tag};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Layer;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ArgOwnership,
};
use crate::ownership::Ownership;

#[derive(Clone, Default)]
struct EventCapture {
    events: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
}

impl<S> Layer<S> for EventCapture
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut fields = BTreeMap::new();
        event.record(&mut FieldCapture(&mut fields));
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(fields);
    }
}

struct FieldCapture<'a>(&'a mut BTreeMap<String, String>);

impl Visit for FieldCapture<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_owned(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_owned(), value.to_owned());
    }
}

/// Assert that an action emits the standard structured ablation event.
pub(crate) fn assert_ablation_event(
    action: impl FnOnce(),
    expected_toggle: &str,
    expected_effect: &str,
) {
    let capture = EventCapture::default();
    let events = Arc::clone(&capture.events);
    let subscriber = tracing_subscriber::registry().with(capture);
    tracing::subscriber::with_default(subscriber, action);

    let events = events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let event = events
        .iter()
        .find(|fields| {
            fields
                .get("toggle")
                .is_some_and(|value| value == expected_toggle)
        })
        .unwrap_or_else(|| panic!("missing ablation event for {expected_toggle}: {events:?}"));
    assert_eq!(
        event.get("effect").map(String::as_str),
        Some(expected_effect)
    );
    assert!(
        event
            .get("message")
            .is_some_and(|message| message.contains("ablation toggle fired")),
        "unexpected ablation event message: {event:?}"
    );
}

/// Assert that an ablation event is reached through its environment reader.
///
/// A subprocess isolates process-global environment and one-shot `LazyLock`
/// readers from the parallel unit-test process.
pub(crate) fn assert_ablation_env_event(
    test_name: &str,
    toggle: &str,
    expected_effect: &str,
    read_disabled: impl FnOnce() -> bool,
) {
    const CHILD_MARKER: &str = "ORI_ARC_ABLATION_EVENT_TEST_CHILD";

    if std::env::var_os(CHILD_MARKER).is_none() {
        let test_name = test_name.strip_prefix("ori_arc::").unwrap_or(test_name);
        let executable = std::env::current_exe()
            .unwrap_or_else(|error| panic!("unit-test executable should be available: {error}"));
        let output = std::process::Command::new(executable)
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env(CHILD_MARKER, "1")
            .env(toggle, "1")
            .output()
            .unwrap_or_else(|error| panic!("ablation-event child test should start: {error}"));
        assert!(
            output.status.success(),
            "ablation-event child failed for {toggle}:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    assert_eq!(std::env::var(toggle).as_deref(), Ok("1"));
    assert_ablation_event(
        || assert!(read_disabled(), "{toggle} reader ignored the set flag"),
        toggle,
        expected_effect,
    );
}

/// Shorthand for `ArcVarId::new(n)`.
pub(crate) fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Shorthand for `ArcBlockId::new(n)`.
pub(crate) fn b(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

/// Build a minimal `ArcFunction` with a default name (`Name::from_raw(1)`).
pub(crate) fn make_func(
    params: Vec<ArcParam>,
    return_type: Idx,
    blocks: Vec<ArcBlock>,
    var_types: Vec<Idx>,
) -> ArcFunction {
    make_func_named(Name::from_raw(1), params, return_type, blocks, var_types)
}

/// Build a minimal `ArcFunction` with an explicit name.
///
/// Used by borrow inference tests that need distinct names for
/// multi-function analysis.
pub(crate) fn make_func_named(
    name: Name,
    params: Vec<ArcParam>,
    return_type: Idx,
    blocks: Vec<ArcBlock>,
    var_types: Vec<Idx>,
) -> ArcFunction {
    let span_vecs: Vec<Vec<Option<Span>>> =
        blocks.iter().map(|bl| vec![None; bl.body.len()]).collect();
    ArcFunction {
        name,
        params,
        return_type,
        blocks,
        entry: ArcBlockId::new(0),
        var_types,
        var_reprs: Vec::new(),
        var_rc_strategies: Vec::new(),
        spans: span_vecs,
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        primitive_facts: crate::ir::PrimitiveFacts::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
        burden_emitted: Vec::new(),
        reassign_deaths: Vec::new(),
        ..Default::default()
    }
}

/// Build a block without phi-like parameters.
pub(crate) fn make_block(
    id: ArcBlockId,
    body: Vec<ArcInstr>,
    terminator: ArcTerminator,
) -> ArcBlock {
    ArcBlock {
        id,
        params: Vec::new(),
        body,
        terminator,
    }
}

/// Build an `Apply` fixture without a monomorphized instance identity.
pub(crate) fn make_apply(
    dst: ArcVarId,
    ty: Idx,
    func: Name,
    args: Vec<ArcVarId>,
    arg_ownership: Vec<ArgOwnership>,
) -> ArcInstr {
    ArcInstr::Apply {
        dst,
        ty,
        func,
        args,
        arg_ownership,
        mono_instance_id: None,
    }
}

/// Build an `Invoke` fixture without a monomorphized instance identity.
pub(crate) fn make_invoke(
    dst: ArcVarId,
    ty: Idx,
    func: Name,
    args: Vec<ArcVarId>,
    arg_ownership: Vec<ArgOwnership>,
    normal: ArcBlockId,
    unwind: ArcBlockId,
) -> ArcTerminator {
    ArcTerminator::Invoke {
        dst,
        ty,
        func,
        args,
        arg_ownership,
        mono_instance_id: None,
        normal,
        unwind,
    }
}

/// Build a zero-argument indirect invoke fixture.
pub(crate) fn invoke_indirect_no_args(
    dst: ArcVarId,
    ty: Idx,
    closure: ArcVarId,
    normal: ArcBlockId,
    unwind: ArcBlockId,
) -> ArcTerminator {
    ArcTerminator::InvokeIndirect {
        dst,
        ty,
        closure,
        args: Vec::new(),
        arg_ownership: Vec::new(),
        normal,
        unwind,
    }
}

/// Build a jump without block arguments.
pub(crate) fn jump_without_args(target: ArcBlockId) -> ArcTerminator {
    ArcTerminator::Jump {
        target,
        args: Vec::new(),
    }
}

/// Build a minimal empty `ArcBlock` with no params, empty body, and
/// `Unreachable` terminator. `id = ArcBlockId::new(n)`.
///
/// Used by validator tests that need a block skeleton but no actual
/// control flow (Test Fixture Strategy — the validator walks
/// `blocks[*].params[*].1` type positions, not the body or terminator).
pub(crate) fn minimal_block(n: u32) -> ArcBlock {
    make_block(ArcBlockId::new(n), Vec::new(), ArcTerminator::Unreachable)
}

/// Create an owned parameter.
pub(crate) fn owned_param(var: u32, ty: Idx) -> ArcParam {
    ArcParam {
        var: ArcVarId::new(var),
        ty,
        ownership: Ownership::Owned,
    }
}

/// Create a borrowed parameter.
pub(crate) fn borrowed_param(var: u32, ty: Idx) -> ArcParam {
    ArcParam {
        var: ArcVarId::new(var),
        ty,
        ownership: Ownership::Borrowed,
    }
}

/// Allocate a `Tag::Var` with the caller-specified pool var id.
///
/// Ensures pool var-state capacity covers `var_id`, then interns
/// `Tag::Var(var_id)`. Distinct from `Pool::fresh_var` which
/// auto-increments the next available id.
///
/// Used by validator tests that need deterministic pool var ids for
/// exemption-set contrast (e.g., Test Fixture Strategy — primary-seam
/// empty-exempt pin needs `Tag::Var(1)` to contrast against a synthetic
/// `scheme_var_ids = [1, 2, 3]` secondary-site exempt set).
pub(crate) fn allocate_pool_var_with_id(pool: &mut Pool, var_id: u32) -> Idx {
    pool.ensure_var_capacity(var_id + 1);
    pool.intern(Tag::Var, var_id)
}
