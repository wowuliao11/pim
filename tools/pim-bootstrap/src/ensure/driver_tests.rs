//! Driver test matrix.
//!
//! Two test styles:
//! - **Fake op**: in-memory [`EnsureOp`] impl with pre-programmed responses.
//!   Exercises driver control flow (sequencing, short-circuit, Plan vs Apply)
//!   without any HTTP.
//! - **Wiremock op**: an [`EnsureOp`] whose `observe` calls a real
//!   `ZitadelClient` pointed at a `wiremock::MockServer`. Proves the
//!   `DynEnsureOp::step` wiring end-to-end against real HTTP machinery.

use async_trait::async_trait;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zitadel_rest_client::{AdminCredential, ZitadelClient, ZitadelError};

use super::op::{EnsureError, EnsureOp};
use super::report::EnsureOutcome;
use super::state::{EnsureState, Flags, Mode};
use super::{run_pipeline, DynEnsureOp};

struct FakeOp {
    name: &'static str,
    observed: Option<()>,
    classified: EnsureState,
    act_result: Option<Result<EnsureOutcome, EnsureError>>,
}

#[async_trait]
impl EnsureOp for FakeOp {
    type Desired = ();
    type Observed = ();

    fn name(&self) -> &str {
        self.name
    }

    fn desired(&self) -> &Self::Desired {
        &()
    }

    async fn observe(&self, _client: &ZitadelClient) -> Result<Option<Self::Observed>, ZitadelError> {
        Ok(self.observed)
    }

    fn classify(&self, _desired: &Self::Desired, _observed: Option<&Self::Observed>) -> EnsureState {
        self.classified
    }

    async fn act(
        &self,
        _state: EnsureState,
        _flags: Flags,
        _observed: Option<Self::Observed>,
        _client: &ZitadelClient,
    ) -> Result<EnsureOutcome, EnsureError> {
        match &self.act_result {
            Some(Ok(o)) => Ok(o.clone()),
            Some(Err(EnsureError::Fatal { op, reason })) => Err(EnsureError::Fatal {
                op: op.clone(),
                reason: reason.clone(),
            }),
            Some(Err(EnsureError::Transport(_))) | None => Err(EnsureError::Fatal {
                op: self.name.into(),
                reason: "act_result not set".into(),
            }),
        }
    }
}

fn test_client() -> ZitadelClient {
    ZitadelClient::new("http://localhost:1/", AdminCredential::Pat("test".into()))
        .expect("client construction must succeed")
}

fn boxed(op: FakeOp) -> Box<dyn DynEnsureOp> {
    Box::new(op)
}

#[tokio::test]
async fn plan_mode_skips_act_and_records_state() {
    let ops: Vec<Box<dyn DynEnsureOp>> = vec![boxed(FakeOp {
        name: "project",
        observed: None,
        classified: EnsureState::Missing,
        act_result: None,
    })];
    let result = run_pipeline(&ops, Mode::Plan, Flags::default(), &test_client()).await;
    assert!(result.is_ok());
    assert_eq!(result.report.rows.len(), 1);
    assert_eq!(result.report.rows[0].state, EnsureState::Missing);
    assert!(result.report.rows[0].outcome.is_none(), "Plan mode must not call act");
}

#[tokio::test]
async fn apply_mode_runs_act_and_captures_outcome() {
    let ops: Vec<Box<dyn DynEnsureOp>> = vec![boxed(FakeOp {
        name: "project",
        observed: None,
        classified: EnsureState::Missing,
        act_result: Some(Ok(EnsureOutcome::Created { id: "p-1".into() })),
    })];
    let result = run_pipeline(&ops, Mode::Apply, Flags::default(), &test_client()).await;
    assert!(result.is_ok());
    assert_eq!(
        result.report.rows[0].outcome,
        Some(EnsureOutcome::Created { id: "p-1".into() })
    );
}

#[tokio::test]
async fn drift_without_sync_flags_report_drift_detected() {
    let ops: Vec<Box<dyn DynEnsureOp>> = vec![boxed(FakeOp {
        name: "api-app",
        observed: Some(()),
        classified: EnsureState::Drift,
        act_result: Some(Ok(EnsureOutcome::Blocked {
            reason: "drift without --sync".into(),
        })),
    })];
    let result = run_pipeline(&ops, Mode::Apply, Flags::default(), &test_client()).await;
    assert!(result.is_ok());
    assert!(
        result.report.drift_detected,
        "driver must flag drift_detected so `diff` can return a non-zero exit code",
    );
}

#[tokio::test]
async fn drift_with_sync_updates_and_does_not_flag_drift() {
    let ops: Vec<Box<dyn DynEnsureOp>> = vec![boxed(FakeOp {
        name: "api-app",
        observed: Some(()),
        classified: EnsureState::Drift,
        act_result: Some(Ok(EnsureOutcome::Updated {
            id: "app-1".into(),
            fields: vec!["name".into()],
        })),
    })];
    let result = run_pipeline(
        &ops,
        Mode::Apply,
        Flags {
            sync: true,
            rotate_keys: false,
        },
        &test_client(),
    )
    .await;
    assert!(result.is_ok());
    assert!(
        !result.report.drift_detected,
        "drift that was reconciled must not leave drift_detected=true",
    );
}

#[tokio::test]
async fn fatal_error_short_circuits_pipeline() {
    let ops: Vec<Box<dyn DynEnsureOp>> = vec![
        boxed(FakeOp {
            name: "project",
            observed: None,
            classified: EnsureState::Missing,
            act_result: Some(Ok(EnsureOutcome::Created { id: "p-1".into() })),
        }),
        boxed(FakeOp {
            name: "api-app",
            observed: None,
            classified: EnsureState::Conflict,
            act_result: Some(Err(EnsureError::Fatal {
                op: "api-app".into(),
                reason: "name collides".into(),
            })),
        }),
        boxed(FakeOp {
            name: "roles",
            observed: None,
            classified: EnsureState::Missing,
            act_result: Some(Ok(EnsureOutcome::Created { id: "r-1".into() })),
        }),
    ];
    let result = run_pipeline(&ops, Mode::Apply, Flags::default(), &test_client()).await;

    assert!(result.error.is_some(), "fatal must surface");
    assert_eq!(result.report.rows.len(), 1, "subsequent ops must not run after fatal",);
    assert_eq!(result.report.rows[0].op_name, "project");
    match result.error.unwrap() {
        EnsureError::Fatal { op, reason } => {
            assert_eq!(op, "api-app");
            assert!(reason.contains("collide"));
        }
        EnsureError::Transport(e) => panic!("expected Fatal, got Transport: {e}"),
    }
}

#[tokio::test]
async fn match_state_is_no_change_in_apply_mode() {
    let ops: Vec<Box<dyn DynEnsureOp>> = vec![boxed(FakeOp {
        name: "project",
        observed: Some(()),
        classified: EnsureState::Match,
        act_result: Some(Ok(EnsureOutcome::NoChange { id: "p-1".into() })),
    })];
    let result = run_pipeline(&ops, Mode::Apply, Flags::default(), &test_client()).await;
    assert!(result.is_ok());
    assert!(!result.report.drift_detected);
    assert_eq!(
        result.report.rows[0].outcome,
        Some(EnsureOutcome::NoChange { id: "p-1".into() })
    );
}

struct WiremockProbeOp {
    name: &'static str,
    path: String,
}

#[async_trait]
impl EnsureOp for WiremockProbeOp {
    type Desired = ();
    type Observed = serde_json::Value;

    fn name(&self) -> &str {
        self.name
    }

    fn desired(&self) -> &Self::Desired {
        &()
    }

    async fn observe(&self, client: &ZitadelClient) -> Result<Option<Self::Observed>, ZitadelError> {
        use reqwest::Method;
        match client
            .send_json::<(), serde_json::Value>(Method::GET, &self.path, None)
            .await
        {
            Ok(v) => Ok(Some(v)),
            Err(ZitadelError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn classify(&self, _desired: &Self::Desired, observed: Option<&Self::Observed>) -> EnsureState {
        if observed.is_some() {
            EnsureState::Match
        } else {
            EnsureState::Missing
        }
    }

    async fn act(
        &self,
        _state: EnsureState,
        _flags: Flags,
        _observed: Option<Self::Observed>,
        _client: &ZitadelClient,
    ) -> Result<EnsureOutcome, EnsureError> {
        Ok(EnsureOutcome::NoChange { id: "mock".into() })
    }
}

#[tokio::test]
async fn wiremock_e2e_observe_classify_match() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/management/v1/projects/p-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "p-123"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = ZitadelClient::new(server.uri(), AdminCredential::Pat("dev-pat".into())).unwrap();

    let ops: Vec<Box<dyn DynEnsureOp>> = vec![Box::new(WiremockProbeOp {
        name: "probe",
        path: "/management/v1/projects/p-123".into(),
    })];

    let result = run_pipeline(&ops, Mode::Plan, Flags::default(), &client).await;
    assert!(result.is_ok());
    assert_eq!(result.report.rows[0].state, EnsureState::Match);
}

#[tokio::test]
async fn wiremock_e2e_observe_404_classifies_missing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/management/v1/projects/nope"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let client = ZitadelClient::new(server.uri(), AdminCredential::Pat("dev-pat".into())).unwrap();

    let ops: Vec<Box<dyn DynEnsureOp>> = vec![Box::new(WiremockProbeOp {
        name: "probe",
        path: "/management/v1/projects/nope".into(),
    })];

    let result = run_pipeline(&ops, Mode::Plan, Flags::default(), &client).await;
    assert!(result.is_ok());
    assert_eq!(result.report.rows[0].state, EnsureState::Missing);
}
