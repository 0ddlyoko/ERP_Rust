//! Bounding how much runs at once.

use erp::concurrency::Gate;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Duration;

/// A gate lets through what it was given room for, and no more.
#[test]
fn test_a_gate_holds_back_what_does_not_fit() {
    let gate = Arc::new(Gate::new(3));
    let inside = Arc::new(AtomicUsize::new(0));
    let high_water = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Barrier::new(21));

    std::thread::scope(|scope| {
        for _ in 0..20 {
            let (gate, inside, high_water, started) = (
                Arc::clone(&gate),
                Arc::clone(&inside),
                Arc::clone(&high_water),
                Arc::clone(&started),
            );
            scope.spawn(move || {
                started.wait();
                let _pass = gate.enter();
                let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                high_water.fetch_max(now, Ordering::SeqCst);
                // Long enough that a gate letting everyone through would be caught.
                std::thread::sleep(Duration::from_millis(20));
                inside.fetch_sub(1, Ordering::SeqCst);
            });
        }
        started.wait();
    });

    assert_eq!(
        inside.load(Ordering::SeqCst),
        0,
        "every pass was given back"
    );
    assert!(
        high_water.load(Ordering::SeqCst) <= 3,
        "at most three at a time, saw {}",
        high_water.load(Ordering::SeqCst)
    );
    assert!(
        high_water.load(Ordering::SeqCst) > 1,
        "and more than one, or the test proves nothing"
    );
}

/// All twenty get through, however narrow the gate.
#[test]
fn test_everything_eventually_passes() {
    let gate = Arc::new(Gate::new(2));
    let done = Arc::new(AtomicUsize::new(0));

    std::thread::scope(|scope| {
        for _ in 0..20 {
            let (gate, done) = (Arc::clone(&gate), Arc::clone(&done));
            scope.spawn(move || {
                let _pass = gate.enter();
                done.fetch_add(1, Ordering::SeqCst);
            });
        }
    });

    assert_eq!(done.load(Ordering::SeqCst), 20);
}

/// Room is given back even when the work it guarded panicked.
#[test]
fn test_a_panic_gives_the_room_back() {
    let gate = Arc::new(Gate::new(1));

    let gate_for_panic = Arc::clone(&gate);
    let _ = std::thread::spawn(move || {
        let _pass = gate_for_panic.enter();
        panic!("on purpose");
    })
    .join();

    assert_eq!(gate.running(), 0, "the pass was dropped while unwinding");
    let _pass = gate.enter();
    assert_eq!(gate.running(), 1, "and the room is usable again");
}

/// Zero means no bound, which is what a configuration file leaving it out says.
#[test]
fn test_zero_means_unbounded() {
    assert_eq!(Gate::new(0).limit(), None);
    assert_eq!(Gate::unlimited().limit(), None);
    assert_eq!(Gate::new(5).limit(), Some(5));

    let gate = Gate::unlimited();
    let _one = gate.enter();
    let _two = gate.enter();
    assert_eq!(gate.running(), 0, "an unbounded gate counts nothing");
}
