//! Avatar actions — deferred operations that require app-level processing.

/// An action requested by Lua that cannot be applied immediately
/// (e.g., requires surface reconfiguration, file I/O, or bind pose reset).
#[derive(Debug, Clone)]
pub enum AvatarAction {
    /// Apply a background image (empty string = clear).
    ApplyBackgroundImage(String),
    /// Toggle mascot mode (deferred to after surface present).
    ToggleMascot,
    /// Reset idle animation pose to bind pose.
    ResetIdlePose,
    /// Open OS native file dialog for background image.
    BrowseBackgroundImage,
    /// Reset speech capture: flush VAD queues and return to idle.
    ResetSpeech,
}

/// Queue of pending avatar actions, drained each frame by the app.
#[derive(Debug, Clone, Default)]
pub struct ActionQueue {
    actions: Vec<AvatarAction>,
}

impl ActionQueue {
    pub fn push(&mut self, action: AvatarAction) {
        self.actions.push(action);
    }

    pub fn drain(&mut self) -> Vec<AvatarAction> {
        std::mem::take(&mut self.actions)
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}
