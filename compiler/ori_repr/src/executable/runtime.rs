//! Backend-neutral runtime-call identities.

/// Concrete source consumed by the iterator-construction runtime operation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IteratorSource {
    /// Integer range.
    Range,
    /// Persistent list.
    List,
    /// UTF-8 string.
    Str,
    /// Key-value map.
    Map,
    /// Unique-element set.
    Set,
    /// Optional value.
    Option,
}

/// Primitive formatting family selected by canonical lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormatRuntime {
    Int,
    Float,
    Str,
    Bool,
    Char,
}

/// Compiler-authored semantic services that survive as direct ARC calls.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompilerOperation {
    /// Recover the message captured by the innermost caught panic.
    CatchRecover,
    /// Format one primitive value with one source format specification.
    Format(FormatRuntime),
    /// Extend and return an owned delegated error trace.
    InjectTrace,
    /// Create the prefix half of a seamless list slice.
    ListSliceTake,
    /// Create the suffix half of a seamless list slice.
    ListSliceDrop,
}

impl CompilerOperation {
    const fn arity(self) -> usize {
        match self {
            Self::CatchRecover => 0,
            Self::InjectTrace => 1,
            Self::Format(_) | Self::ListSliceTake | Self::ListSliceDrop => 2,
        }
    }
}

impl IteratorSource {
    const fn from_receiver(receiver: ori_registry::TypeTag) -> Option<Self> {
        match receiver {
            ori_registry::TypeTag::Range => Some(Self::Range),
            ori_registry::TypeTag::List => Some(Self::List),
            ori_registry::TypeTag::Str => Some(Self::Str),
            ori_registry::TypeTag::Map => Some(Self::Map),
            ori_registry::TypeTag::Set => Some(Self::Set),
            ori_registry::TypeTag::Option => Some(Self::Option),
            ori_registry::TypeTag::Int
            | ori_registry::TypeTag::Float
            | ori_registry::TypeTag::Bool
            | ori_registry::TypeTag::Char
            | ori_registry::TypeTag::Byte
            | ori_registry::TypeTag::Unit
            | ori_registry::TypeTag::Never
            | ori_registry::TypeTag::Duration
            | ori_registry::TypeTag::Size
            | ori_registry::TypeTag::Ordering
            | ori_registry::TypeTag::Error
            | ori_registry::TypeTag::Tuple
            | ori_registry::TypeTag::Result
            | ori_registry::TypeTag::Channel
            | ori_registry::TypeTag::Function
            | ori_registry::TypeTag::Iterator
            | ori_registry::TypeTag::DoubleEndedIterator => None,
        }
    }
}

/// A runtime operation whose source-level meaning is fixed before backend selection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeCall {
    /// Create an iterator from an iterable value.
    Iter(IteratorSource),
    /// Allocate a list builder.
    ListNew,
    /// Release a list builder that did not reach normal finalization.
    ListFree,
    /// Advance an iterator.
    IterNext,
    /// Append a value to a list builder.
    ListBuilderPush,
    /// Append one value to a persistent list with copy-on-write semantics.
    ListPush,
    /// Insert one value into a persistent list with copy-on-write semantics.
    ListInsert,
    /// Remove one value from a persistent list with copy-on-write semantics.
    ListRemove,
    /// Insert one value at the front of a persistent list.
    ListPrepend,
    /// Release iterator state.
    IterDrop,
    /// Finish a list builder.
    ListTake,
    /// Index a collection.
    Index,
    /// Replace one list element through the concrete `List.set` method.
    ListSet,
    /// Read a collection or string length.
    Length,
    /// Convert a value to its string form.
    ToString,
    /// Concatenate strings.
    Concat,
    /// Test whether a string contains another string.
    StringContains,
    /// Test whether a string starts with a prefix.
    StringStartsWith,
    /// Test whether a string ends with a suffix.
    StringEndsWith,
    /// Test whether a string is empty.
    StringIsEmpty,
    /// Trim leading and trailing Unicode whitespace.
    StringTrim,
    /// Convert a string to Unicode uppercase.
    StringUppercase,
    /// Convert a string to Unicode lowercase.
    StringLowercase,
    /// Split a string by a separator.
    StringSplit,
    /// Execute a registry-owned builtin-method semantic operation.
    RegisteredMethod(ori_registry::MethodRuntime),
    /// Execute one exact method from the versioned builtin registry.
    RegistryMethod(ori_registry::RegisteredMethodId),
    /// Execute one exact free function from the versioned prelude registry.
    RegistryPrelude(ori_registry::RegisteredPreludeFunctionId),
    /// Execute one compiler-internal protocol operation.
    Protocol(ori_ir::builtin_constants::protocol::ProtocolBuiltin),
    /// Execute one typed compiler-authored service.
    Compiler(CompilerOperation),
    /// Print a string.
    Print,
    /// Raise an Ori panic.
    Panic,
}

impl RuntimeCall {
    /// Return the fixed number of operands accepted by this runtime operation.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Iter(_)
            | Self::IterDrop
            | Self::ListTake
            | Self::Length
            | Self::ToString
            | Self::StringIsEmpty
            | Self::StringTrim
            | Self::StringUppercase
            | Self::StringLowercase
            | Self::Print
            | Self::Panic => 1,
            Self::ListNew
            | Self::ListFree
            | Self::IterNext
            | Self::Index
            | Self::ListPush
            | Self::ListRemove
            | Self::ListPrepend
            | Self::Concat
            | Self::StringContains
            | Self::StringStartsWith
            | Self::StringEndsWith
            | Self::StringSplit => 2,
            Self::ListBuilderPush | Self::ListInsert | Self::ListSet => 3,
            Self::RegisteredMethod(operation) => operation.arity(),
            Self::RegistryMethod(method) => method.arity(),
            Self::RegistryPrelude(function) => function.arity(),
            Self::Protocol(protocol) => protocol.arg_count(),
            Self::Compiler(operation) => operation.arity(),
        }
    }

    pub(super) fn resolve(symbol: &str, receiver: Option<ori_registry::TypeTag>) -> Option<Self> {
        if let Some(call) = Self::resolve_explicit_registered_method(symbol, receiver) {
            return Some(call);
        }
        if symbol == "iter" {
            return receiver
                .and_then(IteratorSource::from_receiver)
                .map(Self::Iter);
        }
        Self::from_symbol(symbol)
            .or_else(|| Self::resolve_registry_method(symbol, receiver))
            .or_else(|| ori_registry::find_prelude_function_id(symbol).map(Self::RegistryPrelude))
    }

    fn resolve_explicit_registered_method(
        symbol: &str,
        receiver: Option<ori_registry::TypeTag>,
    ) -> Option<Self> {
        let receiver = receiver?;
        let method = ori_registry::find_method(receiver, symbol)?;
        method
            .runtime
            .and_then(|operation| Self::from_method_operation(operation, receiver))
    }

    fn resolve_registry_method(
        symbol: &str,
        receiver: Option<ori_registry::TypeTag>,
    ) -> Option<Self> {
        ori_registry::find_method_id(receiver?, symbol).map(Self::RegistryMethod)
    }

    fn from_method_operation(
        operation: ori_registry::MethodRuntime,
        receiver: ori_registry::TypeTag,
    ) -> Option<Self> {
        Some(match operation {
            ori_registry::MethodRuntime::Length => Self::Length,
            ori_registry::MethodRuntime::ListPush => Self::ListPush,
            ori_registry::MethodRuntime::ListSet => Self::ListSet,
            ori_registry::MethodRuntime::ListInsert => Self::ListInsert,
            ori_registry::MethodRuntime::ListRemove => Self::ListRemove,
            ori_registry::MethodRuntime::ListPrepend => Self::ListPrepend,
            ori_registry::MethodRuntime::Iter => {
                Self::Iter(IteratorSource::from_receiver(receiver)?)
            }
            ori_registry::MethodRuntime::ToString => Self::ToString,
            operation @ (ori_registry::MethodRuntime::Option(_)
            | ori_registry::MethodRuntime::Result(_)) => Self::RegisteredMethod(operation),
            ori_registry::MethodRuntime::Str(operation) => match operation {
                ori_registry::StrRuntime::Concat => Self::Concat,
                ori_registry::StrRuntime::Contains => Self::StringContains,
                ori_registry::StrRuntime::StartsWith => Self::StringStartsWith,
                ori_registry::StrRuntime::EndsWith => Self::StringEndsWith,
                ori_registry::StrRuntime::IsEmpty => Self::StringIsEmpty,
                ori_registry::StrRuntime::Trim => Self::StringTrim,
                ori_registry::StrRuntime::Uppercase => Self::StringUppercase,
                ori_registry::StrRuntime::Lowercase => Self::StringLowercase,
                ori_registry::StrRuntime::Split => Self::StringSplit,
            },
        })
    }

    fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "ori_list_new" => Some(Self::ListNew),
            "ori_list_free" => Some(Self::ListFree),
            "__iter_next" => Some(Self::IterNext),
            "ori_list_push" => Some(Self::ListBuilderPush),
            "ori_iter_drop" => Some(Self::IterDrop),
            "ori_list_take" => Some(Self::ListTake),
            "__index" => Some(Self::Index),
            "__collect_set" => Some(Self::Protocol(
                ori_ir::builtin_constants::protocol::ProtocolBuiltin::CollectSet,
            )),
            "__cast" => Some(Self::Protocol(
                ori_ir::builtin_constants::protocol::ProtocolBuiltin::Cast,
            )),
            "ori_catch_recover" => Some(Self::Compiler(CompilerOperation::CatchRecover)),
            "__ori_inject_trace" => Some(Self::Compiler(CompilerOperation::InjectTrace)),
            "ori_list_slice_take" => Some(Self::Compiler(CompilerOperation::ListSliceTake)),
            "ori_list_slice_drop" => Some(Self::Compiler(CompilerOperation::ListSliceDrop)),
            "ori_format_int" => Some(Self::Compiler(CompilerOperation::Format(
                FormatRuntime::Int,
            ))),
            "ori_format_float" => Some(Self::Compiler(CompilerOperation::Format(
                FormatRuntime::Float,
            ))),
            "ori_format_str" => Some(Self::Compiler(CompilerOperation::Format(
                FormatRuntime::Str,
            ))),
            "ori_format_bool" => Some(Self::Compiler(CompilerOperation::Format(
                FormatRuntime::Bool,
            ))),
            "ori_format_char" => Some(Self::Compiler(CompilerOperation::Format(
                FormatRuntime::Char,
            ))),
            "to_str" | "str" => Some(Self::ToString),
            "ori_print" => Some(Self::Print),
            "ori_panic" | "ori_panic_cstr" => Some(Self::Panic),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeCall;
    use ori_registry::{MethodKind, MethodRuntime, OptionRuntime, ResultRuntime, TypeTag};

    #[test]
    fn unwrap_or_resolution_preserves_receiver_semantics() {
        assert_eq!(
            RuntimeCall::resolve("unwrap_or", Some(TypeTag::Option)),
            Some(RuntimeCall::RegisteredMethod(MethodRuntime::Option(
                OptionRuntime::UnwrapOr
            )))
        );
        assert_eq!(
            RuntimeCall::resolve("unwrap_or", Some(TypeTag::Result)),
            Some(RuntimeCall::RegisteredMethod(MethodRuntime::Result(
                ResultRuntime::UnwrapOr
            )))
        );
        assert_eq!(
            RuntimeCall::RegisteredMethod(MethodRuntime::Option(OptionRuntime::UnwrapOr)).arity(),
            2
        );
        assert_eq!(
            RuntimeCall::RegisteredMethod(MethodRuntime::Result(ResultRuntime::UnwrapOr)).arity(),
            2
        );
    }

    #[test]
    fn registered_wrapper_resolution_is_receiver_exact() {
        let option_cases = [
            ("unwrap", OptionRuntime::Unwrap),
            ("map", OptionRuntime::Map),
            ("and_then", OptionRuntime::AndThen),
            ("clone", OptionRuntime::Clone),
            ("debug", OptionRuntime::Debug),
            ("equals", OptionRuntime::Equals),
            ("hash", OptionRuntime::Hash),
        ];
        for (name, operation) in option_cases {
            assert_eq!(
                RuntimeCall::resolve(name, Some(TypeTag::Option)),
                Some(RuntimeCall::RegisteredMethod(MethodRuntime::Option(
                    operation
                ))),
                "Option.{name}"
            );
        }

        let result_cases = [
            ("unwrap", ResultRuntime::Unwrap),
            ("map", ResultRuntime::Map),
            ("and_then", ResultRuntime::AndThen),
            ("clone", ResultRuntime::Clone),
            ("debug", ResultRuntime::Debug),
            ("equals", ResultRuntime::Equals),
            ("hash", ResultRuntime::Hash),
        ];
        for (name, operation) in result_cases {
            assert_eq!(
                RuntimeCall::resolve(name, Some(TypeTag::Result)),
                Some(RuntimeCall::RegisteredMethod(MethodRuntime::Result(
                    operation
                ))),
                "Result.{name}"
            );
        }

        assert_eq!(RuntimeCall::resolve("unwrap", None), None);
        assert_eq!(RuntimeCall::resolve("unwrap", Some(TypeTag::Int)), None);
    }

    #[test]
    fn backend_required_registry_method_resolves_without_textual_fallback() {
        let call = RuntimeCall::resolve("is_less", Some(TypeTag::Ordering))
            .unwrap_or_else(|| panic!("Ordering.is_less should resolve"));
        let RuntimeCall::RegistryMethod(method) = call else {
            panic!("Ordering.is_less should preserve its registry identity");
        };
        assert_eq!(method.receiver(), TypeTag::Ordering);
        assert_eq!(method.arity(), 1);
    }

    #[test]
    fn every_registered_method_has_an_exact_callable_identity() {
        for receiver in [
            TypeTag::Unit,
            TypeTag::List,
            TypeTag::Map,
            TypeTag::Set,
            TypeTag::Tuple,
            TypeTag::Ordering,
        ] {
            for method in ori_registry::methods_for(receiver) {
                let call = RuntimeCall::resolve(method.name, Some(receiver))
                    .unwrap_or_else(|| panic!("{receiver:?}.{} should resolve", method.name));
                let receiver_arity = usize::from(method.kind == MethodKind::Instance);
                assert_eq!(call.arity(), method.params.len() + receiver_arity);
            }
        }
    }

    #[test]
    fn registered_prelude_resolution_is_exact_and_arity_checked() {
        let call = RuntimeCall::resolve("hash_combine", None)
            .unwrap_or_else(|| panic!("hash_combine should resolve"));
        let RuntimeCall::RegistryPrelude(function) = call else {
            panic!("hash_combine should preserve its prelude registry identity");
        };
        assert_eq!(function.arity(), 2);

        assert_eq!(RuntimeCall::resolve("missing_prelude", None), None);
    }

    #[test]
    fn compiler_protocols_close_with_typed_identities() {
        let cases = [
            ("__cast", 1),
            ("__collect_set", 1),
            ("ori_catch_recover", 0),
            ("__ori_inject_trace", 1),
            ("ori_list_slice_drop", 2),
            ("ori_format_int", 2),
        ];
        for (symbol, arity) in cases {
            let call = RuntimeCall::resolve(symbol, None)
                .unwrap_or_else(|| panic!("{symbol} should have a closed identity"));
            assert_eq!(call.arity(), arity, "{symbol}");
        }
    }
}
