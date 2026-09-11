use std::{error::Error, fmt};

/// Internal resolution, independent of meter denominator.
pub const TICKS_PER_QUARTER: i64 = 960;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeValueError {
    NonFiniteSeconds,
    InvalidBpm,
}

impl fmt::Display for TimeValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFiniteSeconds => "seconds must be finite",
            Self::InvalidBpm => "BPM must be finite and strictly positive",
        })
    }
}

impl Error for TimeValueError {}

/// Signed absolute time; negative values support pre-roll.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Seconds(f64);

impl Seconds {
    pub fn new(value: f64) -> Result<Self, TimeValueError> {
        if !value.is_finite() {
            return Err(TimeValueError::NonFiniteSeconds);
        }
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Bpm(f64);

impl Bpm {
    pub fn new(value: f64) -> Result<Self, TimeValueError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(TimeValueError::InvalidBpm);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Signed sample position. A sample rate is required to interpret it as time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Samples(i64);

impl Samples {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(i64);

impl Tick {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}
