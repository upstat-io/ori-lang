//! Tests for the Environment and `LocalScope` types.
//!
//! Tests variable scoping, binding, shadowing, mutability, and capture.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::Value;
use crate::ir::SharedInterner;
use ori_eval::{Environment, LocalScope, Scope};

// Scope Tests

mod scope {
    use super::*;

    #[test]
    fn define_and_lookup() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut scope = Scope::new();
        scope.define(x, Value::int(42), false);
        assert_eq!(scope.lookup(x), Some(Value::int(42)));
    }

    #[test]
    fn lookup_undefined_returns_none() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let scope = Scope::new();
        assert_eq!(scope.lookup(x), None);
    }

    #[test]
    fn shadowing_in_child_scope() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let parent = LocalScope::new(Scope::new());
        parent.borrow_mut().define(x, Value::int(1), false);

        let mut child = Scope::with_parent(parent);
        child.define(x, Value::int(2), false);

        // Child's binding shadows parent's
        assert_eq!(child.lookup(x), Some(Value::int(2)));
    }

    #[test]
    fn child_sees_parent_binding() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");

        let parent = LocalScope::new(Scope::new());
        parent.borrow_mut().define(x, Value::int(1), false);

        let mut child = Scope::with_parent(parent);
        child.define(y, Value::int(2), false);

        // Child can see parent's binding
        assert_eq!(child.lookup(x), Some(Value::int(1)));
        assert_eq!(child.lookup(y), Some(Value::int(2)));
    }

    #[test]
    fn assign_mutable_succeeds() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut scope = Scope::new();
        scope.define(x, Value::int(1), true);
        assert!(scope.assign(x, Value::int(2)).is_ok());
        assert_eq!(scope.lookup(x), Some(Value::int(2)));
    }

    #[test]
    fn assign_immutable_fails() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut scope = Scope::new();
        scope.define(x, Value::int(1), false);
        let result = scope.assign(x, Value::int(2));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("immutable"));
    }

    #[test]
    fn assign_undefined_fails() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut scope = Scope::new();
        let result = scope.assign(x, Value::int(1));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("undefined"));
    }

    #[test]
    fn assign_propagates_to_parent() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let parent = LocalScope::new(Scope::new());
        parent.borrow_mut().define(x, Value::int(1), true);

        let mut child = Scope::with_parent(parent.clone());
        assert!(child.assign(x, Value::int(2)).is_ok());

        // Check parent was updated
        assert_eq!(parent.borrow().lookup(x), Some(Value::int(2)));
    }
}

// Environment Tests

mod environment {
    use super::*;

    #[test]
    fn push_pop_scope() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(1), false);

        env.push_scope();
        env.define(x, Value::int(2), false);
        assert_eq!(env.lookup(x), Some(Value::int(2)));

        env.pop_scope();
        assert_eq!(env.lookup(x), Some(Value::int(1)));
    }

    #[test]
    fn mutable_binding() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(1), true);
        assert!(env.assign(x, Value::int(2)).is_ok());
        assert_eq!(env.lookup(x), Some(Value::int(2)));
    }

    #[test]
    fn immutable_binding_rejects_assign() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(1), false);
        assert!(env.assign(x, Value::int(2)).is_err());
    }

    #[test]
    fn capture_collects_all_visible_bindings() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");

        let mut env = Environment::new();
        env.define(x, Value::int(1), false);
        env.push_scope();
        env.define(y, Value::int(2), false);

        let captures = env.capture();
        assert_eq!(captures.get(&x), Some(&Value::int(1)));
        assert_eq!(captures.get(&y), Some(&Value::int(2)));
    }

    #[test]
    fn capture_uses_innermost_binding() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(1), false);
        env.push_scope();
        env.define(x, Value::int(2), false);

        let captures = env.capture();
        // Inner scope's binding wins
        assert_eq!(captures.get(&x), Some(&Value::int(2)));
    }

    #[test]
    fn global_binding() {
        let interner = SharedInterner::default();
        let g = interner.intern("global_var");

        let mut env = Environment::new();
        env.define_global(g, Value::int(100));

        env.push_scope();
        env.push_scope();
        // Global is visible from nested scope
        assert_eq!(env.lookup(g), Some(Value::int(100)));
    }

    #[test]
    fn depth_tracking() {
        let mut env = Environment::new();
        assert_eq!(env.depth(), 1); // Global scope

        env.push_scope();
        assert_eq!(env.depth(), 2);

        env.push_scope();
        assert_eq!(env.depth(), 3);

        env.pop_scope();
        assert_eq!(env.depth(), 2);
    }

    #[test]
    fn child_environment_shares_global() {
        let interner = SharedInterner::default();
        let g = interner.intern("global_var");

        let mut env = Environment::new();
        env.define_global(g, Value::int(100));

        let child = env.child();
        // Child sees global
        assert_eq!(child.lookup(g), Some(Value::int(100)));
    }

    #[test]
    fn pop_scope_preserves_global() {
        let mut env = Environment::new();
        // Can't pop below global
        env.pop_scope();
        assert_eq!(env.depth(), 1);
        env.pop_scope();
        assert_eq!(env.depth(), 1);
    }
}

// LocalScope Tests

mod local_scope {
    use super::*;
    use std::ops::Deref;

    #[test]
    fn new_and_borrow() {
        let scope = LocalScope::new(42);
        assert_eq!(*scope.borrow(), 42);
    }

    #[test]
    fn borrow_mut_modifies() {
        let scope = LocalScope::new(vec![1, 2, 3]);
        scope.borrow_mut().push(4);
        assert_eq!(*scope.borrow(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn clone_shares_allocation() {
        let scope1 = LocalScope::new(42);
        let scope2 = scope1.clone();

        // Both point to the same allocation
        scope1.borrow_mut().clone_from(&100);
        assert_eq!(*scope2.borrow(), 100);
    }

    #[test]
    fn default_uses_type_default() {
        let scope: LocalScope<i32> = LocalScope::default();
        assert_eq!(*scope.borrow(), 0);

        let scope: LocalScope<String> = LocalScope::default();
        assert_eq!(*scope.borrow(), "");
    }

    #[test]
    fn deref_returns_refcell() {
        let scope = LocalScope::new(42);
        // Deref returns &RefCell<T>
        let borrowed = scope.deref().borrow();
        assert_eq!(*borrowed, 42);
    }

    #[test]
    fn debug_format() {
        let scope = LocalScope::new(42);
        let debug_str = format!("{scope:?}");
        assert!(debug_str.contains("LocalScope"));
    }
}

// Edge Cases

mod edge_cases {
    use super::*;

    #[test]
    fn deeply_nested_scopes() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(0), false);

        // Create 100 nested scopes
        for i in 1..=100 {
            env.push_scope();
            env.define(x, Value::int(i), false);
        }

        assert_eq!(env.lookup(x), Some(Value::int(100)));
        assert_eq!(env.depth(), 101);

        // Pop all
        for i in (0..100).rev() {
            env.pop_scope();
            assert_eq!(env.lookup(x), Some(Value::int(i)));
        }
    }

    #[test]
    fn many_variables_in_one_scope() {
        let interner = SharedInterner::default();

        let mut env = Environment::new();
        for i in 0..1000 {
            let name = interner.intern(&format!("var_{i}"));
            env.define(name, Value::int(i), false);
        }

        // Verify all can be looked up
        for i in 0..1000 {
            let name = interner.intern(&format!("var_{i}"));
            assert_eq!(env.lookup(name), Some(Value::int(i)));
        }
    }

    #[test]
    fn redefine_in_same_scope_overwrites() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(1), false);
        env.define(x, Value::int(2), false);

        assert_eq!(env.lookup(x), Some(Value::int(2)));
    }

    #[test]
    fn capture_empty_environment() {
        let env = Environment::new();
        let captures = env.capture();
        assert!(captures.is_empty());
    }

    #[test]
    #[expect(
        clippy::many_single_char_names,
        reason = "single-char names mirror the variable names being interned"
    )]
    #[expect(
        clippy::approx_constant,
        reason = "testing float binding, not using pi"
    )]
    fn different_value_types() {
        let interner = SharedInterner::default();

        let mut env = Environment::new();

        let i = interner.intern("i");
        let f = interner.intern("f");
        let b = interner.intern("b");
        let s = interner.intern("s");
        let l = interner.intern("l");

        env.define(i, Value::int(42), false);
        env.define(f, Value::Float(3.14), false);
        env.define(b, Value::Bool(true), false);
        env.define(s, Value::string("hello"), false);
        env.define(l, Value::list(vec![Value::int(1)]), false);

        assert_eq!(env.lookup(i), Some(Value::int(42)));
        assert_eq!(env.lookup(f), Some(Value::Float(3.14)));
        assert_eq!(env.lookup(b), Some(Value::Bool(true)));
        assert_eq!(env.lookup(s), Some(Value::string("hello")));
    }
}
