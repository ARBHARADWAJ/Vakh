use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::Sender;
use webrtc_vad::{Vad, VadMode, SampleRate};
use crate::state::AudioStatus;

const RMS_SCALE: f32 = 10.0;

pub struct AudioProcessor {
    vad: Vad,
    sample_rate: u32,
    resample_buffer: Vec<f32>,
    frame_buffer: Vec<i16>,
    is_speech_active: bool,
    silence_counter: usize,
    level_counter: usize,
    hangover_counter: usize,
    voice_activation_counter: usize,
    warmup_counter: usize,
    status_tx: Option<Sender<AudioStatus>>,
}

impl AudioProcessor {
    pub fn new(input_sample_rate: u32, status_tx: Option<Sender<AudioStatus>>) -> Self {
        let vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Quality);

        Self {
            vad,
            sample_rate: input_sample_rate,
            resample_buffer: Vec::new(),
            frame_buffer: Vec::new(),
            is_speech_active: false,
            silence_counter: 0,
            level_counter: 0,
            hangover_counter: 0,
            voice_activation_counter: 0,
            warmup_counter: 0,
            status_tx,
        }
    }

    pub fn start_listening(
        tx1: Sender<Vec<f32>>,
        tx2: Sender<Vec<f32>>,
        status_tx: Sender<AudioStatus>
    ) -> Result<cpal::Stream, String> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No input device found")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let format = config.sample_format();

        println!("[Audio] Device: {}, Rate: {}, Channels: {}, Format: {:?}",
            device.name().unwrap_or("Unknown".to_string()),
            sample_rate, channels, format);

        let (raw_tx, raw_rx) = std::sync::mpsc::channel::<Vec<f32>>();

        std::thread::spawn(move || {
            let mut processor = Self::new(sample_rate, Some(status_tx));
            while let Ok(data) = raw_rx.recv() {
                processor.process_audio(&data, channels, &tx1, &tx2);
            }
        });

        let stream = match format {
            cpal::SampleFormat::F32 => {
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _| {
                        let _ = raw_tx.send(data.to_vec());
                    },
                    |err| eprintln!("Audio stream error: {}", err),
                    None
                )
            },
            cpal::SampleFormat::I16 => {
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _| {
                        let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        let _ = raw_tx.send(f32_data);
                    },
                    |err| eprintln!("Audio stream error: {}", err),
                    None
                )
            },
            _ => return Err(format!("Unsupported sample format: {:?}", format)),
        }.map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;
        println!("[Audio] Stream started successfully");
        Ok(stream)
    }

    fn process_audio(&mut self, data: &[f32], channels: u16, tx1: &Sender<Vec<f32>>, tx2: &Sender<Vec<f32>>) {
        if data.is_empty() { return; }

        let mono_data: Vec<f32> = if channels > 1 {
            data.chunks(channels as usize)
                .map(|chunk| {
                    let sum: f32 = chunk.iter().sum();
                    sum.clamp(-1.0, 1.0)
                })
                .collect()
        } else {
            data.to_vec()
        };

        let target_rate = 16000.0;
        let ratio = self.sample_rate as f32 / target_rate;

        let target_len = (mono_data.len() as f32 / ratio) as usize;
        for i in 0..target_len {
            let src_pos = i as f32 * ratio;
            let src_idx = src_pos.floor() as usize;
            let frac = src_pos - src_idx as f32;

            if src_idx + 1 < mono_data.len() {
                let s0 = mono_data[src_idx];
                let s1 = mono_data[src_idx + 1];
                let sample = s0 + frac * (s1 - s0);
                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                self.frame_buffer.push(sample_i16);
                self.resample_buffer.push(sample);
            } else if src_idx < mono_data.len() {
                let sample = mono_data[src_idx];
                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                self.frame_buffer.push(sample_i16);
                self.resample_buffer.push(sample);
            }
        }

        let frame_size = 160;
        const LONG_SILENCE:   usize = 700; // ~7s auto-stop
        const HANGOVER_LIMIT: usize = 25;  // 250ms hangover

        while self.frame_buffer.len() >= frame_size {
            let frame: Vec<i16> = self.frame_buffer.drain(0..frame_size).collect();
            let resampled_segment: Vec<f32> = self.resample_buffer.drain(0..frame_size).collect();

            match self.vad.is_voice_segment(&frame) {
                Ok(is_voice) => {
                    // Warmup: ignore first 100 frames (~1s) to let mic settle
                    let is_voice = if self.warmup_counter < 100 {
                        self.warmup_counter += 1;
                        false
                    } else {
                        is_voice
                    };

                    let mut has_speech = is_voice;

                    if is_voice {
                        self.hangover_counter = HANGOVER_LIMIT;
                        self.voice_activation_counter += 1;
                    } else {
                        self.voice_activation_counter = 0;
                        if self.hangover_counter > 0 {
                            self.hangover_counter -= 1;
                            has_speech = true;
                        }
                    }

                    if has_speech {
                        if !self.is_speech_active {
                            // Require 120ms of continuous voice before activating
                            if self.voice_activation_counter >= 12 {
                                println!("[VAD] Speech Detected - Active");
                                self.is_speech_active = true;
                                self.silence_counter = 0;
                                if let Some(ref tx) = self.status_tx {
                                    let _ = tx.send(AudioStatus::Active);
                                }
                            }
                        } else {
                            // Already active — reset silence counter on resumed speech
                            if self.voice_activation_counter >= 5 || self.hangover_counter > 0 {
                                if self.silence_counter >= 100 {
                                    println!("[VAD] Resuming speech - Active");
                                    if let Some(ref tx) = self.status_tx {
                                        let _ = tx.send(AudioStatus::Active);
                                    }
                                }
                                self.silence_counter = 0;
                            }
                        }
                    } else {
                        // Only count silence AFTER speech has started
                        if self.is_speech_active {
                            self.silence_counter += 1;

                            if self.silence_counter == 100 {
                                println!("[VAD] Brief pause (1s)");
                                if let Some(ref tx) = self.status_tx {
                                    let _ = tx.send(AudioStatus::Listening);
                                }
                            }

                            if self.silence_counter >= LONG_SILENCE {
                                self.is_speech_active = false;
                                self.silence_counter = 0;
                                println!("[VAD] Auto-Stop — 7s silence");
                                if let Some(ref tx) = self.status_tx {
                                    let _ = tx.send(AudioStatus::Idle);
                                }
                            }
                        }
                        // If not speech active — silence counter stays at 0
                        // App will never auto-stop before user speaks
                    }

                    // tx1 — always send to main thread (UI levels)
                    let _ = tx1.send(resampled_segment.clone());

                    // FIX: tx2 — send to AI worker on has_speech, not is_speech_active
                    // This ensures audio is captured even when is_speech_active
                    // hasn't been set yet (first 120ms) or after a pause resumes
                    if has_speech {
                        let _ = tx2.send(resampled_segment.clone());
                    }

                    // Level reporting — throttled to every 100ms (10 frames)
                    self.level_counter += 1;
                    if self.level_counter % 10 == 0 {
                        let rms = (resampled_segment.iter()
                            .map(|x| x * x)
                            .sum::<f32>() / resampled_segment.len() as f32)
                            .sqrt();
                        let scaled_rms = (rms * RMS_SCALE).min(1.0);
                        if let Some(ref tx) = self.status_tx {
                            let _ = tx.send(AudioStatus::Level { level: scaled_rms });
                        }
                    }
                }
                Err(e) => eprintln!("VAD Error: {:?}", e),
            }
        }
    }
}