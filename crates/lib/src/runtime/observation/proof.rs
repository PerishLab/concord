use super::{Ambient, select, within};
use locus::{Config, Context, Engine, Key, Policy, Role};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Barrier;

fn frame(value: &str) -> Context {
    Context::load([(Role::span(), Key::new(value).expect("span"))])
}

#[test]
fn nested() {
    let root = frame("root");
    let outer = frame("outer");
    let inner = frame("inner");
    assert_eq!(select(&root), root);
    within(Some(outer.clone()), || {
        assert_eq!(select(&root), outer);
        let result: Result<(), &str> = within(Some(inner.clone()), || {
            assert_eq!(select(&root), inner);
            Err("business")
        });
        assert_eq!(result, Err("business"));
        assert_eq!(select(&root), outer);
        assert_eq!(within(None, || select(&root)), root);
        assert_eq!(select(&root), outer);
    });
    assert_eq!(select(&root), root);
}

#[test]
fn unwind() {
    let root = frame("root");
    let outer = frame("outer");
    within(Some(outer.clone()), || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            within(Some(frame("inner")), || panic!("inner failed"));
        }));
        assert!(result.is_err());
        assert_eq!(select(&root), outer);
        let result = catch_unwind(AssertUnwindSafe(|| {
            within(None, || {
                assert_eq!(select(&root), root);
                panic!("masked work failed");
            });
        }));
        assert!(result.is_err());
        assert_eq!(select(&root), outer);
    });
    assert_eq!(select(&root), root);
}

#[test]
fn threads() {
    let root = frame("root");
    let outer = frame("outer");
    within(Some(outer.clone()), || {
        parallel(&root, &outer);
        assert_eq!(select(&root), outer);
    });
    assert_eq!(select(&root), root);
}

fn parallel(root: &Context, outer: &Context) {
    let barrier = Barrier::new(2);
    let captured = Ambient::current();
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            let before = select(root);
            let during = captured.run(|| {
                barrier.wait();
                select(root)
            });
            (before, during, select(root))
        });
        let second = scope.spawn(|| {
            let during = within(Some(frame("sibling")), || {
                barrier.wait();
                select(root)
            });
            (during, select(root))
        });
        assert_eq!(
            first.join().expect("inherited thread"),
            (root.clone(), outer.clone(), root.clone())
        );
        assert_eq!(
            second.join().expect("sibling thread"),
            (frame("sibling"), root.clone())
        );
    });
}

#[test]
fn parent() {
    let engine = Engine::bootstrap(Config::new(Policy::default())).expect("disabled engine");
    let source = Context::load([
        (Role::trace(), Key::new("trace").expect("trace")),
        (Role::span(), Key::new("parent").expect("parent")),
    ]);
    let child = engine.fork(&source).expect("fork");
    assert_eq!(child.read(&Role::parent()), source.read(&Role::span()));
    assert_ne!(child.read(&Role::trace()), source.read(&Role::trace()));
    assert!(child.read(&Role::span()).is_none());
    std::thread::scope(|scope| {
        let result = scope.spawn(|| {
            within(Some(child.clone()), || {
                let observed = select(&source);
                assert_eq!(observed, child);
                assert_eq!(observed.read(&Role::parent()), source.read(&Role::span()));
                42
            })
        });
        assert_eq!(result.join().expect("fork thread"), 42);
    });
    assert_eq!(select(&source), source);
}
