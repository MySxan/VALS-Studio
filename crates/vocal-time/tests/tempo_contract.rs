use vocal_domain::time::{Bpm, Seconds, Tick, TICKS_PER_QUARTER};
use vocal_time::{TempoCurve, TempoError, TempoEvent, TempoMap};

fn event(tick: i64, bpm: f64) -> TempoEvent {
    TempoEvent {
        tick: Tick::new(tick),
        bpm: Bpm::new(bpm).unwrap(),
        curve: TempoCurve::Step,
    }
}

fn seconds(value: f64) -> Seconds {
    Seconds::new(value).unwrap()
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn quarter_note_duration_uses_ppq() {
    let map = TempoMap::new(vec![event(0, 120.0)]).unwrap();
    assert_close(
        map.tick_to_seconds(Tick::new(TICKS_PER_QUARTER))
            .unwrap()
            .get(),
        0.5,
    );
    assert_eq!(
        map.seconds_to_tick(seconds(0.5)).unwrap().get(),
        TICKS_PER_QUARTER
    );
}

#[test]
fn tempo_changes_integrate_previous_segments_and_are_continuous() {
    let map = TempoMap::new(vec![event(0, 120.0), event(3840, 60.0), event(5760, 180.0)]).unwrap();
    for (tick, expected) in [(0, 0.0), (3840, 2.0), (4800, 3.0), (5760, 4.0), (8640, 5.0)] {
        assert_close(
            map.tick_to_seconds(Tick::new(tick)).unwrap().get(),
            expected,
        );
        assert_eq!(map.seconds_to_tick(seconds(expected)).unwrap().get(), tick);
    }
    assert_close(
        map.tick_to_seconds(Tick::new(3839)).unwrap().get(),
        2.0 - 0.5 / 960.0,
    );
    assert_close(
        map.tick_to_seconds(Tick::new(3841)).unwrap().get(),
        2.0 + 1.0 / 960.0,
    );
}

#[test]
fn preroll_uses_the_first_tempo() {
    let map = TempoMap::new(vec![event(0, 120.0), event(960, 60.0)]).unwrap();
    assert_close(map.tick_to_seconds(Tick::new(-1920)).unwrap().get(), -1.0);
    assert_eq!(map.seconds_to_tick(seconds(-1.0)).unwrap().get(), -1920);
}

#[test]
fn rejects_empty_unanchored_unordered_and_duplicate_maps() {
    assert_eq!(TempoMap::new(vec![]).unwrap_err(), TempoError::EmptyMap);
    for start in [-1, 1] {
        assert_eq!(
            TempoMap::new(vec![event(start, 120.0)]).unwrap_err(),
            TempoError::MissingOrigin
        );
    }
    for ticks in [[0, 0], [960, 480], [0, -1]] {
        assert_eq!(
            TempoMap::new(vec![
                event(0, 120.0),
                event(ticks[0], 100.0),
                event(ticks[1], 90.0)
            ])
            .unwrap_err(),
            TempoError::UnorderedEvents
        );
    }
}

#[test]
fn linear_is_not_silently_interpreted_as_step() {
    let linear = TempoEvent {
        curve: TempoCurve::Linear,
        ..event(0, 120.0)
    };
    assert_eq!(
        TempoMap::new(vec![linear]).unwrap_err(),
        TempoError::UnsupportedLinear
    );
    assert_eq!(
        TempoMap::new(vec![
            event(0, 120.0),
            TempoEvent {
                tick: Tick::new(960),
                ..linear
            }
        ])
        .unwrap_err(),
        TempoError::UnsupportedLinear
    );
}

#[test]
fn rounding_is_nearest_with_ties_away_from_zero() {
    // Exactly representable seconds per tick = 1/1024.
    let map = TempoMap::new(vec![event(0, 64.0)]).unwrap();
    for (tick, expected) in [
        (0.49, 0),
        (0.5, 1),
        (0.51, 1),
        (-0.49, 0),
        (-0.5, -1),
        (-0.51, -1),
    ] {
        assert_eq!(
            map.seconds_to_tick(seconds(tick / 1024.0)).unwrap().get(),
            expected
        );
    }
}

#[test]
fn round_trip_across_varied_tempo_maps() {
    for bpm in [30.0, 59.94, 120.0, 173.25, 300.0] {
        let map = TempoMap::new(vec![
            event(0, bpm),
            event(3840, bpm * 1.5),
            event(9723, bpm / 2.0),
        ])
        .unwrap();
        for tick in (-20000..100000).step_by(13) {
            let original = Tick::new(tick);
            assert_eq!(
                map.seconds_to_tick(map.tick_to_seconds(original).unwrap())
                    .unwrap(),
                original
            );
        }
        for tick in [3839, 3840, 3841, 9722, 9723, 9724] {
            let original = Tick::new(tick);
            assert_eq!(
                map.seconds_to_tick(map.tick_to_seconds(original).unwrap())
                    .unwrap(),
                original
            );
        }
    }
}

#[test]
fn seconds_round_trip_error_is_at_most_half_a_tick() {
    let map = TempoMap::new(vec![event(0, 120.0), event(3840, 60.0)]).unwrap();
    for value in [
        -3.123456, -0.0001, 0.72345, 1.999999, 2.0, 2.000001, 4.321098,
    ] {
        let actual = map
            .tick_to_seconds(map.seconds_to_tick(seconds(value)).unwrap())
            .unwrap()
            .get();
        assert!((actual - value).abs() <= 0.5 / 960.0 + 1e-12);
    }
}

#[test]
fn numeric_limits_return_errors_instead_of_saturating() {
    let map = TempoMap::new(vec![event(0, 120.0)]).unwrap();
    for tick in [i64::MIN, -(1_i64 << 53), 1_i64 << 53, i64::MAX] {
        assert_eq!(
            map.tick_to_seconds(Tick::new(tick)),
            Err(TempoError::NumericRange)
        );
    }
    for value in [f64::MAX, -f64::MAX] {
        assert_eq!(
            map.seconds_to_tick(seconds(value)),
            Err(TempoError::NumericRange)
        );
    }
    assert_eq!(
        TempoMap::new(vec![event(0, f64::from_bits(1))]).unwrap_err(),
        TempoError::NumericRange
    );
    assert_eq!(
        TempoMap::new(vec![event(0, f64::MAX)]).unwrap_err(),
        TempoError::NumericRange
    );
    assert_eq!(
        TempoMap::new(vec![event(0, 120.0), event(1_i64 << 53, 120.0)]).unwrap_err(),
        TempoError::NumericRange
    );
    let slow = TempoMap::new(vec![event(0, 1e-300)]).unwrap();
    assert_eq!(
        slow.tick_to_seconds(Tick::new(1_i64 << 52)),
        Err(TempoError::NumericRange)
    );
}

#[test]
fn rebuilding_the_map_does_not_mutate_original_inputs() {
    let original = TempoMap::new(vec![event(0, 120.0)]).unwrap();
    let mut edits = original.events().to_vec();
    edits.push(event(960, 60.0));
    let edited = TempoMap::new(edits).unwrap();
    assert_close(
        original.tick_to_seconds(Tick::new(1920)).unwrap().get(),
        1.0,
    );
    assert_close(edited.tick_to_seconds(Tick::new(1920)).unwrap().get(), 1.5);
}
