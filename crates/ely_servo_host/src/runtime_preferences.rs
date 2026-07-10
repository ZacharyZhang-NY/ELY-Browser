use servo::Preferences;

pub(super) fn ely_servo_preferences() -> Preferences {
    // Servo's current permission request omits the requesting principal. WebRTC,
    // Clipboard, and Bluetooth also contain paths that bypass the embedder broker.
    // Keep every affected API explicitly gated until those contracts are complete.
    Preferences {
        dom_async_clipboard_enabled: false,
        dom_bluetooth_enabled: false,
        dom_geolocation_enabled: false,
        dom_notification_enabled: false,
        dom_permissions_enabled: false,
        dom_storage_manager_api_enabled: false,
        dom_wakelock_enabled: false,
        dom_webrtc_enabled: false,
        dom_intersection_observer_enabled: true,
        layout_grid_enabled: true,
        layout_variable_fonts_enabled: true,
        ..Preferences::default()
    }
}

#[cfg(test)]
mod tests {
    use super::ely_servo_preferences;

    #[test]
    fn unsafe_permission_surfaces_stay_gated() {
        let preferences = ely_servo_preferences();

        assert!(!preferences.dom_async_clipboard_enabled);
        assert!(!preferences.dom_bluetooth_enabled);
        assert!(!preferences.dom_geolocation_enabled);
        assert!(!preferences.dom_notification_enabled);
        assert!(!preferences.dom_permissions_enabled);
        assert!(!preferences.dom_storage_manager_api_enabled);
        assert!(!preferences.dom_wakelock_enabled);
        assert!(!preferences.dom_webrtc_enabled);
    }

    #[test]
    fn required_rendering_features_stay_enabled() {
        let preferences = ely_servo_preferences();

        assert!(preferences.dom_intersection_observer_enabled);
        assert!(preferences.layout_grid_enabled);
        assert!(preferences.layout_variable_fonts_enabled);
    }
}
