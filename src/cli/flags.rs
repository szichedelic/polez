//! Advanced stealth and fingerprint removal flag definitions.
//!
//! These structs map directly to CLI flags that toggle individual DSP
//! operations in the sanitization pipeline.

use clap::Args;

/// CLI flags controlling 13 individual stealth DSP operations.
#[derive(Args, Debug, Clone)]
pub struct AdvancedFlagsCli {
    /// Toggle sub-block phase dither
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub phase_dither: bool,

    /// Toggle dynamic comb masking
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub comb_mask: bool,

    /// Toggle transient micro-shift
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub transient_shift: bool,

    /// Toggle resample nudge
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub resample_nudge: bool,

    /// Toggle FFT phase noise
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub phase_noise: bool,

    /// Toggle phase swirl
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub phase_swirl: bool,

    /// Toggle masked high-frequency phase noise
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub masked_hf_phase: bool,

    /// Toggle RMS-gated resample nudge
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub gated_resample_nudge: bool,

    /// Toggle gated micro-EQ flutter
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub micro_eq_flutter: bool,

    /// Toggle HF band decorrelation
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub hf_decorrelate: bool,

    /// Toggle refined transient micro-shift
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub refined_transient: bool,

    /// Toggle adaptive transient shift (onset-strength gated)
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub adaptive_transient: bool,

    /// Toggle adaptive ultrasonic notch filter (scans and removes anomalous HF peaks)
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub adaptive_notch: bool,
}

/// CLI flags controlling fingerprint removal techniques.
#[derive(Args, Debug, Clone)]
pub struct FingerprintFlagsCli {
    /// Toggle statistical normalization (kurtosis adjustment)
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub stat_normalization: bool,

    /// Toggle temporal randomization (sample-level jitter)
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub temporal_randomization: bool,

    /// Toggle phase randomization (frequency-domain phase noise)
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub phase_randomization: bool,

    /// Toggle micro-timing perturbation (~1ms shift)
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub micro_timing: bool,

    /// Toggle human imperfections (velocity drift, micro distortion)
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    pub human_imperfections: bool,
}

impl From<FingerprintFlagsCli> for crate::config::FingerprintRemovalConfig {
    fn from(cli: FingerprintFlagsCli) -> Self {
        Self {
            statistical_normalization: cli.stat_normalization,
            temporal_randomization: cli.temporal_randomization,
            phase_randomization: cli.phase_randomization,
            micro_timing_perturbation: cli.micro_timing,
            human_imperfections: cli.human_imperfections,
        }
    }
}

impl From<AdvancedFlagsCli> for crate::config::AdvancedFlags {
    fn from(cli: AdvancedFlagsCli) -> Self {
        Self {
            phase_dither: cli.phase_dither,
            comb_mask: cli.comb_mask,
            transient_shift: cli.transient_shift,
            resample_nudge: cli.resample_nudge,
            phase_noise: cli.phase_noise,
            phase_swirl: cli.phase_swirl,
            masked_hf_phase: cli.masked_hf_phase,
            gated_resample_nudge: cli.gated_resample_nudge,
            micro_eq_flutter: cli.micro_eq_flutter,
            hf_decorrelate: cli.hf_decorrelate,
            refined_transient: cli.refined_transient,
            adaptive_transient: cli.adaptive_transient,
            adaptive_notch: cli.adaptive_notch,
        }
    }
}
