use karate_core::{Error, assessment::*, evidence::*};

#[test]
fn budget_reservations_reject_negative_values_exhaustion_and_overflow() {
    for (available, reserved, requested) in [(-1, 0, 1), (1, -1, 1), (1, 0, 0), (1, 0, -1)] {
        assert_eq!(
            reserve_budget(available, reserved, requested),
            Err(Error::Validation)
        );
    }
    assert_eq!(reserve_budget(10, 3, 7), Ok(10));
    assert_eq!(reserve_budget(10, 3, 8), Err(Error::LimitReached));
    assert_eq!(
        reserve_budget(i64::MAX, i64::MAX, 1),
        Err(Error::LimitReached)
    );
}

#[test]
fn the_full_journey_can_pause_resume_cancel_or_complete_honestly() {
    let mut state = Lifecycle::default();
    for event in [
        Event::Queue,
        Event::Start,
        Event::Discover,
        Event::Test,
        Event::Validate,
    ] {
        state = state.apply(event).unwrap();
        if state.phase().active() {
            let paused = state.apply(Event::Pause).unwrap();
            assert_eq!(paused.phase(), Phase::Paused);
            assert_eq!(paused.apply(Event::Resume).unwrap(), state);
            let cancelled = paused.apply(Event::Cancel).unwrap();
            assert_eq!(cancelled.phase(), Phase::Cancelled);
            assert_eq!(cancelled.resume_phase(), None);
        }
    }
    assert_eq!(
        state.apply(Event::Complete).unwrap().phase(),
        Phase::Completed
    );
}

#[test]
fn persisted_lifecycle_rejects_inconsistent_pause_metadata() {
    let phases = [
        Phase::Draft,
        Phase::Queued,
        Phase::Preflight,
        Phase::Discovering,
        Phase::Testing,
        Phase::Validating,
        Phase::Paused,
        Phase::Completed,
        Phase::Cancelled,
        Phase::Failed,
    ];
    for phase in phases {
        assert_eq!(
            Lifecycle::restore(phase, None).is_ok(),
            phase != Phase::Paused
        );
        for resume in phases {
            assert_eq!(
                Lifecycle::restore(phase, Some(resume)).is_ok(),
                phase == Phase::Paused && resume.active()
            );
        }
    }
    let paused = Lifecycle::restore(Phase::Paused, Some(Phase::Testing)).unwrap();
    assert_eq!(paused.resume_phase(), Some(Phase::Testing));
    let failed = paused.apply(Event::Fail).unwrap();
    assert_eq!(failed.phase(), Phase::Failed);
    assert_eq!(failed.resume_phase(), None);
    assert_eq!(
        Lifecycle::default().apply(Event::Resume),
        Err(Error::Conflict)
    );
}

#[test]
fn every_state_event_pair_has_a_documented_outcome() {
    let phases = [
        Phase::Draft,
        Phase::Queued,
        Phase::Preflight,
        Phase::Discovering,
        Phase::Testing,
        Phase::Validating,
        Phase::Paused,
        Phase::Completed,
        Phase::Cancelled,
        Phase::Failed,
    ];
    let events = [
        Event::Queue,
        Event::Start,
        Event::Discover,
        Event::Test,
        Event::Validate,
        Event::Complete,
        Event::Pause,
        Event::Resume,
        Event::Cancel,
        Event::Fail,
    ];
    // Explicit table independent of implementation predicates. Each row lists allowed event indices.
    let allowed: [&[usize]; 10] = [
        &[0, 8],
        &[1, 8, 9],
        &[2, 6, 8, 9],
        &[3, 6, 8, 9],
        &[4, 6, 8, 9],
        &[5, 6, 8, 9],
        &[7, 8, 9],
        &[],
        &[],
        &[],
    ];
    for (i, phase) in phases.into_iter().enumerate() {
        let state =
            Lifecycle::restore(phase, (phase == Phase::Paused).then_some(Phase::Testing)).unwrap();
        for (j, event) in events.into_iter().enumerate() {
            assert_eq!(
                state.apply(event).is_ok(),
                allowed[i].contains(&j),
                "{phase:?} {event:?}"
            );
        }
    }
}

#[test]
fn observations_are_presence_only_and_never_contain_response_values() {
    assert_eq!(inspect_headers(&[], true).unwrap().len(), 4);
    assert_eq!(inspect_headers(&[], false).unwrap().len(), 3);
    let headers = [
        "Content-Security-Policy",
        "X-Content-Type-Options",
        "Strict-Transport-Security",
        "Referrer-Policy",
    ]
    .map(|key| (key.to_owned(), "untrusted-secret-value".to_owned()));
    assert!(inspect_headers(&headers, true).unwrap().is_empty());
    let observations = inspect_headers(
        &[
            ("Content-Security-Policy".into(), " \t".into()),
            ("authorization".into(), "secret".into()),
        ],
        true,
    )
    .unwrap();
    assert_eq!(observations.len(), 4);
    assert!(
        observations
            .iter()
            .all(|o| o.classification == Classification::HardeningObservation)
    );
    let json = serde_json::to_string(&observations).unwrap();
    assert!(!json.contains("secret"));
    assert!(!json.contains("verified"));
    assert_eq!(coverage(0, 0), None);
    assert_eq!(coverage(2, 1), None);
    assert_eq!(coverage(1, 2), Some(50.0));
    assert_eq!(coverage(0, 2), Some(0.0));
    assert_eq!(coverage(2, 2), Some(100.0));
}

#[test]
fn errors_are_closed_stable_and_free_of_caller_data() {
    for (error, code) in [
        (Error::Validation, "VALIDATION"),
        (Error::Unauthenticated, "UNAUTHENTICATED"),
        (Error::Forbidden, "FORBIDDEN"),
        (Error::NotFound, "NOT_FOUND"),
        (Error::Conflict, "CONFLICT"),
        (Error::OwnerRequired, "OWNER_REQUIRED"),
        (Error::OutOfScope, "OUT_OF_SCOPE"),
        (Error::Unverified, "UNVERIFIED"),
        (Error::LimitReached, "LIMIT_REACHED"),
        (Error::Unavailable, "UNAVAILABLE"),
    ] {
        assert_eq!(serde_json::to_value(error).unwrap(), code);
        assert!(!error.to_string().is_empty());
        assert_eq!(
            serde_json::from_value::<Error>(serde_json::json!(code)).unwrap(),
            error
        );
    }
}

#[test]
fn header_observation_work_is_bounded_without_echoing_oversized_input() {
    for headers in [
        vec![("x".into(), "y".into()); 129],
        vec![("x".repeat(257), "y".into())],
        vec![("x".into(), "y".repeat(8193))],
        vec![("x".into(), "y".repeat(8192)); 8],
    ] {
        assert_eq!(inspect_headers(&headers, true), Err(Error::LimitReached));
    }
    assert!(inspect_headers(&vec![("x".repeat(256), "y".into()); 128], true).is_ok());
    assert!(inspect_headers(&vec![("x".into(), "y".repeat(8191)); 8], true).is_ok());
}

#[test]
fn names_are_bounded_and_not_empty() {
    for name in ["", " ", "a\nb", "a\0b", &"x".repeat(121)] {
        assert_eq!(karate_core::validate_name(name), Err(Error::Validation));
    }
    for name in ["Karate", "São Paulo", &"x".repeat(120)] {
        assert_eq!(karate_core::validate_name(name), Ok(()));
    }
}
