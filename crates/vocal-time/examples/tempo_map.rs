use vocal_domain::time::{Bpm, Seconds, Tick};
use vocal_time::{TempoCurve, TempoEvent, TempoMap};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let map = TempoMap::new(vec![
        TempoEvent {
            tick: Tick::new(0),
            bpm: Bpm::new(120.0)?,
            curve: TempoCurve::Step,
        },
        TempoEvent {
            tick: Tick::new(3840),
            bpm: Bpm::new(60.0)?,
            curve: TempoCurve::Step,
        },
    ])?;
    let position = map.tick_to_seconds(Tick::new(4800))?;
    let restored = map.seconds_to_tick(Seconds::new(3.0)?)?;
    println!("120 BPM -> 60 BPM at tick 3840 (2 seconds)");
    println!(
        "tick 4800 -> {} seconds -> tick {}",
        position.get(),
        restored.get()
    );
    assert_eq!(position.get(), 3.0);
    assert_eq!(restored.get(), 4800);
    Ok(())
}
