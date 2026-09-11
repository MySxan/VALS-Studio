use std::{error::Error, fmt};
use vocal_domain::time::{Bpm, Seconds, Tick, TICKS_PER_QUARTER};

// Beyond this bound, f64 cannot represent every integral tick.
const EXACT_TICK_LIMIT: i64 = 1_i64 << 53;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempoCurve {
    Step,
    /// Reserved for a future implementation; never silently treated as Step.
    Linear,
}

/// The curve governs the interval starting at this event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoEvent {
    pub tick: Tick,
    pub bpm: Bpm,
    pub curve: TempoCurve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempoError {
    EmptyMap,
    MissingOrigin,
    UnorderedEvents,
    UnsupportedLinear,
    NumericRange,
}

impl fmt::Display for TempoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyMap => "tempo map must contain an event",
            Self::MissingOrigin => "first tempo event must be at tick zero",
            Self::UnorderedEvents => "tempo events must be strictly increasing",
            Self::UnsupportedLinear => "linear tempo curves are not implemented",
            Self::NumericRange => "time conversion exceeds floating-point range or precision",
        })
    }
}

impl Error for TempoError {}

/// Immutable map with a derived segment index. Tick zero equals zero seconds.
/// The first tempo extends into negative pre-roll; the last extends forward.
#[derive(Debug, Clone)]
pub struct TempoMap {
    events: Vec<TempoEvent>,
    starts_sec: Vec<f64>,
    seconds_per_tick: Vec<f64>,
}

impl TempoMap {
    pub fn new(events: Vec<TempoEvent>) -> Result<Self, TempoError> {
        let first = events.first().ok_or(TempoError::EmptyMap)?;
        if first.tick.get() != 0 {
            return Err(TempoError::MissingOrigin);
        }
        let mut starts_sec = vec![0.0_f64];
        let mut seconds_per_tick: Vec<f64> = Vec::with_capacity(events.len());
        for (i, event) in events.iter().enumerate() {
            if event.curve != TempoCurve::Step {
                return Err(TempoError::UnsupportedLinear);
            }
            check_tick(event.tick.get())?;
            let scale = (60.0 / TICKS_PER_QUARTER as f64) / event.bpm.get();
            if !scale.is_finite() || scale <= 0.0 || !scale.recip().is_finite() {
                return Err(TempoError::NumericRange);
            }
            if i > 0 {
                let previous = events[i - 1].tick.get();
                if event.tick.get() <= previous {
                    return Err(TempoError::UnorderedEvents);
                }
                let start = starts_sec[i - 1]
                    + (event.tick.get() - previous) as f64 * seconds_per_tick[i - 1];
                if !start.is_finite() || start <= starts_sec[i - 1] {
                    return Err(TempoError::NumericRange);
                }
                starts_sec.push(start);
            }
            seconds_per_tick.push(scale);
        }
        Ok(Self {
            events,
            starts_sec,
            seconds_per_tick,
        })
    }

    pub fn events(&self) -> &[TempoEvent] {
        &self.events
    }

    pub fn tick_to_seconds(&self, tick: Tick) -> Result<Seconds, TempoError> {
        check_tick(tick.get())?;
        let i = self
            .events
            .partition_point(|event| event.tick <= tick)
            .saturating_sub(1);
        let result = self.starts_sec[i]
            + (tick.get() - self.events[i].tick.get()) as f64 * self.seconds_per_tick[i];
        Seconds::new(result).map_err(|_| TempoError::NumericRange)
    }

    /// Nearest integral tick; exact half-tick ties round away from zero.
    /// This is time representation rounding, not musical grid quantization.
    pub fn seconds_to_tick(&self, seconds: Seconds) -> Result<Tick, TempoError> {
        let i = self
            .starts_sec
            .partition_point(|start| *start <= seconds.get())
            .saturating_sub(1);
        let tick = (self.events[i].tick.get() as f64
            + (seconds.get() - self.starts_sec[i]) / self.seconds_per_tick[i])
            .round();
        if !tick.is_finite() || tick.abs() >= EXACT_TICK_LIMIT as f64 {
            return Err(TempoError::NumericRange);
        }
        Ok(Tick::new(tick as i64))
    }
}

fn check_tick(tick: i64) -> Result<(), TempoError> {
    if tick <= -EXACT_TICK_LIMIT || tick >= EXACT_TICK_LIMIT {
        Err(TempoError::NumericRange)
    } else {
        Ok(())
    }
}
