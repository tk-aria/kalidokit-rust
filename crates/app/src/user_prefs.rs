use std::path::PathBuf;

const PREFS_FILE: &str = "user_prefs.json";

/// Persisted user preferences across sessions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserPrefs {
    pub camera_distance: f32,
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            camera_distance: 3.0,
        }
    }
}

impl UserPrefs {
    /// Load preferences from the prefs file next to the executable,
    /// falling back to defaults if the file doesn't exist or is invalid.
    pub fn load() -> Self {
        let path = prefs_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save preferences to disk. Errors are logged but not fatal.
    pub fn save(&self) {
        let path = prefs_path();
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    log::warn!("Failed to save user prefs: {e}");
                }
            }
            Err(e) => log::warn!("Failed to serialize user prefs: {e}"),
        }
    }
}

fn prefs_path() -> PathBuf {
    // Store next to the executable (or in cwd as fallback)
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(PREFS_FILE)))
        .unwrap_or_else(|| PathBuf::from(PREFS_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_camera_distance() {
        let prefs = UserPrefs::default();
        assert!((prefs.camera_distance - 3.0).abs() < 1e-6);
    }

    #[test]
    fn roundtrip_serialize() {
        let prefs = UserPrefs {
            camera_distance: 5.5,
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: UserPrefs = serde_json::from_str(&json).unwrap();
        assert!((loaded.camera_distance - 5.5).abs() < 1e-6);
    }
}
