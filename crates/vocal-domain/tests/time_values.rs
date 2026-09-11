use vocal_domain::time::{Bpm, Samples, Seconds, Tick, TimeValueError};

#[test]
fn seconds_reject_nonfinite_values_and_allow_preroll() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(Seconds::new(value), Err(TimeValueError::NonFiniteSeconds));
    }
    assert_eq!(Seconds::new(-2.0).unwrap().get(), -2.0);
    assert!(!Seconds::new(-0.0).unwrap().get().is_sign_negative());
}

#[test]
fn bpm_is_positive_and_finite() {
    for value in [
        0.0,
        -0.0,
        -120.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert_eq!(Bpm::new(value), Err(TimeValueError::InvalidBpm));
    }
    assert_eq!(Bpm::new(120.0).unwrap().get(), 120.0);
}

#[test]
fn integral_positions_keep_the_full_signed_range() {
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        assert_eq!(Samples::new(value).get(), value);
        assert_eq!(Tick::new(value).get(), value);
    }
}
