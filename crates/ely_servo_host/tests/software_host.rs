#![cfg(feature = "servo-engine")]

use std::{error::Error, thread, time::Duration};

use ely_domain::{ProfileId, TabId, UrlText};
use ely_servo_host::{
    NavigationRequest, ServoHost, ServoHostError, ServoSurfaceSize, SoftwareServoHost, WebViewState,
};

#[test]
fn manages_real_servo_webview_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut host = SoftwareServoHost::new(ServoSurfaceSize::new(320, 240))?;
    let tab_id = TabId::new();
    let profile_id = ProfileId::new();

    let webview_id = host.create_webview(tab_id.clone(), profile_id.clone())?;
    let snapshot = host.snapshot(&webview_id)?;

    assert_eq!(snapshot.webview_id(), &webview_id);
    assert_eq!(snapshot.tab_id(), &tab_id);
    assert_eq!(snapshot.profile_id(), &profile_id);
    assert_eq!(snapshot.state(), &WebViewState::Created);

    let url = UrlText::parse(
        "data:text/html,%3Ctitle%3EELY%20Host%3C%2Ftitle%3E%3Cmain%3EReady%3C%2Fmain%3E",
    )?;

    host.navigate(NavigationRequest { webview_id: webview_id.clone(), tab_id, url })?;

    for _ in 0..1_000 {
        host.tick();
        if host.state(&webview_id)? == WebViewState::Complete {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }

    let snapshot = host.snapshot(&webview_id)?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert!(
        snapshot.url().is_some_and(|value| value.starts_with("data:text/html,")),
        "snapshot: {snapshot:?}"
    );

    assert!(matches!(
        SoftwareServoHost::new(ServoSurfaceSize::new(320, 240)),
        Err(ServoHostError::RuntimeAlreadyStarted)
    ));
    Ok(())
}
