use std::io::Write;

use super::live_protocol::{
    LiveOutcome, LiveResponse, LiveSidecarError, MAX_ERROR_BYTES, MAX_RESPONSE_HEADER_BYTES,
    MAX_TITLE_BYTES, validated_frame_byte_count,
};

const TITLE_TRUNCATION_SUFFIX: &str = "…";
const ERROR_TRUNCATION_SUFFIX: &str = "… [truncated]";

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
    bound_response_text(&mut outcome.response);

    let header = serde_json::to_vec(&outcome.response)?;
    let wire_bytes = header.len().saturating_add(1);
    if wire_bytes > MAX_RESPONSE_HEADER_BYTES {
        return Err(LiveSidecarError::ResponseHeaderTooLarge {
            bytes: wire_bytes,
            limit: MAX_RESPONSE_HEADER_BYTES,
        });
    }
    stdout.write_all(&header)?;
    stdout.write_all(b"\n")?;
    if let Some(frame) = outcome.frame.as_ref() {
        stdout.write_all(frame.rgba_bytes())?;
    }
    stdout.flush()?;
    Ok(())
}

fn bound_response_text(response: &mut LiveResponse) {
    response.error = response
        .error
        .take()
        .map(|error| truncate_utf8(error, MAX_ERROR_BYTES, ERROR_TRUNCATION_SUFFIX));
    if let Some(frame) = response.frame.as_mut() {
        frame.title = frame
            .title
            .take()
            .map(|title| truncate_utf8(title, MAX_TITLE_BYTES, TITLE_TRUNCATION_SUFFIX));
    }
}

fn truncate_utf8(mut value: String, limit: usize, suffix: &str) -> String {
    if value.len() <= limit {
        return value;
    }

    let mut end = limit - suffix.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push_str(suffix);
    value
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
    use super::super::live_protocol::{
        LiveFrameReport, LivePermissionConsumption, MAX_LOADED_URL_BYTES,
        MAX_PERMISSION_CONSUMPTIONS_PER_RESPONSE,
    };
    use super::*;

    #[test]
    fn overlong_utf8_title_stays_within_the_app_header_limit() -> Result<(), LiveSidecarError> {
        let frame = one_pixel_frame();
        let snapshot = snapshot(Some("https://example.com"), Some(&"界\"\n".repeat(40_000)));
        let report = LiveFrameReport::new(&snapshot, &frame, 1.0, true)?;
        let mut output = Vec::new();

        write_outcome(&mut output, Ok(LiveOutcome::frame(report, frame)))?;

        assert!(header_bytes(&output) <= MAX_RESPONSE_HEADER_BYTES);
        Ok(())
    }

    #[test]
    fn wire_text_preserves_exact_boundary_and_marks_ascii_overflow() {
        let exact = "a".repeat(MAX_TITLE_BYTES);
        assert_eq!(truncate_utf8(exact.clone(), MAX_TITLE_BYTES, TITLE_TRUNCATION_SUFFIX), exact);

        let overflow = truncate_utf8(
            "a".repeat(MAX_TITLE_BYTES + 1),
            MAX_TITLE_BYTES,
            TITLE_TRUNCATION_SUFFIX,
        );
        assert_eq!(overflow.len(), MAX_TITLE_BYTES);
        assert!(overflow.ends_with(TITLE_TRUNCATION_SUFFIX));
    }

    #[test]
    fn error_text_truncates_on_a_utf8_boundary() {
        let mut response = LiveOutcome::error("界".repeat(MAX_ERROR_BYTES)).response;

        bound_response_text(&mut response);

        let error = response.error.as_deref().unwrap_or_default();
        assert!(error.len() <= MAX_ERROR_BYTES);
        assert!(error.ends_with(ERROR_TRUNCATION_SUFFIX));
    }

    #[test]
    fn escape_heavy_maximum_frame_and_permission_batch_fit_header() -> Result<(), LiveSidecarError>
    {
        let frame = one_pixel_frame();
        let report = LiveFrameReport {
            loaded_url: Some("\u{2}".repeat(MAX_LOADED_URL_BYTES)),
            title: Some("\u{1}".repeat(MAX_TITLE_BYTES)),
            state: "complete",
            width: 1,
            height: 1,
            device_pixel_ratio: 1.0,
            css_viewport_width: 1,
            css_viewport_height: 1,
            rgba_byte_count: 4,
            pixels_changed: true,
            non_white_pixel_count: 0,
            content_pixel_count: 0,
            sample_hash: 0,
        };
        let mut outcome = LiveOutcome::frame(report, frame);
        let profile_id = ely_domain::ProfileId::new().as_str().to_string();
        outcome.response.permission_consumptions = (0..MAX_PERMISSION_CONSUMPTIONS_PER_RESPONSE)
            .map(|revision| LivePermissionConsumption {
                profile_id: profile_id.clone(),
                origin: "\u{3}".repeat(ely_domain::MAX_SITE_ORIGIN_BYTES),
                feature: "storage-persistence".to_string(),
                grant_revision: revision as u64,
            })
            .collect();
        let mut output = Vec::new();

        write_outcome(&mut output, Ok(outcome))?;

        assert!(header_bytes(&output) <= MAX_RESPONSE_HEADER_BYTES);
        Ok(())
    }

    #[test]
    fn loaded_url_preserves_exact_boundary_and_rejects_overflow() -> Result<(), LiveSidecarError> {
        let prefix = "https://example.com/";
        let exact_url = format!("{prefix}{}", "a".repeat(MAX_LOADED_URL_BYTES - prefix.len()));
        assert!(url::Url::parse(&exact_url).is_ok());
        let exact_snapshot = snapshot(Some(&exact_url), None);
        let exact = LiveFrameReport::new(&exact_snapshot, &one_pixel_frame(), 1.0, true)?;
        assert_eq!(exact.loaded_url.as_deref(), Some(exact_url.as_str()));

        let overflow_url =
            format!("{prefix}{}", "a".repeat(MAX_LOADED_URL_BYTES + 1 - prefix.len()));
        assert!(url::Url::parse(&overflow_url).is_ok());
        let overflow_snapshot = snapshot(Some(&overflow_url), None);
        assert!(matches!(
            LiveFrameReport::new(&overflow_snapshot, &one_pixel_frame(), 1.0, true),
            Err(LiveSidecarError::LoadedUrlTooLong { limit: MAX_LOADED_URL_BYTES })
        ));
        Ok(())
    }

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
        let report = LiveFrameReport::new(&snapshot, &frame, 1.0, true)?;
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

    fn one_pixel_frame() -> ely_servo_host::RenderedFrame {
        ely_servo_host::RenderedFrame::from_rgba_bytes(1, 1, vec![255; 4])
    }

    fn snapshot(url: Option<&str>, title: Option<&str>) -> ely_servo_host::WebViewSnapshot {
        ely_servo_host::WebViewSnapshot::new(
            ely_domain::WebViewId::new(),
            ely_domain::TabId::new(),
            ely_domain::ProfileId::new(),
            ely_servo_host::WebViewState::Complete,
            url.map(str::to_string),
            title.map(str::to_string),
            ely_servo_host::WebViewSnapshotPending::new(false, false),
        )
    }

    fn header_bytes(output: &[u8]) -> usize {
        output.iter().position(|byte| *byte == b'\n').map_or(0, |end| end + 1)
    }
}
