//! Optional native progress rendering shared by library phases.

#[cfg(all(not(target_arch = "wasm32"), feature = "progress"))]
use indicatif::{ProgressBar, ProgressStyle};

/// A phase-specific progress bar which is a no-op on portable builds.
pub(crate) struct PhaseProgress {
    #[cfg(all(not(target_arch = "wasm32"), feature = "progress"))]
    bar: Option<ProgressBar>,
}

impl PhaseProgress {
    pub(crate) fn new(length: u64, quiet: bool, label: &str) -> Self {
        #[cfg(all(not(target_arch = "wasm32"), feature = "progress"))]
        {
            let bar = if quiet {
                None
            } else {
                let bar = ProgressBar::new(length);
                let template = format!("{label} [{{bar:40.cyan/blue}}] {{pos}}/{{len}} {{msg}}");
                let style = ProgressStyle::with_template(&template)
                    .unwrap_or_else(|_| ProgressStyle::default_bar());
                bar.set_style(style);
                Some(bar)
            };
            Self { bar }
        }
        #[cfg(not(all(not(target_arch = "wasm32"), feature = "progress")))]
        {
            let _ = (length, quiet, label);
            Self {}
        }
    }

    pub(crate) fn inc(&self, amount: u64) {
        #[cfg(all(not(target_arch = "wasm32"), feature = "progress"))]
        if let Some(bar) = &self.bar {
            bar.inc(amount);
        }
        #[cfg(not(all(not(target_arch = "wasm32"), feature = "progress")))]
        let _ = amount;
    }

    pub(crate) fn set(&self, position: u64, message: impl FnOnce() -> String) {
        #[cfg(all(not(target_arch = "wasm32"), feature = "progress"))]
        if let Some(bar) = &self.bar {
            bar.set_position(position);
            bar.set_message(message());
        }
        #[cfg(not(all(not(target_arch = "wasm32"), feature = "progress")))]
        let _ = (position, message);
    }

    pub(crate) fn finish(&self, message: Option<String>) {
        #[cfg(all(not(target_arch = "wasm32"), feature = "progress"))]
        if let Some(bar) = &self.bar {
            if let Some(message) = message {
                bar.finish_with_message(message);
            } else {
                bar.finish_and_clear();
            }
        }
        #[cfg(not(all(not(target_arch = "wasm32"), feature = "progress")))]
        let _ = message;
    }
}

impl Drop for PhaseProgress {
    fn drop(&mut self) {
        #[cfg(all(not(target_arch = "wasm32"), feature = "progress"))]
        if let Some(bar) = &self.bar
            && !bar.is_finished()
        {
            bar.finish_and_clear();
        }
    }
}
