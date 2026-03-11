//! ASCII art banners with gradient coloring for CLI output.

use colored::Colorize;

/// Renders styled ASCII banners for various processing stages.
pub struct BannerManager {
    stderr_mode: bool,
}

/// Apply a vertical gradient across lines, interpolating from `top` to `bot` RGB.
fn gradient_lines(lines: &[&str], top: [u8; 3], bot: [u8; 3], stderr_mode: bool) {
    let n = lines.len().max(1) as f32;
    for (i, line) in lines.iter().enumerate() {
        let t = i as f32 / (n - 1.0).max(1.0);
        let r = (top[0] as f32 + (bot[0] as f32 - top[0] as f32) * t) as u8;
        let g = (top[1] as f32 + (bot[1] as f32 - top[1] as f32) * t) as u8;
        let b = (top[2] as f32 + (bot[2] as f32 - top[2] as f32) * t) as u8;
        if stderr_mode {
            eprintln!("{}", line.truecolor(r, g, b).bold());
        } else {
            println!("{}", line.truecolor(r, g, b).bold());
        }
    }
}

impl BannerManager {
    /// Create a new banner manager that prints to stdout.
    pub fn new() -> Self {
        Self { stderr_mode: false }
    }

    /// Create a banner manager that prints to stderr (for JSON mode).
    pub fn stderr() -> Self {
        Self { stderr_mode: true }
    }

    fn out(&self, args: std::fmt::Arguments<'_>) {
        if self.stderr_mode {
            eprintln!("{args}");
        } else {
            println!("{args}");
        }
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

        gradient_lines(lines, [0, 220, 255], [140, 80, 255], self.stderr_mode);

        self.out(format_args!(
            "{}",
            "    ─╴╶─╴╶─▁▂▃▄▅▆▇█▇▆▅▄▃▂▁─╴╶─▁▂▃▅▇█▇▅▃▂▁─╴╶─╴╶─".truecolor(80, 140, 220)
        ));
        self.out(format_args!(""));
        self.out(format_args!(
            "    {}    {}",
            "Audio Forensics & Sanitization Engine"
                .truecolor(160, 180, 220)
                .bold(),
            "v2.0".truecolor(100, 100, 140)
        ));
        self.out(format_args!(
            "    {}",
            "─╴╶─╴╶─▁▂▃▅▇█▇▅▃▂▁─╴╶─▁▂▃▄▅▆▇█▇▆▅▄▃▂▁─╴╶─╴╶─".truecolor(60, 100, 180)
        ));
        self.out(format_args!(""));
    }

    /// Display the main banner followed by version and feature details.
    pub fn show_version_info(&self) {
        self.show_main_banner();
        self.out(format_args!(
            "  Build:    Rust {}",
            env!("CARGO_PKG_VERSION")
        ));
        self.out(format_args!(
            "  Target:   Audio watermarks, metadata, and fingerprints"
        ));
        self.out(format_args!(
            "  Features: Spectral cleaning, statistical normalization,"
        ));
        self.out(format_args!(
            "            fingerprint removal, batch processing"
        ));
        self.out(format_args!(""));
        self.out(format_args!(
            "{}",
            "  LEGAL NOTICE: This tool is for authorized security research only."
                .red()
                .bold()
        ));
        self.out(format_args!(
            "  Educational purposes only. Use responsibly and ethically."
        ));
        self.out(format_args!(""));
    }

    /// Display a banner indicating sanitization is in progress.
    pub fn show_processing_banner(&self) {
        let lines: &[&str] = &[
            "",
            "    ░░▒▒▓▓██ SANITIZATION IN PROGRESS ██▓▓▒▒░░",
            "    ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁ scrubbing ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁",
            "",
        ];
        gradient_lines(lines, [0, 200, 220], [0, 140, 180], self.stderr_mode);
    }

    /// Display a banner indicating sanitization completed successfully.
    pub fn show_success_banner(&self) {
        let lines: &[&str] = &[
            "",
            "    ░░▒▒▓▓██ SANITIZATION COMPLETE ██▓▓▒▒░░",
            "    ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁ all clear ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁",
            "",
        ];
        gradient_lines(lines, [0, 220, 120], [0, 160, 80], self.stderr_mode);
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
        gradient_lines(&refs, [0, 220, 120], [0, 160, 80], self.stderr_mode);
    }
}
