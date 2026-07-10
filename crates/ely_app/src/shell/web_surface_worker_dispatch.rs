use std::sync::mpsc;

use crate::services::servo_live::ServoLiveFrame;

use super::{
    LiveRuntimeClient, LiveRuntimeClientError, RequestGeneration, WorkerRequest, WorkerResponse,
};

pub(super) fn can_merge_consecutive_scroll(
    latest: &WorkerRequest,
    previous: &WorkerRequest,
) -> bool {
    let (
        WorkerRequest::Ensure { request: latest, .. },
        WorkerRequest::Ensure { request: previous, .. },
    ) = (latest, previous)
    else {
        return false;
    };
    is_scroll_only(latest)
        && is_scroll_only(previous)
        && latest.profile_id == previous.profile_id
        && latest.url == previous.url
        && latest.width == previous.width
        && latest.height == previous.height
        && latest.page_zoom_percent == previous.page_zoom_percent
        && latest.device_pixel_ratio == previous.device_pixel_ratio
}

pub(super) fn merge_consecutive_scroll(latest: &mut WorkerRequest, previous: &WorkerRequest) {
    if !can_merge_consecutive_scroll(latest, previous) {
        return;
    }
    let (
        WorkerRequest::Ensure { request: latest, .. },
        WorkerRequest::Ensure { request: previous, .. },
    ) = (latest, previous)
    else {
        return;
    };

    latest.scroll_delta_x = previous.scroll_delta_x.saturating_add(latest.scroll_delta_x);
    latest.scroll_delta_y = previous.scroll_delta_y.saturating_add(latest.scroll_delta_y);
    if latest.hover_x.is_none() && latest.hover_y.is_none() {
        latest.hover_x = previous.hover_x;
        latest.hover_y = previous.hover_y;
    }
}

fn is_scroll_only(request: &crate::services::servo_live::ServoLiveEnsureRequest) -> bool {
    (request.scroll_delta_x != 0 || request.scroll_delta_y != 0)
        && request.click_x.is_none()
        && request.click_y.is_none()
        && request.typed_text.is_none()
        && !request.site_permissions.iter().any(|permission| permission.state == "allow-once")
}

pub(super) fn forward_permission_consumptions(
    client: &mut dyn LiveRuntimeClient,
    response_tx: &mpsc::Sender<WorkerResponse>,
) {
    for consumption in client.take_permission_consumptions() {
        let _ = response_tx.send(WorkerResponse::PermissionConsumed(consumption));
    }
}

pub(super) fn dispatch_result(
    response_tx: &mpsc::Sender<WorkerResponse>,
    generation: RequestGeneration,
    tab_id: String,
    result: Result<Option<ServoLiveFrame>, LiveRuntimeClientError>,
) -> bool {
    match result {
        Ok(Some(frame)) => {
            let _ = response_tx.send(WorkerResponse::Frame { generation, tab_id, frame });
            false
        }
        Ok(None) => false,
        Err(error) => {
            let unavailable = error.is_runtime_unavailable();
            let message = error.to_string();
            let _ = response_tx.send(WorkerResponse::Failed { generation, tab_id, message });
            if unavailable {
                let _ = response_tx.send(WorkerResponse::RuntimeUnavailable);
                return true;
            }
            false
        }
    }
}

pub(super) fn request_has_ordered_input(request: &WorkerRequest) -> bool {
    let WorkerRequest::Ensure { request, .. } = request else {
        return false;
    };
    request.scroll_delta_x != 0
        || request.scroll_delta_y != 0
        || request.scroll_point_x.is_some()
        || request.scroll_point_y.is_some()
        || request.click_x.is_some()
        || request.click_y.is_some()
        || request.typed_text.is_some()
        || request.site_permissions.iter().any(|permission| permission.state == "allow-once")
}

pub(super) fn preserve_latest_hover(latest: &mut WorkerRequest, previous: &WorkerRequest) {
    let (
        WorkerRequest::Ensure { request: latest, .. },
        WorkerRequest::Ensure { request: previous, .. },
    ) = (latest, previous)
    else {
        return;
    };
    if latest.hover_x.is_none() && latest.hover_y.is_none() {
        latest.hover_x = previous.hover_x;
        latest.hover_y = previous.hover_y;
    }
}
