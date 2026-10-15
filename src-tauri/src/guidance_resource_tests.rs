use crate::guidance_resources::GuidanceResources;
use tauri::http::{Request, StatusCode};

#[test]
fn imported_html_is_an_independent_readable_copy_and_never_a_video_protocol_page() {
    let dir = std::env::temp_dir().join(format!("sit-break-html-copy-{}", fastrand::u64(..)));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("动作.html");
    let original = "<!doctype html><style>body{color:green}</style><button onclick='this.textContent=2'>一步</button>";
    std::fs::write(&source, original).unwrap();
    let resources = GuidanceResources::open(dir.join("imports")).unwrap();
    let guide = resources.import_html(&source).unwrap();
    assert_eq!(serde_json::to_value(&guide).unwrap()["kind"], "html");
    std::fs::write(&source, "<h1>更新后的内容</h1>").unwrap();
    let next = resources.import_html(&source).unwrap();
    assert_ne!(guide, next);
    std::fs::remove_file(source).unwrap();
    drop(resources);
    let reopened = GuidanceResources::open(dir.join("imports")).unwrap();
    let read = reopened.read(&guide).unwrap();
    assert_eq!(read.mime, "text/html; charset=utf-8");
    assert_eq!(read.bytes, original.as_bytes());
    assert_eq!(
        reopened.read(&next).unwrap().bytes,
        "<h1>更新后的内容</h1>".as_bytes()
    );
    let crate::GuidanceSource::Local { id } = &guide.source else {
        panic!()
    };
    assert_eq!(
        reopened
            .video_response(
                Request::get(format!("guidance://localhost/{id}"))
                    .body(Vec::new())
                    .unwrap()
            )
            .status(),
        StatusCode::NOT_FOUND
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn imported_video_survives_source_deletion_and_resource_store_reopening() {
    let dir = std::env::temp_dir().join(format!("sit-break-resources-{}", fastrand::u64(..)));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("自制视频.mp4");
    let expected = include_bytes!("../../src/assets/guidance/squat-demo.mp4");
    std::fs::write(&source, expected).unwrap();
    let resources = GuidanceResources::open(dir.join("imports")).unwrap();
    let guidance = resources.import_video(&source).unwrap();
    let id = match &guidance.source {
        crate::GuidanceSource::Local { id } => id,
        _ => panic!("import must return an opaque local reference"),
    };
    assert!(!id.contains("自制"));
    std::fs::remove_file(&source).unwrap();
    drop(resources);
    let reopened = GuidanceResources::open(dir.join("imports")).unwrap();
    let response = reopened.video_response(
        Request::get(format!("guidance://localhost/{id}"))
            .body(Vec::new())
            .unwrap(),
    );
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "video/mp4");
    assert_eq!(response.body(), expected);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn replacing_and_removing_guidance_keeps_current_and_valid_restored_copies_until_unreferenced() {
    use crate::{
        timer::{Action, LocalTime, TimerApp, TimerTime},
        ReminderEntry, Settings,
    };
    use std::time::Duration;
    let dir =
        std::env::temp_dir().join(format!("sit-break-resource-lifetime-{}", fastrand::u64(..)));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("same-name.mp4");
    let original = include_bytes!("../../src/assets/guidance/squat-demo.mp4");
    std::fs::write(&source, original).unwrap();
    let resources = GuidanceResources::open(dir.join("imports")).unwrap();
    let old = resources.import_video(&source).unwrap();
    let mut updated = original.to_vec();
    updated.extend_from_slice(b"updated source");
    std::fs::write(&source, &updated).unwrap();
    let replacement = resources.import_video(&source).unwrap();
    assert_ne!(old, replacement);
    assert_eq!(resources.read(&old).unwrap().bytes, original);
    assert_eq!(resources.read(&replacement).unwrap().bytes, updated);
    let now = |date: &str, seconds: u32, monotonic: u64| TimerTime {
        local: Some(LocalTime {
            date: date.into(),
            seconds_of_day: seconds,
        }),
        monotonic: Duration::from_secs(monotonic),
    };
    let mut settings = Settings {
        use_schedule: true,
        idle_reset: false,
        ..Settings::default()
    };
    settings.schedules = serde_json::from_value(serde_json::json!([{
        "id":"daily", "name":"日程", "enabled":true,
        "repeat":{"kind":"weekly", "weekdays":[1,2,3,4,5,6,7]},
        "periods":[{"id":"morning", "start":"09:00", "end":"12:00", "work_minutes":1, "break_minutes":5,
          "overrides":[{"round":1, "guidance":old}]}]
    }])).unwrap();
    let mut app = TimerApp::new_at(
        settings,
        fastrand::Rng::with_seed(1),
        now("2026-10-15", 32400, 0),
    );
    app.dispatch(
        Action::Tick {
            elapsed_secs: 60,
            idle_secs: 0,
        },
        now("2026-10-15", 32460, 60),
    );
    app.dispatch(Action::StartBreak, now("2026-10-15", 32460, 60));
    let mut changed = app.settings().clone();
    changed.schedules[0].periods[0].overrides[0].guidance = None;
    changed.tips = vec![ReminderEntry {
        id: "new-entry".into(),
        text: String::new(),
        guidance: Some(replacement.clone()),
    }];
    app.dispatch(Action::SaveSettings(changed), now("2026-10-15", 32460, 60));
    resources.retain_for(app.settings(), &app.info()).unwrap();
    assert_eq!(resources.read(&old).unwrap().bytes, original);
    assert_eq!(resources.read(&replacement).unwrap().bytes, updated);
    let mut changed = app.settings().clone();
    changed.tips.clear();
    app.dispatch(Action::SaveSettings(changed), now("2026-10-15", 32460, 60));
    app.settings().save_to(&dir.join("settings.json")).unwrap();
    app.save_progress_to(&dir.join("progress.json"), now("2026-10-15", 32460, 60))
        .unwrap();
    let reopened = TimerApp::restore_from(
        Settings::load_from(&dir.join("settings.json")),
        &dir.join("progress.json"),
        fastrand::Rng::with_seed(1),
        now("2026-10-15", 32520, 0),
    );
    assert_eq!(reopened.info().guidance, Some(old.clone()));
    resources
        .retain_for(reopened.settings(), &reopened.info())
        .unwrap();
    assert_eq!(resources.read(&old).unwrap().bytes, original);
    assert!(resources.read(&replacement).is_err());
    let next_day = TimerApp::restore_from(
        Settings::load_from(&dir.join("settings.json")),
        &dir.join("progress.json"),
        fastrand::Rng::with_seed(1),
        now("2026-10-16", 32400, 0),
    );
    resources
        .retain_for(next_day.settings(), &next_day.info())
        .unwrap();
    assert!(resources.read(&old).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn imported_video_supports_native_range_and_head_requests_without_a_path_read_escape() {
    let dir = std::env::temp_dir().join(format!("sit-break-resource-stream-{}", fastrand::u64(..)));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("video.mp4");
    let content = include_bytes!("../../src/assets/guidance/squat-demo.mp4");
    std::fs::write(&source, content).unwrap();
    let resources = GuidanceResources::open(dir.join("imports")).unwrap();
    let guidance = resources.import_video(&source).unwrap();
    let crate::GuidanceSource::Local { id } = &guidance.source else {
        panic!()
    };
    let url = format!("http://guidance.localhost/{id}");
    for (range, expected) in [
        ("bytes=4-7", b"ftyp".as_slice()),
        ("bytes=4-", &content[4..]),
        ("bytes=-4", &content[content.len() - 4..]),
    ] {
        let response = resources.video_response(
            Request::get(&url)
                .header("range", range)
                .body(Vec::new())
                .unwrap(),
        );
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT, "{range}");
        assert_eq!(response.headers()["accept-ranges"], "bytes");
        assert_eq!(response.body(), expected);
    }
    for range in [
        "bytes=bad",
        "bytes=999999999-",
        "bytes=8-4",
        "bytes=-0",
        "items=0-4",
        "bytes=0-1,4-5",
    ] {
        let response = resources.video_response(
            Request::get(&url)
                .header("range", range)
                .body(Vec::new())
                .unwrap(),
        );
        assert_eq!(
            response.status(),
            StatusCode::RANGE_NOT_SATISFIABLE,
            "{range}"
        );
        assert_eq!(
            response.headers()["content-range"],
            format!("bytes */{}", content.len())
        );
    }
    for range in [None, Some("bytes=4-7")] {
        let mut request = Request::head(&url);
        if let Some(range) = range {
            request = request.header("range", range);
        }
        let response = resources.video_response(request.body(Vec::new()).unwrap());
        assert!(response.body().is_empty());
        assert_eq!(
            response.headers()["content-length"],
            if range.is_some() {
                "4".into()
            } else {
                content.len().to_string()
            }
        );
    }
    assert_eq!(
        resources
            .video_response(Request::post(&url).body(Vec::new()).unwrap())
            .status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    for path in [
        "../video.mp4",
        "%2e%2e/video.mp4",
        "local-invalid",
        "local-00000000000000000000000000000000.mp4",
        "local-00000000000000000000000000000000",
    ] {
        assert_eq!(
            resources
                .video_response(
                    Request::get(format!("guidance://localhost/{path}"))
                        .body(Vec::new())
                        .unwrap()
                )
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_imports_leave_the_old_selection_readable_and_unreadable_copies_cannot_be_saved() {
    let dir = std::env::temp_dir().join(format!("sit-break-resource-errors-{}", fastrand::u64(..)));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("video.mp4");
    let content = include_bytes!("../../src/assets/guidance/squat-demo.mp4");
    std::fs::write(&source, content).unwrap();
    let resources = GuidanceResources::open(dir.join("imports")).unwrap();
    let old = resources.import_video(&source).unwrap();
    let settings = crate::Settings {
        tips: vec![crate::ReminderEntry {
            id: "kept-entry".into(),
            text: String::new(),
            guidance: Some(old.clone()),
        }],
        ..crate::Settings::default()
    };
    settings.save_to(&dir.join("settings.json")).unwrap();
    for bytes in [b"<html><script>bad()</script></html>".as_slice(), b"short"] {
        std::fs::write(&source, bytes).unwrap();
        assert!(resources.import_video(&source).is_err());
        assert_eq!(resources.read(&old).unwrap().bytes, content);
        assert_eq!(
            crate::Settings::load_from(&dir.join("settings.json")).tips[0].guidance,
            Some(old.clone())
        );
    }
    std::fs::remove_file(&source).unwrap();
    assert!(resources.import_video(&source).is_err());
    resources.validate_settings(&settings).unwrap();
    std::fs::remove_dir_all(dir.join("imports")).unwrap();
    assert!(resources.validate_settings(&settings).is_err());
    assert_eq!(
        crate::Settings::load_from(&dir.join("settings.json")).tips[0].guidance,
        Some(old)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
