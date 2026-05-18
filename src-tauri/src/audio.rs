use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::Sender;
use webrtc_vad::{Vad, VadMode, SampleRate};
use crate::state::AudioStatus;

const RMS_SCALE: f32 = 10.0; // Amplify RMS for better visual response

pub struct AudioProcessor {
    vad: Vad,
    sample_rate: u32,
    resample_buffer: Vec<f32>,
    frame_buffer: Vec<i16>,
    is_speech_active: bool,
    silence_counter: usize,
    status_tx: Option<Sender<AudioStatus>>,
}

impl AudioProcessor {
    pub fn new(input_sample_rate: u32, status_tx: Option<Sender<AudioStatus>>) -> Self {
        // VAD always runs on 16kHz in our pipeline
        // Mode 'Aggressive' is better for filtering background noise/keyboard clicks
        let vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive);

        Self {
            vad,
            sample_rate: input_sample_rate,
            resample_buffer: Vec::new(),
            frame_buffer: Vec::new(),
            is_speech_active: false,
            silence_counter: 0,
            status_tx,
        }
    }

    pub fn start_listening(tx: Sender<Vec<f32>>, status_tx: Sender<AudioStatus>) -> Result<cpal::Stream, String> {
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
                processor.process_audio(&data, channels, &tx);
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

    fn process_audio(&mut self, data: &[f32], channels: u16, tx: &Sender<Vec<f32>>) {
        if data.is_empty() { return; }
        
        // Average channels for better mono conversion
        let mono_data: Vec<f32> = if channels > 1 {
            data.chunks(channels as usize)
                .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                .collect()
        } else {
            data.to_vec()
        };

        let target_rate = 16000.0;
        let ratio = self.sample_rate as f32 / target_rate;
        
        // Simple linear resampling
        for i in 0..((mono_data.len() as f32 / ratio) as usize) {
            let src_index = (i as f32 * ratio) as usize;
            if src_index < mono_data.len() {
                let sample = mono_data[src_index];
                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                self.frame_buffer.push(sample_i16);
                self.resample_buffer.push(sample);
            }
        }

        // VAD Frame size must be 10ms, 20ms, or 30ms. At 16kHz: 160, 320, 480.
        let frame_size = 160; 
        while self.frame_buffer.len() >= frame_size {
            let frame: Vec<i16> = self.frame_buffer.drain(0..frame_size).collect();
            let resampled_segment: Vec<f32> = self.resample_buffer.drain(0..frame_size).collect();

            // ALWAYS send the audio to the processing thread (Thread 1/2)
            // This preserves natural pauses for better AI accuracy.
            let _ = tx.send(resampled_segment.clone());

            match self.vad.is_voice_segment(&frame) {
                Ok(is_voice) => {
                    if is_voice {
                        self.silence_counter = 0;
                        if !self.is_speech_active {
                            println!("[VAD] Speech Detected - Active");
                            self.is_speech_active = true;
                            if let Some(ref tx) = self.status_tx { let _ = tx.send(AudioStatus::Active); }
                        }
                        
                        // Calculate audio level for visualization
                        let rms = (resampled_segment.iter().map(|x| x * x).sum::<f32>() / resampled_segment.len() as f32).sqrt();
                        let scaled_rms = (rms * RMS_SCALE).min(1.0);
                        if let Some(ref tx) = self.status_tx {
                            let _ = tx.send(AudioStatus::Level { level: scaled_rms });
                        }
                    } else {
                        if self.is_speech_active {
                            self.silence_counter += 1;

                            // Warning at 5 seconds (500 frames)
                            if self.silence_counter == 500 {
                                println!("[VAD] Silence warning - 5s");
                                if let Some(ref tx) = self.status_tx {
                                    let _ = tx.send(AudioStatus::Warning { duration: 5 });
                                }
                            }

                            // Send zero level when no voice
                            if let Some(ref tx) = self.status_tx {
                                let _ = tx.send(AudioStatus::Level { level: 0.0 });
                            }

                            if self.silence_counter > 500 { // ~5 seconds of silence
                                self.is_speech_active = false;
                                self.silence_counter = 0;
                                println!("[VAD] Auto-Stop — silence detected");
                                if let Some(ref tx) = self.status_tx { let _ = tx.send(AudioStatus::Idle); }
                            }
                        } else {
                            // Still send zero level when idle
                            if let Some(ref tx) = self.status_tx {
                                let _ = tx.send(AudioStatus::Level { level: 0.0 });
                            }
                        }
                    }
                }
                Err(e) => eprintln!("VAD Error: {:?}", e),
            }
        }
    }
}
