use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SampleFormat, SizedSample, Stream,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use vocal_app::{PlaybackEngine, PlaybackEngineStatus, PlaybackPhase};
use vocal_domain::signal::Pcm;

struct Shared {
    playing: AtomicBool,
    failed: AtomicBool,
    position: AtomicU64,
}

impl Shared {
    fn new() -> Self {
        Self {
            playing: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            position: AtomicU64::new(0),
        }
    }
}

pub struct CpalPlayback {
    stream: Option<Stream>,
    shared: Arc<Shared>,
    total_frames: u64,
}

impl Default for CpalPlayback {
    fn default() -> Self {
        Self {
            stream: None,
            shared: Arc::new(Shared::new()),
            total_frames: 0,
        }
    }
}

impl PlaybackEngine for CpalPlayback {
    fn load(&mut self, pcm: Pcm) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "未找到默认音频输出设备".to_string())?;
        let metadata = pcm.metadata();
        let rate = metadata.sample_rate();
        let supported = device
            .supported_output_configs()
            .map_err(|_| "无法读取音频输出设备配置".to_string())?
            .filter(|range| {
                range.channels() >= metadata.channels()
                    && range.min_sample_rate() <= rate
                    && range.max_sample_rate() >= rate
                    && matches!(
                        range.sample_format(),
                        SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
                    )
            })
            .min_by_key(|range| {
                (
                    range.channels() - metadata.channels(),
                    match range.sample_format() {
                        SampleFormat::F32 => 0,
                        SampleFormat::I16 => 1,
                        SampleFormat::U16 => 2,
                        _ => 3,
                    },
                )
            })
            .ok_or_else(|| "输出设备不支持音频的原采样率/声道配置".to_string())?
            .with_sample_rate(rate);
        let sample_format = supported.sample_format();
        let config = supported.into();
        let shared = Arc::new(Shared::new());
        let pcm = Arc::new(pcm);
        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, pcm, &shared),
            SampleFormat::I16 => build_stream::<i16>(&device, &config, pcm, &shared),
            SampleFormat::U16 => build_stream::<u16>(&device, &config, pcm, &shared),
            _ => Err("输出设备使用了不支持的采样格式".into()),
        }?;
        self.stream = Some(stream);
        self.shared = shared;
        self.total_frames = metadata.frames().get() as u64;
        Ok(())
    }

    fn play(&mut self) -> Result<(), String> {
        let stream = self
            .stream
            .as_ref()
            .ok_or_else(|| "播放尚未载入".to_string())?;
        if self.shared.failed.load(Ordering::Acquire) {
            return Err("音频输出设备已中断".into());
        }
        if self.shared.position.load(Ordering::Acquire) >= self.total_frames {
            self.shared.position.store(0, Ordering::Release);
        }
        stream.play().map_err(|_| "无法启动音频输出".to_string())?;
        self.shared.playing.store(true, Ordering::Release);
        Ok(())
    }

    fn pause(&mut self) -> Result<(), String> {
        self.shared.playing.store(false, Ordering::Release);
        if let Some(stream) = &self.stream {
            stream.pause().map_err(|_| "无法暂停音频输出".to_string())?;
        }
        Ok(())
    }

    fn seek(&mut self, frame: u64) -> Result<(), String> {
        if self.stream.is_none() {
            return Err("播放尚未载入".into());
        }
        self.shared
            .position
            .store(frame.min(self.total_frames), Ordering::Release);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.pause()?;
        self.shared.position.store(0, Ordering::Release);
        Ok(())
    }

    fn status(&self) -> Result<PlaybackEngineStatus, String> {
        if self.stream.is_none() {
            return Err("播放尚未载入".into());
        }
        if self.shared.failed.load(Ordering::Acquire) {
            return Err("音频输出设备已中断".into());
        }
        let position = self.shared.position.load(Ordering::Acquire);
        let playing = self.shared.playing.load(Ordering::Acquire);
        Ok(PlaybackEngineStatus {
            phase: if playing {
                PlaybackPhase::Playing
            } else if position >= self.total_frames {
                PlaybackPhase::Ended
            } else if position == 0 {
                PlaybackPhase::Stopped
            } else {
                PlaybackPhase::Paused
            },
            position_frames: position,
            total_frames: self.total_frames,
        })
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    pcm: Arc<Pcm>,
    shared: &Arc<Shared>,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let callback_state = Arc::clone(shared);
    let error_state = Arc::clone(shared);
    let output_channels = config.channels as usize;
    device
        .build_output_stream(
            *config,
            move |output: &mut [T], _| render(output, output_channels, &pcm, &callback_state),
            move |_| {
                error_state.playing.store(false, Ordering::Release);
                error_state.failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|_| "无法创建音频输出流".to_string())
}

fn render<T>(output: &mut [T], output_channels: usize, pcm: &Pcm, shared: &Shared)
where
    T: Sample + FromSample<f32>,
{
    let silence = T::from_sample(0.0);
    if !shared.playing.load(Ordering::Acquire) {
        output.fill(silence);
        return;
    }
    let source_channels = pcm.metadata().channels() as usize;
    let total_frames = pcm.metadata().frames().get() as u64;
    let mut position = shared.position.load(Ordering::Acquire).min(total_frames);
    for frame in output.chunks_mut(output_channels) {
        if position >= total_frames {
            frame.fill(silence);
            continue;
        }
        let source = &pcm.samples()
            [position as usize * source_channels..(position as usize + 1) * source_channels];
        for (channel, target) in frame.iter_mut().enumerate() {
            *target = T::from_sample(if source_channels == 1 {
                source[0]
            } else {
                source.get(channel).copied().unwrap_or(0.0)
            });
        }
        position += 1;
    }
    shared.position.store(position, Ordering::Release);
    if position >= total_frames {
        shared.playing.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vocal_domain::{audio::AudioMetadata, time::Samples};

    #[test]
    fn callback_duplicates_mono_advances_cursor_and_ends_without_allocation_contract_work() {
        let metadata = AudioMetadata::new(48_000, 1, Samples::new(2)).unwrap();
        let pcm = Pcm::new(metadata, vec![0.25, -0.5]).unwrap();
        let shared = Shared::new();
        shared.playing.store(true, Ordering::Release);
        let mut output = [9.0_f32; 6];
        render(&mut output, 2, &pcm, &shared);
        assert_eq!(output, [0.25, 0.25, -0.5, -0.5, 0.0, 0.0]);
        assert_eq!(shared.position.load(Ordering::Acquire), 2);
        assert!(!shared.playing.load(Ordering::Acquire));
    }
}
