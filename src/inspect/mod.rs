//! Visualization tools for watermark inspection.

use crate::audio::AudioBuffer;
use crate::error::Result;
use colored::Colorize;
use std::fmt::Write as FmtWrite;
use std::path::Path;

struct SpectrogramData {
    spectrogram: Vec<Vec<f64>>,
    min_db: f64,
    max_db: f64,
    sample_rate: f64,
    window_size: usize,
}

fn intensity_to_rgb(normalized: f64) -> (u8, u8, u8) {
    // Dark blue -> cyan -> green -> yellow -> red
    let (r, g, b) = if normalized < 0.2 {
        let t = normalized / 0.2;
        (0.0, 0.0, 0.3 + 0.7 * t)
    } else if normalized < 0.4 {
        let t = (normalized - 0.2) / 0.2;
        (0.0, t, 1.0)
    } else if normalized < 0.6 {
        let t = (normalized - 0.4) / 0.2;
        (0.0, 1.0, 1.0 - t)
    } else if normalized < 0.8 {
        let t = (normalized - 0.6) / 0.2;
        (t, 1.0, 0.0)
    } else {
        let t = (normalized - 0.8) / 0.2;
        (1.0, 1.0 - t, 0.0)
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Spectrogram visualization for high-frequency watermark detection.
pub struct SpectrogramView {
    pub freq_min: u32,
    pub freq_max: u32,
    pub start_secs: f64,
    pub duration_secs: f64,
}

impl SpectrogramView {
    pub fn new(freq_min: u32, freq_max: u32, start_secs: f64, duration_secs: f64) -> Self {
        Self {
            freq_min,
            freq_max,
            start_secs,
            duration_secs,
        }
    }

    fn compute_spectrogram(&self, buffer: &AudioBuffer) -> Option<SpectrogramData> {
        let sample_rate = buffer.sample_rate as f64;
        let samples = buffer.to_mono_samples();

        let start_sample = (self.start_secs * sample_rate) as usize;
        let duration_samples = if self.duration_secs > 0.0 {
            (self.duration_secs * sample_rate) as usize
        } else {
            samples.len() - start_sample
        };
        let end_sample = (start_sample + duration_samples).min(samples.len());
        let chunk = &samples[start_sample..end_sample];

        let window_size = 2048;
        let hop = 256;
        let n_windows = (chunk.len().saturating_sub(window_size)) / hop;

        if n_windows < 10 {
            return None;
        }

        let mut spectrogram: Vec<Vec<f64>> = Vec::new();
        let hann: Vec<f64> = (0..window_size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / window_size as f64).cos())
            })
            .collect();

        for w in 0..n_windows {
            let start = w * hop;
            let mut windowed: Vec<f64> = chunk[start..start + window_size]
                .iter()
                .zip(hann.iter())
                .map(|(s, h)| *s as f64 * h)
                .collect();

            let fft_size = window_size;
            windowed.resize(fft_size, 0.0);

            let freq_resolution = sample_rate / fft_size as f64;
            let bin_min = (self.freq_min as f64 / freq_resolution) as usize;
            let bin_max =
                (self.freq_max as f64 / freq_resolution).min((fft_size / 2) as f64) as usize;

            let mut spectrum = Vec::new();
            for k in bin_min..=bin_max {
                let mut re = 0.0;
                let mut im = 0.0;
                for (n, sample) in windowed.iter().enumerate() {
                    let angle = 2.0 * std::f64::consts::PI * k as f64 * n as f64 / fft_size as f64;
                    re += sample * angle.cos();
                    im -= sample * angle.sin();
                }
                let magnitude = (re * re + im * im).sqrt();
                spectrum.push(20.0 * (magnitude + 1e-10).log10());
            }
            spectrogram.push(spectrum);
        }

        let mut min_db = f64::INFINITY;
        let mut max_db = f64::NEG_INFINITY;
        for row in &spectrogram {
            for &val in row {
                if val.is_finite() {
                    min_db = min_db.min(val);
                    max_db = max_db.max(val);
                }
            }
        }

        Some(SpectrogramData {
            spectrogram,
            min_db,
            max_db,
            sample_rate,
            window_size,
        })
    }

    /// Export spectrogram as SVG file.
    pub fn export_svg(&self, buffer: &AudioBuffer, path: &Path) -> Result<()> {
        let data = self.compute_spectrogram(buffer).ok_or_else(|| {
            crate::error::PolezError::Dsp("Not enough audio data for spectrogram".into())
        })?;

        let margin_left = 70.0_f64;
        let margin_top = 30.0_f64;
        let margin_right = 20.0_f64;
        let margin_bottom = 50.0_f64;
        let plot_width = 800.0_f64;
        let plot_height = 400.0_f64;
        let total_width = margin_left + plot_width + margin_right;
        let total_height = margin_top + plot_height + margin_bottom;

        let freq_bins = data.spectrogram.first().map(|r| r.len()).unwrap_or(1);
        let time_bins = data.spectrogram.len();
        let cell_w = plot_width / time_bins as f64;
        let cell_h = plot_height / freq_bins as f64;

        let mut svg = String::with_capacity(1024 * 64);

        let _ = writeln!(
            svg,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total_width}" height="{total_height}" viewBox="0 0 {total_width} {total_height}">"##
        );
        let _ = writeln!(
            svg,
            r##"<rect width="{total_width}" height="{total_height}" fill="#1a1a2e"/>"##
        );
        let _ = writeln!(
            svg,
            r##"<text x="{:.0}" y="20" fill="#e0e0e0" font-family="monospace" font-size="14" text-anchor="middle">Spectrogram: {}-{} kHz ({:.1}s - {:.1}s)</text>"##,
            total_width / 2.0,
            self.freq_min / 1000,
            self.freq_max / 1000,
            self.start_secs,
            self.start_secs + self.duration_secs
        );

        for (ti, spectrum) in data.spectrogram.iter().enumerate() {
            for (fi, &val) in spectrum.iter().enumerate() {
                let normalized =
                    ((val - data.min_db) / (data.max_db - data.min_db + 1e-10)).clamp(0.0, 1.0);
                let (r, g, b) = intensity_to_rgb(normalized);
                let x = margin_left + ti as f64 * cell_w;
                let y = margin_top + plot_height - (fi + 1) as f64 * cell_h;
                let _ = write!(
                    svg,
                    r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="rgb({},{},{})"/>"##,
                    x,
                    y,
                    cell_w.ceil(),
                    cell_h.ceil(),
                    r,
                    g,
                    b
                );
            }
        }

        // Frequency axis labels
        let num_freq_labels: usize = 6;
        for i in 0..=num_freq_labels {
            let freq = self.freq_min as f64
                + (i as f64 / num_freq_labels as f64) * (self.freq_max - self.freq_min) as f64;
            let y = margin_top + plot_height - (i as f64 / num_freq_labels as f64) * plot_height;
            let _ = write!(
                svg,
                r##"<text x="{:.0}" y="{:.0}" fill="#e0e0e0" font-family="monospace" font-size="11" text-anchor="end">{:.1}k</text>"##,
                margin_left - 5.0,
                y + 4.0,
                freq / 1000.0
            );
            let _ = write!(
                svg,
                r##"<line x1="{}" y1="{:.0}" x2="{:.0}" y2="{:.0}" stroke="#444" stroke-width="0.5"/>"##,
                margin_left,
                y,
                margin_left + plot_width,
                y
            );
        }

        // Time axis labels
        let num_time_labels: usize = 8;
        let time_range = self.duration_secs;
        for i in 0..=num_time_labels {
            let t = self.start_secs + (i as f64 / num_time_labels as f64) * time_range;
            let x = margin_left + (i as f64 / num_time_labels as f64) * plot_width;
            let _ = write!(
                svg,
                r##"<text x="{:.0}" y="{:.0}" fill="#e0e0e0" font-family="monospace" font-size="11" text-anchor="middle">{:.2}s</text>"##,
                x,
                margin_top + plot_height + 20.0,
                t
            );
        }

        // Axis labels
        let freq_label_y = margin_top + plot_height / 2.0;
        let _ = write!(
            svg,
            r##"<text x="15" y="{freq_label_y:.0}" fill="#e0e0e0" font-family="monospace" font-size="12" text-anchor="middle" transform="rotate(-90,15,{freq_label_y:.0})">Frequency (kHz)</text>"##
        );
        let _ = write!(
            svg,
            r##"<text x="{:.0}" y="{:.0}" fill="#e0e0e0" font-family="monospace" font-size="12" text-anchor="middle">Time (s)</text>"##,
            margin_left + plot_width / 2.0,
            margin_top + plot_height + 42.0
        );

        // Plot border
        let _ = write!(
            svg,
            r##"<rect x="{margin_left}" y="{margin_top}" width="{plot_width}" height="{plot_height}" fill="none" stroke="#666" stroke-width="1"/>"##
        );

        svg.push_str("</svg>");

        std::fs::write(path, &svg)?;

        Ok(())
    }

    /// Render ASCII spectrogram to console.
    pub fn render(&self, buffer: &AudioBuffer) -> Result<()> {
        let data = match self.compute_spectrogram(buffer) {
            Some(d) => d,
            None => {
                println!("{}", "Not enough audio data for spectrogram".red());
                return Ok(());
            }
        };

        let SpectrogramData {
            ref spectrogram,
            min_db,
            max_db,
            sample_rate,
            window_size,
        } = data;

        // Render ASCII
        let display_rows = 24;
        let display_cols = 80;
        let freq_bins = spectrogram.first().map(|r| r.len()).unwrap_or(0);

        println!();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════════════════════════════╗"
                .cyan()
        );
        println!("{}", format!("║  SPECTROGRAM VIEW: {}-{} kHz                                                      ║",
            self.freq_min / 1000, self.freq_max / 1000).cyan());
        println!(
            "{}",
            "╠══════════════════════════════════════════════════════════════════════════════════╣"
                .cyan()
        );

        let chars = [' ', '░', '▒', '▓', '█'];

        for row in (0..display_rows).rev() {
            let freq_idx = (row as f64 / display_rows as f64 * freq_bins as f64) as usize;
            let freq_hz = self.freq_min as f64
                + (row as f64 / display_rows as f64) * (self.freq_max - self.freq_min) as f64;

            print!("{}", format!("║ {:5.1}k │", freq_hz / 1000.0).cyan());

            for col in 0..display_cols {
                let time_idx =
                    (col as f64 / display_cols as f64 * spectrogram.len() as f64) as usize;
                let time_idx = time_idx.min(spectrogram.len().saturating_sub(1));
                let freq_idx = freq_idx.min(freq_bins.saturating_sub(1));

                let val = if time_idx < spectrogram.len() && freq_idx < spectrogram[time_idx].len()
                {
                    spectrogram[time_idx][freq_idx]
                } else {
                    min_db
                };

                let normalized = ((val - min_db) / (max_db - min_db + 1e-10)).clamp(0.0, 1.0);
                let char_idx = (normalized * (chars.len() - 1) as f64) as usize;

                // Color based on intensity
                let c = chars[char_idx];
                if normalized > 0.8 {
                    print!("{}", format!("{c}").red().bold());
                } else if normalized > 0.6 {
                    print!("{}", format!("{c}").yellow());
                } else if normalized > 0.4 {
                    print!("{}", format!("{c}").green());
                } else if normalized > 0.2 {
                    print!("{}", format!("{c}").blue());
                } else {
                    print!("{}", format!("{c}").white().dimmed());
                }
            }
            println!("{}", "│║".cyan());
        }

        println!(
            "{}",
            "╠═══════╧════════════════════════════════════════════════════════════════════════╣"
                .cyan()
        );

        let time_range = self.duration_secs;
        println!(
            "{}",
            format!(
                "║  Time: {:.1}s - {:.1}s {:>60} ║",
                self.start_secs,
                self.start_secs + time_range,
                ""
            )
            .cyan()
        );

        // Energy analysis
        let mut band_energy: Vec<(String, f64)> = Vec::new();
        let bands = [
            (15000, 17000),
            (17000, 19000),
            (19000, 21000),
            (21000, 23000),
            (23000, 24000),
        ];

        let freq_resolution = sample_rate / window_size as f64;
        let bin_min_base = (self.freq_min as f64 / freq_resolution) as usize;

        for (low, high) in bands {
            if high <= self.freq_max && low >= self.freq_min {
                let bin_low =
                    ((low as f64 / freq_resolution) as usize).saturating_sub(bin_min_base);
                let bin_high =
                    ((high as f64 / freq_resolution) as usize).saturating_sub(bin_min_base);

                let mut sum = 0.0;
                let mut count = 0;
                for row in spectrogram {
                    for val in row.iter().take(bin_high.min(row.len())).skip(bin_low) {
                        sum += 10_f64.powf(val / 20.0);
                        count += 1;
                    }
                }
                let avg = if count > 0 { sum / count as f64 } else { 0.0 };
                band_energy.push((format!("{}-{}k", low / 1000, high / 1000), avg));
            }
        }

        println!(
            "{}",
            "╠══════════════════════════════════════════════════════════════════════════════════╣"
                .cyan()
        );
        println!(
            "{}",
            "║  FREQUENCY BAND ENERGY:                                                          ║"
                .cyan()
        );

        let total: f64 = band_energy.iter().map(|(_, e)| e).sum();
        for (name, energy) in &band_energy {
            let pct = if total > 0.0 {
                energy / total * 100.0
            } else {
                0.0
            };
            let bar_len = (pct / 100.0 * 50.0) as usize;
            let bar: String = "█".repeat(bar_len);
            let warning = if name.contains("23") && pct > 25.0 {
                " ← AI WM?"
            } else {
                ""
            };
            println!(
                "{}",
                format!("║  {name:>7}: {bar:50} {pct:5.1}%{warning:8} ║").cyan()
            );
        }

        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════════════════════════╝"
                .cyan()
        );

        Ok(())
    }
}

/// Bit pattern visualization for LSB watermark detection.
pub struct BitsView {
    pub bit_plane: u8,
    pub offset: usize,
    pub count: usize,
    pub search_strings: bool,
}

impl BitsView {
    pub fn new(bit_plane: u8, offset: usize, count: usize, search_strings: bool) -> Self {
        Self {
            bit_plane,
            offset,
            count,
            search_strings,
        }
    }

    /// Render bit pattern analysis to console.
    pub fn render(&self, buffer: &AudioBuffer) -> Result<()> {
        let samples = buffer.to_mono_samples();

        if self.offset >= samples.len() {
            println!("{}", "Offset exceeds audio length".red());
            return Ok(());
        }

        let end = (self.offset + self.count).min(samples.len());
        let chunk = &samples[self.offset..end];

        // Convert samples to i16 and extract bit plane
        let samples_i16: Vec<i16> = chunk.iter().map(|s| (*s * 32767.0) as i16).collect();
        let bits: Vec<u8> = samples_i16
            .iter()
            .map(|s| ((s >> self.bit_plane) & 1) as u8)
            .collect();

        println!();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════════════════════════════╗"
                .green()
        );
        println!(
            "{}",
            format!(
                "║  BIT PLANE {} ({}): Samples {} - {}                                     ║",
                self.bit_plane,
                if self.bit_plane == 0 {
                    "LSB"
                } else if self.bit_plane == 7 {
                    "MSB"
                } else {
                    "   "
                },
                self.offset,
                end
            )
            .green()
        );
        println!(
            "{}",
            "╠══════════════════════════════════════════════════════════════════════════════════╣"
                .green()
        );

        // Statistics
        let ones: usize = bits.iter().map(|b| *b as usize).sum();
        let ratio = ones as f64 / bits.len() as f64;
        let expected = 0.5;
        let deviation = (ratio - expected).abs();

        let bias_warning = if deviation > 0.01 {
            format!(" ← BIAS DETECTED ({:+.2}%)", (ratio - expected) * 100.0)
                .red()
                .to_string()
        } else {
            "".to_string()
        };

        println!(
            "{}",
            format!(
                "║  Ones: {} ({:.2}%)  Zeros: {} ({:.2}%){}                    ║",
                ones,
                ratio * 100.0,
                bits.len() - ones,
                (1.0 - ratio) * 100.0,
                bias_warning
            )
            .green()
        );

        println!(
            "{}",
            "╠══════════════════════════════════════════════════════════════════════════════════╣"
                .green()
        );
        println!(
            "{}",
            "║  RAW BIT PATTERN (first 512 bits):                                               ║"
                .green()
        );

        // Display raw bits in rows of 64
        for row in 0..8 {
            let start = row * 64;
            let end = (start + 64).min(bits.len());
            if start >= bits.len() {
                break;
            }

            let bit_str: String = bits[start..end]
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    if i > 0 && i % 8 == 0 {
                        format!(" {b}")
                    } else {
                        format!("{b}")
                    }
                })
                .collect();

            println!("{}", format!("║  {start:04X}: {bit_str}  ║").green());
        }

        // Autocorrelation analysis
        println!(
            "{}",
            "╠══════════════════════════════════════════════════════════════════════════════════╣"
                .green()
        );
        println!(
            "{}",
            "║  AUTOCORRELATION (periodic pattern detection):                                   ║"
                .green()
        );

        let test_len = 4096.min(bits.len());
        let bits_f: Vec<f64> = bits[..test_len].iter().map(|b| *b as f64 - 0.5).collect();

        let mut peaks: Vec<(usize, f64)> = Vec::new();
        for lag in [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
            if lag >= test_len {
                continue;
            }
            let mut sum = 0.0;
            for i in 0..(test_len - lag) {
                sum += bits_f[i] * bits_f[i + lag];
            }
            let corr = sum / (test_len - lag) as f64 * 4.0; // Normalize
            if corr.abs() > 0.05 {
                peaks.push((lag, corr));
            }
        }

        if peaks.is_empty() {
            println!("{}", "║  No significant periodic patterns found                                          ║".green());
        } else {
            for (lag, corr) in peaks.iter().take(5) {
                let bar_len = (corr.abs() * 40.0) as usize;
                let bar: String = if *corr > 0.0 { "+" } else { "-" }.repeat(bar_len);
                println!(
                    "{}",
                    format!("║  Period {lag:4}: {bar:40} ({corr:+.3})     ║").green()
                );
            }
        }

        // Search for ASCII strings
        if self.search_strings {
            println!("{}", "╠══════════════════════════════════════════════════════════════════════════════════╣".green());
            println!("{}", "║  ASCII STRING SEARCH:                                                            ║".green());

            // Convert bits to bytes and search for strings
            let bytes: Vec<u8> = bits
                .chunks(8)
                .filter(|chunk| chunk.len() == 8)
                .map(|chunk| {
                    chunk
                        .iter()
                        .enumerate()
                        .fold(0u8, |acc, (i, &b)| acc | (b << i))
                })
                .collect();

            // Search for common watermark strings
            let search_terms = ["WATERMARK", "watermark", "AI", "ai", "GEN", "gen"];
            let mut found_any = false;

            for term in search_terms {
                if let Some(pos) = bytes.windows(term.len()).position(|w| w == term.as_bytes()) {
                    println!("{}", format!("║  FOUND: \"{term}\" at byte offset {pos}                                        ║").red().bold());
                    found_any = true;
                }
            }

            // Also look for printable ASCII sequences
            let mut current_str = String::new();
            let mut start_pos = 0;
            let mut strings_found: Vec<(usize, String)> = Vec::new();

            for (i, &b) in bytes.iter().enumerate() {
                if (32..127).contains(&b) {
                    if current_str.is_empty() {
                        start_pos = i;
                    }
                    current_str.push(b as char);
                } else {
                    if current_str.len() >= 4 {
                        strings_found.push((start_pos, current_str.clone()));
                    }
                    current_str.clear();
                }
            }

            if !found_any && strings_found.is_empty() {
                println!("{}", "║  No ASCII strings found in bit stream                                            ║".green());
            } else if !strings_found.is_empty() {
                println!("{}", "║  Printable strings (4+ chars):                                                   ║".green());
                for (pos, s) in strings_found.iter().take(5) {
                    println!("{}", format!("║    @{:04X}: \"{}\"                                                            ║", pos, &s[..s.len().min(30)]).yellow());
                }
            }
        }

        // Per-bit statistics across all 8 bit planes
        println!(
            "{}",
            "╠══════════════════════════════════════════════════════════════════════════════════╣"
                .green()
        );
        println!(
            "{}",
            "║  ALL BIT PLANES SUMMARY:                                                         ║"
                .green()
        );

        for bit in 0..8 {
            let plane_bits: Vec<u8> = samples_i16
                .iter()
                .take(4096)
                .map(|s| ((s >> bit) & 1) as u8)
                .collect();
            let ones: usize = plane_bits.iter().map(|b| *b as usize).sum();
            let ratio = ones as f64 / plane_bits.len() as f64;
            let deviation = (ratio - 0.5).abs() * 100.0;

            let bar_len = (ratio * 40.0) as usize;
            let bar: String = "█".repeat(bar_len);

            let label = match bit {
                0 => "LSB",
                7 => "MSB",
                _ => "   ",
            };

            let anomaly = if deviation > 1.0 { " ← ANOMALY" } else { "" };
            println!(
                "{}",
                format!(
                    "║  Bit {}: {:40} {:.2}% ones {:3} {:10}║",
                    bit,
                    bar,
                    ratio * 100.0,
                    label,
                    anomaly
                )
                .green()
            );
        }

        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════════════════════════╝"
                .green()
        );

        Ok(())
    }
}
