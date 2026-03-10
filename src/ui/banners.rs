//! ASCII art banners with gradient coloring for CLI output.

use colored::Colorize;

/// Renders styled ASCII banners for various processing stages.
pub struct BannerManager;

/// Apply a vertical gradient across lines, interpolating from `top` to `bot` RGB.
fn gradient_lines(lines: &[&str], top: [u8; 3], bot: [u8; 3]) {
    let n = lines.len().max(1) as f32;
    for (i, line) in lines.iter().enumerate() {
        let t = i as f32 / (n - 1.0).max(1.0);
        let r = (top[0] as f32 + (bot[0] as f32 - top[0] as f32) * t) as u8;
        let g = (top[1] as f32 + (bot[1] as f32 - top[1] as f32) * t) as u8;
        let b = (top[2] as f32 + (bot[2] as f32 - top[2] as f32) * t) as u8;
        println!("{}", line.truecolor(r, g, b).bold());
    }
}

impl BannerManager {
    /// Create a new banner manager.
    pub fn new() -> Self {
        Self
    }

    /// Display the main Polez ASCII art banner with gradient coloring.
    pub fn show_main_banner(&self) {
        let lines: &[&str] = &[
            "",
            r"      ██████╗  ██████╗ ██╗     ███████╗███████╗",
            r"      ██╔══██╗██╔═══██╗██║     ██╔════╝╚══███╔╝",
            r"      ██████╔╝██║   ██║██║     █████╗    ███╔╝ ",
            r"      ██╔═══╝ ██║   ██║██║     ██╔══╝   ███╔╝  ",
            r"      ██║     ╚██████╔╝███████╗███████╗███████╗",
            r"      ╚═╝      ╚═════╝ ╚══════╝╚══════╝╚══════╝",
            "",
        ];

        gradient_lines(lines, [0, 220, 255], [140, 80, 255]);

        println!(
            "{}",
            "    ─╴╶─╴╶─▁▂▃▄▅▆▇█▇▆▅▄▃▂▁─╴╶─▁▂▃▅▇█▇▅▃▂▁─╴╶─╴╶─".truecolor(80, 140, 220)
        );
        println!();
        println!(
            "    {}    {}",
            "Audio Forensics & Sanitization Engine"
                .truecolor(160, 180, 220)
                .bold(),
            "v2.0".truecolor(100, 100, 140)
        );
        println!(
            "    {}",
            "─╴╶─╴╶─▁▂▃▅▇█▇▅▃▂▁─╴╶─▁▂▃▄▅▆▇█▇▆▅▄▃▂▁─╴╶─╴╶─".truecolor(60, 100, 180)
        );
        println!();
    }

    /// Display the main banner followed by version and feature details.
    pub fn show_version_info(&self) {
        self.show_main_banner();
        println!("  Build:    Rust {}", env!("CARGO_PKG_VERSION"));
        println!("  Target:   Audio watermarks, metadata, and fingerprints");
        println!("  Features: Spectral cleaning, statistical normalization,");
        println!("            fingerprint removal, batch processing");
        println!();
        println!(
            "{}",
            "  LEGAL NOTICE: This tool is for authorized security research only."
                .red()
                .bold()
        );
        println!("  Educational purposes only. Use responsibly and ethically.");
        println!();
    }

    /// Display a banner indicating sanitization is in progress.
    pub fn show_processing_banner(&self) {
        let lines: &[&str] = &[
            "",
            "    ░░▒▒▓▓██ SANITIZATION IN PROGRESS ██▓▓▒▒░░",
            "    ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁ scrubbing ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁",
            "",
        ];
        gradient_lines(lines, [0, 200, 220], [0, 140, 180]);
    }

    /// Display a banner indicating sanitization completed successfully.
    pub fn show_success_banner(&self) {
        let lines: &[&str] = &[
            "",
            "    ░░▒▒▓▓██ SANITIZATION COMPLETE ██▓▓▒▒░░",
            "    ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁ all clear ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁",
            "",
        ];
        gradient_lines(lines, [0, 220, 120], [0, 160, 80]);
    }

    /// Display a banner indicating batch sweep completed with the file count.
    pub fn show_batch_complete_banner(&self, count: usize) {
        let lines: Vec<String> = vec![
            String::new(),
            "    ░░▒▒▓▓██ BATCH SWEEP COMPLETE ██▓▓▒▒░░".to_string(),
            format!("    ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁ {count} files ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁"),
            String::new(),
        ];
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        gradient_lines(&refs, [0, 220, 120], [0, 160, 80]);
    }
}
