use colored::Colorize;

pub struct BannerManager;

impl BannerManager {
    pub fn new() -> Self {
        Self
    }

    pub fn show_main_banner(&self) {
        let banner = r#"
  ╔═══════════════════════════════════════════════════════════╗
  ║                                                           ║
  ║           P O L E Z  v2.0  [Rust Edition]                 ║
  ║        Audio Forensics & Sanitization Engine               ║
  ║                                                           ║
  ║     "The silent hand that scrubs the score"               ║
  ║                                                           ║
  ╚═══════════════════════════════════════════════════════════╝
"#;
        println!("{}", banner.blue().bold());
    }

    pub fn show_version_info(&self) {
        println!("{}", "Polez v2.0.0 [Rust Edition]".cyan().bold());
        println!();
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

    pub fn show_processing_banner(&self) {
        let banner = r#"
  ╔══════════════════════════════╗
  ║  SANITIZATION IN PROGRESS   ║
  ║  Stand by...                ║
  ╚══════════════════════════════╝
"#;
        println!("{}", banner.cyan().bold());
    }

    pub fn show_success_banner(&self) {
        let banner = r#"
  ╔══════════════════════════════╗
  ║    SANITIZATION COMPLETE    ║
  ║    All threats eliminated   ║
  ╚══════════════════════════════╝
"#;
        println!("{}", banner.green().bold());
    }

    pub fn show_batch_complete_banner(&self, count: usize) {
        println!();
        println!("{}", "  ╔══════════════════════════════╗".green().bold());
        println!("{}", "  ║    BATCH MASSACRE COMPLETE   ║".green().bold());
        println!(
            "  {}",
            format!("  ║    {count} files processed       ║")
                .green()
                .bold()
        );
        println!("{}", "  ╚══════════════════════════════╝".green().bold());
        println!();
    }
}
