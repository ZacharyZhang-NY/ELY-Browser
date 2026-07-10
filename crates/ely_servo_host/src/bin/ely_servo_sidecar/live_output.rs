use std::io::Write;

use super::live_protocol::{LiveOutcome, LiveSidecarError, validated_frame_byte_count};

pub(super) fn write_outcome(
    stdout: &mut impl Write,
    outcome: Result<LiveOutcome, LiveSidecarError>,
) -> Result<(), LiveSidecarError> {
    let mut outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => LiveOutcome::error(error.to_string()),
    };
    if let Some(frame) = outcome.frame.as_ref()
        && let Err(error) = validate_frame(frame.width(), frame.height(), frame.rgba_bytes().len())
    {
        let consumptions = std::mem::take(&mut outcome.response.permission_consumptions);
        outcome = LiveOutcome::error(error.to_string());
        outcome.response.permission_consumptions = consumptions;
    }

    serde_json::to_writer(&mut *stdout, &outcome.response)?;
    stdout.write_all(b"\n")?;
    if let Some(frame) = outcome.frame.as_ref() {
        stdout.write_all(frame.rgba_bytes())?;
    }
    stdout.flush()?;
    Ok(())
}

fn validate_frame(width: u32, height: u32, actual: usize) -> Result<(), LiveSidecarError> {
    let expected = validated_frame_byte_count(width, height)?;
    if expected != actual {
        return Err(LiveSidecarError::FrameByteCountMismatch { expected, actual });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mismatched_frame_becomes_header_only_error() -> Result<(), LiveSidecarError> {
        let profile_id = ely_domain::ProfileId::new();
        let frame = ely_servo_host::RenderedFrame::from_rgba_bytes(2, 2, vec![0; 4]);
        let snapshot = ely_servo_host::WebViewSnapshot::new(
            ely_domain::WebViewId::new(),
            ely_domain::TabId::new(),
            profile_id.clone(),
            ely_servo_host::WebViewState::Complete,
            None,
            None,
            ely_servo_host::WebViewSnapshotPending::new(false, false),
        );
        let report =
            super::super::live_protocol::LiveFrameReport::new(&snapshot, &frame, 1.0, true);
        let mut output = Vec::new();
        let outcome = LiveOutcome::frame(report, frame).with_permission_consumptions(vec![
            ely_servo_host::ConsumedPermission {
                profile_id: profile_id.clone(),
                origin: ely_domain::SiteOrigin::parse("https://example.com")?,
                feature: ely_domain::SitePermissionFeature::Camera,
                grant_revision: 7,
            },
        ]);

        write_outcome(&mut output, Ok(outcome))?;

        assert!(output.ends_with(b"\n"));
        let response: serde_json::Value = serde_json::from_slice(&output)?;
        assert!(response["error"].as_str().is_some());
        assert!(response["frame"].is_null());
        assert_eq!(response["permission_consumptions"][0]["profile_id"], profile_id.as_str());
        assert_eq!(response["permission_consumptions"][0]["grant_revision"], 7);
        Ok(())
    }
}
