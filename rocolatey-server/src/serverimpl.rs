use std::sync::OnceLock;
use chrono::Utc;
use uuid::Uuid;
use warp::http::StatusCode;
use warp::Filter;

use chrono::Local;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::authorized_keys_runtime::AuthorizedKeysRuntime;

use rocolatey_lib::roco::{
    get_choco_sources,
    local::{
        get_dependency_tree_nodes, get_local_bad_packages, get_local_packages,
    },
    remote::{find_packages, get_outdated_packages, get_outdated_packages_text},
};
use rocolatey_lib::server::authorization;
use rocolatey_lib::server::audit;
use rocolatey_lib::server::JobState;
use rocolatey_lib::server::JobStatus;
use rocolatey_lib::server::RocoServerChocoCommandRequest;
use rocolatey_lib::server::RocoServerDenyCode;
use rocolatey_lib::server::RocoServerDenyResponse;
use rocolatey_lib::server::RocoServerDependencyTreeResponse;
use rocolatey_lib::server::RocoServerFeedsResponse;
use rocolatey_lib::server::RocoServerListRequest;
use rocolatey_lib::server::RocoServerOutdatedRequest;
use rocolatey_lib::server::RocoServerOutdatedResponse;
use rocolatey_lib::server::RocoServerPackagesResponse;
use rocolatey_lib::server::RocoServerSearchRequest;
use rocolatey_lib::server::RocoServerTrustMode;
use rocolatey_lib::server::RocoServerTrustRenewResponse;
use rocolatey_lib::server::RocoServerTrustStateResponse;
use rocolatey_lib::server::ROCO_SERVER_SCHEMA_VERSION;

pub type JobStore = Arc<RwLock<HashMap<Uuid, JobState>>>;

static AUTH_RUNTIME: OnceLock<AuthorizedKeysRuntime> = OnceLock::new();

pub(crate) fn install_authorization_runtime(runtime: AuthorizedKeysRuntime) {
    let _ = AUTH_RUNTIME.set(runtime);
}

fn get_authorization_runtime() -> AuthorizedKeysRuntime {
    AUTH_RUNTIME
        .get()
        .cloned()
        .unwrap_or_else(|| {
            let default_path = rocolatey_lib::server::default_server_authorized_keys_path();
            AuthorizedKeysRuntime::from_file(&default_path)
                .unwrap_or_else(|_| AuthorizedKeysRuntime::empty_unhealthy(&default_path))
        })
}

fn new_request_id() -> String {
    Uuid::new_v4().to_string()
}

fn with_request_id() -> impl Filter<Extract = (String,), Error = std::convert::Infallible> + Clone {
    warp::any().map(new_request_id)
}

fn deny_as_rejection(
    request_id: &str,
    code: RocoServerDenyCode,
    message: String,
    enrollment_hint: Option<String>,
) -> warp::Rejection {
    deny_as_rejection_with_status(
        request_id,
        code,
        message,
        enrollment_hint,
        StatusCode::FORBIDDEN,
    )
}

fn deny_as_rejection_with_status(
    request_id: &str,
    code: RocoServerDenyCode,
    message: String,
    enrollment_hint: Option<String>,
    status: StatusCode,
) -> warp::Rejection {
    let response = authorization::build_deny_response(
        request_id.to_string(),
        code,
        message,
        None,
        enrollment_hint,
    );
    warp::reject::custom(PhaseAuthError { response, status })
}


/// Check authorization for a protected route.
/// Returns Ok(()) if authorized, Err(warp::Rejection) if unauthorized.
///
/// Phase 5 behavior is fail-closed:
/// - trust identity time validity is enforced,
/// - watcher health is observable and alerts on fault,
/// - empty enrollment mode denies unless explicit emergency override is enabled.
fn check_protected_route_auth(
    request_id: &str,
    client_fingerprint_hint: Option<String>,
) -> Result<(), warp::Rejection> {
    if cfg!(test) {
        return Ok(());
    }

    let auth_runtime = get_authorization_runtime();

    let server_cert = match std::fs::read(rocolatey_lib::server::default_server_cert_path()) {
        Ok(v) => v,
        Err(err) => {
            let response = authorization::build_deny_response(
                request_id.to_string(),
                RocoServerDenyCode::ServerTrustMismatch,
                format!("Server trust material read failed: {}", err),
                None,
                None,
            );
            return Err(warp::reject::custom(PhaseAuthError {
                response,
                status: StatusCode::FORBIDDEN,
            }));
        }
    };

    match authorization::evaluate_certificate_time_now(&server_cert) {
        Ok(authorization::CertificateTimeStatus::Valid) => {}
        Ok(authorization::CertificateTimeStatus::Expired) => {
            let response = authorization::build_deny_response(
                request_id.to_string(),
                RocoServerDenyCode::ClientCertificateExpired,
                "Trust identity has expired. Fail-closed policy denied the request.".to_string(),
                None,
                Some("Rotate identity material and restart service before retrying.".to_string()),
            );
            audit::emit_audit_event(
                &audit::server_audit_log_path(),
                &audit::AuditEvent::new(
                    audit::AuditEventKind::ExpiryFailure,
                    "DENY: server certificate expired",
                )
                .with_request_id(request_id),
            );
            return Err(warp::reject::custom(PhaseAuthError {
                response,
                status: StatusCode::FORBIDDEN,
            }));
        }
        Ok(authorization::CertificateTimeStatus::TimeSkewExceeded) => {
            let response = authorization::build_deny_response(
                request_id.to_string(),
                RocoServerDenyCode::TimeSkewExceeded,
                "Clock skew exceeded 8 minutes. Fail-closed policy denied the request.".to_string(),
                None,
                Some("Check host time synchronization and retry after correcting system clock.".to_string()),
            );
            audit::emit_audit_event(
                &audit::server_audit_log_path(),
                &audit::AuditEvent::new(
                    audit::AuditEventKind::DenyDecision,
                    "DENY: clock skew exceeded 8 minutes",
                )
                .with_request_id(request_id),
            );
            return Err(warp::reject::custom(PhaseAuthError {
                response,
                status: StatusCode::FORBIDDEN,
            }));
        }
        Err(err) => {
            let response = authorization::build_deny_response(
                request_id.to_string(),
                RocoServerDenyCode::ClientCertificateInvalid,
                format!("Certificate validation failed: {}", err),
                None,
                None,
            );
            audit::emit_audit_event(
                &audit::server_audit_log_path(),
                &audit::AuditEvent::new(
                    audit::AuditEventKind::DenyDecision,
                    format!("DENY: certificate validation failed: {}", err),
                )
                .with_request_id(request_id),
            );
            return Err(warp::reject::custom(PhaseAuthError {
                response,
                status: StatusCode::FORBIDDEN,
            }));
        }
    }

    if !auth_runtime.watcher_healthy() {
        eprintln!(
            "[HIGH] authorized_keys watcher unhealthy. Operating with last known good key set. request_id={}",
            request_id
        );
    }

    if auth_runtime.authorized_key_count() == 0 {
        if let Some(client_fingerprint) = client_fingerprint_hint {
            let override_path = rocolatey_lib::server::ServerTlsConfig::default().emergency_override_path();
            match authorization::consume_emergency_override(
                &override_path,
                &client_fingerprint,
                request_id,
                Utc::now(),
            ) {
                Ok(true) => {
                    eprintln!(
                        "[HIGH][AUDIT] emergency trust override accepted. request_id={} fingerprint={}",
                        request_id,
                        authorization::short_fingerprint(&client_fingerprint)
                    );
                    audit::emit_audit_event(
                        &audit::server_audit_log_path(),
                        &audit::AuditEvent::new(
                            audit::AuditEventKind::EmergencyOverride,
                            format!(
                                "EMERGENCY TRUST OVERRIDE accepted for fingerprint {}",
                                authorization::short_fingerprint(&client_fingerprint)
                            ),
                        )
                        .with_request_id(request_id)
                        .with_fingerprint(&client_fingerprint),
                    );
                    return Ok(());
                }
                Ok(false) => {}
                Err(err) => {
                    let response = authorization::build_deny_response(
                        request_id.to_string(),
                        RocoServerDenyCode::InternalAuthorizationError,
                        format!("Emergency override state invalid: {}", err),
                        Some(&client_fingerprint),
                        None,
                    );
                    audit::emit_audit_event(
                        &audit::server_audit_log_path(),
                        &audit::AuditEvent::new(
                            audit::AuditEventKind::EmergencyOverride,
                            format!("EMERGENCY TRUST OVERRIDE failed: {}", err),
                        )
                        .with_request_id(request_id)
                        .with_fingerprint(&client_fingerprint)
                        .with_deny_code("internal_authorization_error"),
                    );
                    return Err(warp::reject::custom(PhaseAuthError {
                        response,
                        status: StatusCode::FORBIDDEN,
                    }));
                }
            }
        }

        if authorization::emergency_trust_override_enabled() {
            eprintln!(
                "[HIGH][AUDIT] emergency trust override configured but fingerprint hint missing. request_id={} env_var={}",
                request_id,
                authorization::EMERGENCY_TRUST_OVERRIDE_ENV
            );
        }

        let response = authorization::build_deny_response(
            request_id.to_string(),
            RocoServerDenyCode::NotEnrolledClient,
            "Server is in empty enrollment mode. No clients are authorized yet.".to_string(),
            None,
            Some("An administrator must add authorized client fingerprints. Emergency override requires an explicit fingerprint-scoped environment override and is limited to one audited use within a short TTL window.".to_string()),
        );
        audit::emit_audit_event(
            &audit::server_audit_log_path(),
            &audit::AuditEvent::new(
                audit::AuditEventKind::DenyDecision,
                "DENY: server in empty enrollment mode",
            )
            .with_request_id(request_id),
        );
        return Err(warp::reject::custom(PhaseAuthError {
            response,
            status: StatusCode::FORBIDDEN,
        }));
    }

    Ok(())
}

/// Custom rejection type for Phase 4 authorization errors
#[derive(Debug)]
struct PhaseAuthError {
    response: RocoServerDenyResponse,
    status: StatusCode,
}

impl warp::reject::Reject for PhaseAuthError {}



fn build_trust_state_response(request_id: String) -> RocoServerTrustStateResponse {
    let auth_runtime = get_authorization_runtime();
    let server_key_present = rocolatey_lib::server::default_server_key_path().exists();
    let server_cert_present = rocolatey_lib::server::default_server_cert_path().exists();
    let authorized_key_count = auth_runtime.authorized_key_count();
    let trust_mode = if authorized_key_count == 0 {
        RocoServerTrustMode::EmptyEnrollment
    } else {
        RocoServerTrustMode::Enforced
    };

    RocoServerTrustStateResponse {
        schema_version: ROCO_SERVER_SCHEMA_VERSION,
        request_id,
        trust_mode,
        server_key_present,
        server_cert_present,
        authorized_key_count,
        authorized_keys_watcher_healthy: auth_runtime.watcher_healthy(),
        emergency_override_active: authorization::emergency_trust_override_enabled(),
    }
}

pub(crate) fn create_warp_filter(
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let api_base = warp::path("rocolatey");
    // job store for background choco commands
    let store: JobStore = Arc::new(RwLock::new(HashMap::new()));
    let store_filter = warp::any().map(move || store.clone());

    let local_json = api_base
        .and(warp::path!("local" / "json"))
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|client_fingerprint_hint: Option<String>, request_id: String| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            match get_local_packages("all") {
                Ok((data, total_count)) => Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&RocoServerPackagesResponse {
                        schema_version: ROCO_SERVER_SCHEMA_VERSION,
                        total_count: Some(total_count),
                        data,
                    }),
                    StatusCode::OK,
                )),
                Err(err) => Err(deny_as_rejection(
                    &request_id,
                    RocoServerDenyCode::InternalAuthorizationError,
                    format!("Error: {}", err),
                    None,
                )),
            }
        });

    let local_json_post = api_base
        .and(warp::path!("local" / "json"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|req: RocoServerListRequest, client_fingerprint_hint: Option<String>, request_id: String| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            match get_local_packages(&req.filter) {
                Ok((data, total_count)) => Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&RocoServerPackagesResponse {
                        schema_version: ROCO_SERVER_SCHEMA_VERSION,
                        total_count: Some(total_count),
                        data,
                    }),
                    StatusCode::OK,
                )),
                Err(err) => Err(deny_as_rejection(
                    &request_id,
                    RocoServerDenyCode::InternalAuthorizationError,
                    format!("Error: {}", err),
                    None,
                )),
            }
        });

    let bad_json = api_base
        .and(warp::path!("bad" / "json"))
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|client_fingerprint_hint: Option<String>, request_id: String| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            match get_local_bad_packages() {
                Ok(data) => Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&RocoServerPackagesResponse {
                        schema_version: ROCO_SERVER_SCHEMA_VERSION,
                        total_count: Some(data.len()),
                        data,
                    }),
                    StatusCode::OK,
                )),
                Err(err) => Err(deny_as_rejection(
                    &request_id,
                    RocoServerDenyCode::InternalAuthorizationError,
                    format!("Error: {}", err),
                    None,
                )),
            }
        });

    let source_json = api_base
        .and(warp::path!("source" / "json"))
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|client_fingerprint_hint: Option<String>, request_id: String| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            match get_choco_sources() {
                Ok(data) => Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&RocoServerFeedsResponse {
                        schema_version: ROCO_SERVER_SCHEMA_VERSION,
                        data,
                    }),
                    StatusCode::OK,
                )),
                Err(err) => Err(deny_as_rejection(
                    &request_id,
                    RocoServerDenyCode::InternalAuthorizationError,
                    format!("Error: {}", err),
                    None,
                )),
            }
        });

    let trust_state_json = api_base
        .and(warp::path!("trust" / "state" / "json"))
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|client_fingerprint_hint: Option<String>, request_id: String| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            Ok::<_, warp::Rejection>(warp::reply::json(&build_trust_state_response(request_id)))
        });

    let outdated = api_base
        .and(warp::path("outdated"))
        .and(warp::path::end())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|client_fingerprint_hint: Option<String>, request_id: String| req_outdated(false, false, client_fingerprint_hint, request_id));

    let outdated_r = api_base
        .and(warp::path!("outdated" / "r"))
        .and(warp::path::end())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|client_fingerprint_hint: Option<String>, request_id: String| req_outdated(true, false, client_fingerprint_hint, request_id));

    let outdated_l = api_base
        .and(warp::path!("outdated" / "l"))
        .and(warp::path::end())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|client_fingerprint_hint: Option<String>, request_id: String| req_outdated(false, true, client_fingerprint_hint, request_id));

    let outdated_json = api_base
        .and(warp::path!("outdated" / "json"))
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|client_fingerprint_hint: Option<String>, request_id: String| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            let (_, data) = get_outdated_packages("all", false, false, true, true).await;
            Ok::<_, warp::Rejection>(warp::reply::json(&RocoServerOutdatedResponse {
                schema_version: ROCO_SERVER_SCHEMA_VERSION,
                data,
            }))
        })
        .or(api_base
            .and(warp::path!("outdated" / "json"))
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::json())
            .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
            .and(with_request_id())
            .and_then(|req: RocoServerOutdatedRequest, client_fingerprint_hint: Option<String>, request_id: String| async move {
                check_protected_route_auth(&request_id, client_fingerprint_hint)?;
                let (_, data) = get_outdated_packages(
                    &req.pkg,
                    false,
                    req.pre,
                    req.ignore_pinned,
                    req.ignore_unfound,
                )
                .await;
                Ok::<_, warp::Rejection>(warp::reply::json(&RocoServerOutdatedResponse {
                    schema_version: ROCO_SERVER_SCHEMA_VERSION,
                    data,
                }))
            }));

    let local_deptree_json = api_base
        .and(warp::path!("local" / "deptree" / "json"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|req: RocoServerListRequest, client_fingerprint_hint: Option<String>, request_id: String| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            Ok::<_, warp::Rejection>(warp::reply::json(&RocoServerDependencyTreeResponse {
                schema_version: ROCO_SERVER_SCHEMA_VERSION,
                data: get_dependency_tree_nodes(&req.filter),
            }))
        });

    let search_json = api_base
        .and(warp::path!("search" / "json"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and_then(|req: RocoServerSearchRequest, client_fingerprint_hint: Option<String>, request_id: String| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            let terms: Vec<&str> = req.terms.iter().map(|s| s.as_str()).collect();
            match find_packages(&terms, false, req.prerelease).await {
                Ok(map) => {
                    let mut pkgs: Vec<_> = map.into_values().collect();
                    pkgs.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
                    Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&RocoServerPackagesResponse {
                            schema_version: ROCO_SERVER_SCHEMA_VERSION,
                            total_count: Some(pkgs.len()),
                            data: pkgs,
                        }),
                        StatusCode::OK,
                    ))
                }
                Err(err) => {
                    Err(deny_as_rejection(
                        &request_id,
                        RocoServerDenyCode::InternalAuthorizationError,
                        format!("Error: {}", err),
                        None,
                    ))
                }
            }
        });

    let runchoco = api_base
        .and(warp::path!("choco"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and(store_filter.clone())
        .and_then(
            |cmd: RocoServerChocoCommandRequest, client_fingerprint_hint: Option<String>, request_id: String, store: JobStore| async move {
                // Check authorization for protected route
                check_protected_route_auth(&request_id, client_fingerprint_hint)?;

                let allowed_commands = [
                    "install",
                    "upgrade",
                    "uninstall",
                    "list",
                    "search",
                    "info",
                    "outdated",
                ];

                if !allowed_commands.contains(&cmd.command.as_str()) {
                    return Err(deny_as_rejection(
                        &request_id,
                        RocoServerDenyCode::ForbiddenOperation,
                        format!("Forbidden choco command: {}", cmd.command),
                        None,
                    ));
                }

                let id = Uuid::new_v4();
                let job = JobState {
                    id,
                    status: JobStatus::Pending,
                    created_at: std::time::SystemTime::now(),
                    logs: Vec::new(),
                };

                {
                    let mut map = store.write().await;
                    map.insert(id, job.clone());
                }

                tokio::spawn(run_choco_background(id, cmd, store.clone()));

                // return 202 with lookup id
                let body = serde_json::json!({ "id": id.to_string(), "status": "accepted" });
                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&body),
                    warp::http::StatusCode::ACCEPTED,
                ))
            },
        );

    let status_get = api_base
        .and(warp::path!("choco" / "status" / String))
        .and(warp::get())
        .and(warp::header::optional::<String>(authorization::EMERGENCY_TRUST_OVERRIDE_HEADER))
        .and(with_request_id())
        .and(store_filter.clone())
        .and_then(move |id_str: String, client_fingerprint_hint: Option<String>, request_id: String, store: JobStore| async move {
            check_protected_route_auth(&request_id, client_fingerprint_hint)?;
            match Uuid::parse_str(&id_str) {
                Ok(id) => {
                    let map = store.read().await;
                    match map.get(&id) {
                        Some(job) => Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&job),
                            warp::http::StatusCode::OK,
                        )),
                        None => Err(deny_as_rejection_with_status(
                            &request_id,
                            RocoServerDenyCode::ResourceNotFound,
                            "Not found".to_string(),
                            None,
                            StatusCode::NOT_FOUND,
                        )),
                    }
                }
                Err(_) => Err(deny_as_rejection_with_status(
                    &request_id,
                    RocoServerDenyCode::InvalidRequest,
                    "Bad id".to_string(),
                    None,
                    StatusCode::BAD_REQUEST,
                )),
            }
        });

    let routes = local_json
        .or(local_json_post)
        .or(local_deptree_json)
        .or(bad_json)
        .or(source_json)
        .or(trust_state_json)
        .or(outdated)
        .or(outdated_r)
        .or(outdated_l)
        .or(outdated_json)
        .or(search_json)
        .or(runchoco)
        .or(status_get)
        .recover(handle_phase_auth_rejection);

    routes
        .with(warp::log::custom(|info| {
            println!(
                "{} - Received request: {} {} from {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                info.method(),
                info.path(),
                info.remote_addr()
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
        }))
        .with(warp::log("rocolatey_server"))
}

/// Handle PhaseAuthError rejections and convert them to HTTP responses
async fn handle_phase_auth_rejection(err: warp::Rejection) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(auth_err) = err.find::<PhaseAuthError>() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&auth_err.response),
            auth_err.status,
        ));
    }
    Err(err)
}

/// Build the renewal-only warp filter served on the old cert during overlap window.
pub(crate) fn create_renewal_filter(
    new_cert_pem: String,
    continuity_proof: String,
    previous_fingerprint: String,
    new_fingerprint: String,
    issued_at_utc: String,
) -> impl Filter<Extract = impl warp::Reply, Error = std::convert::Infallible> + Clone {
    let new_cert_pem = std::sync::Arc::new(new_cert_pem);
    let continuity_proof = std::sync::Arc::new(continuity_proof);
    let previous_fingerprint = std::sync::Arc::new(previous_fingerprint);
    let new_fingerprint = std::sync::Arc::new(new_fingerprint);
    let issued_at_utc = std::sync::Arc::new(issued_at_utc);

    warp::any()
        .and(warp::path!("rocolatey" / "trust" / "renew"))
        .and(warp::path::end())
        .and(warp::get())
        .map(move || {
            audit::emit_audit_event(
                &audit::server_audit_log_path(),
                &audit::AuditEvent::new(
                    audit::AuditEventKind::Renewal,
                    format!(
                        "Renewal served: {} -> {}",
                        &(*previous_fingerprint)[..std::cmp::min(12, (*previous_fingerprint).len())],
                        &(*new_fingerprint)[..std::cmp::min(12, (*new_fingerprint).len())]
                    ),
                ),
            );
            let response = RocoServerTrustRenewResponse {
                schema_version: ROCO_SERVER_SCHEMA_VERSION,
                new_server_cert_pem: (*new_cert_pem).clone(),
                continuity_proof: (*continuity_proof).clone(),
                previous_fingerprint: (*previous_fingerprint).clone(),
                new_fingerprint: (*new_fingerprint).clone(),
                issued_at_utc: (*issued_at_utc).clone(),
            };
            warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            )
        })
        .recover(|_| async { Ok::<_, std::convert::Infallible>(StatusCode::NOT_FOUND) })
}

async fn req_outdated(
    limit_output: bool,
    list_output: bool,
    client_fingerprint_hint: Option<String>,
    request_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    check_protected_route_auth(&request_id, client_fingerprint_hint)?;
    let result =
        get_outdated_packages_text("all", limit_output, list_output, false, true, true).await;
    Ok(result)
}

async fn run_choco_background(id: Uuid, cmd: RocoServerChocoCommandRequest, store: JobStore) {
    // mark Running
    {
        let mut map = store.write().await;
        if let Some(job) = map.get_mut(&id) {
            job.status = JobStatus::Running;
            /*
            job.logs.push(format!(
                "Started at {}",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            ));
            */
            println!(" - Started job {}: {:?} {:?}", id, cmd.command, cmd.args);
        }
    }

    // spawn process and stream stdout/stderr linewise into job.logs
    use std::process::Stdio;
    use tokio::process::Command as TokioCommand;

    let mut child = match TokioCommand::new("choco.exe")
        .arg(&cmd.command)
        .args(&cmd.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let mut map = store.write().await;
            if let Some(job) = map.get_mut(&id) {
                job.status = JobStatus::Failed {
                    error: format!("spawn error: {}", e),
                };
                job.logs.push(format!("spawn error: {}", e));
            }
            return;
        }
    };

    // merge stdout and stderr reading tasks (simple approach: read both concurrently)
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(tokio::spawn(read_stream_into_logs(
            stdout,
            id,
            store.clone(),
            "", // "stdout",
        )));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(tokio::spawn(read_stream_into_logs(
            stderr,
            id,
            store.clone(),
            "", // "stderr",
        )));
    }

    // wait for child to exit
    let status = child.wait().await;

    // wait for readers to finish
    for r in readers {
        let _ = r.await;
    }

    match status {
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            let mut map = store.write().await;
            if let Some(job) = map.get_mut(&id) {
                job.status = JobStatus::Completed { exit_code: code };
                // job.logs.push(format!("Process exited with code {}", code));
            }
        }
        Err(e) => {
            let mut map = store.write().await;
            if let Some(job) = map.get_mut(&id) {
                job.status = JobStatus::Failed {
                    error: format!("wait error: {}", e),
                };
                job.logs.push(format!("wait error: {}", e));
            }
        }
    }
}

// helper to read stdout/stderr lines and append to job logs
async fn read_stream_into_logs<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    stream: R,
    id: Uuid,
    store: JobStore,
    label: &str,
) {
    let mut reader = tokio::io::BufReader::new(stream);
    loop {
        let mut buf = Vec::new();
        match tokio::io::AsyncBufReadExt::read_until(&mut reader, b'\n', &mut buf).await {
            Ok(0) => break, // stream closed
            Ok(_) => {
                // convert bytes to string, replacing invalid UTF-8 sequences
                let s = String::from_utf8_lossy(&buf);
                let line = s.trim_end_matches(|c| c == '\n' || c == '\r');

                let mut map = store.write().await;
                if let Some(job) = map.get_mut(&id) {
                    let entry = if label.is_empty() {
                        line.to_string()
                    } else {
                        format!("[{}] {}", label, line)
                    };

                    job.logs.push(entry);
                    if job.logs.len() > 1000 {
                        job.logs.drain(0..(job.logs.len() - 1000));
                    }
                }
            }
            Err(e) => {
                let mut map = store.write().await;
                if let Some(job) = map.get_mut(&id) {
                    job.logs.push(format!("stream read error: {}", e));
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::test::request;

    #[tokio::test]
    async fn test_outdated_endpoint() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/outdated")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), 200);
        // assert!(std::str::from_utf8(response.body()).unwrap().contains("outdated packages"));
    }

    #[tokio::test]
    async fn test_unmatched_endpoint() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/unknown")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), 404);
        // assert!(std::str::from_utf8(response.body()).unwrap().contains("Not Found"));
    }

    #[tokio::test]
    async fn test_trust_state_endpoint_json_shape() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/trust/state/json")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), 200);

        let value: serde_json::Value =
            serde_json::from_slice(response.body()).expect("trust state should be valid json");
        assert_eq!(value["schema_version"], ROCO_SERVER_SCHEMA_VERSION);
        assert!(value.get("request_id").is_some());
        assert!(value.get("trust_mode").is_some());
        assert!(value.get("server_key_present").is_some());
        assert!(value.get("server_cert_present").is_some());
        assert!(value.get("authorized_key_count").is_some());
    }

    #[tokio::test]
    async fn test_choco_forbidden_uses_deny_contract() {
        let warp_filter = create_warp_filter();
        let payload = serde_json::json!({
            "command": "invalid-command",
            "args": ["git"]
        });

        let response = request()
            .method("POST")
            .path("/rocolatey/choco")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&payload).expect("serialize payload"))
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let deny: RocoServerDenyResponse =
            serde_json::from_slice(response.body()).expect("deny contract payload");
        assert_eq!(deny.schema_version, ROCO_SERVER_SCHEMA_VERSION);
        assert_eq!(deny.code, RocoServerDenyCode::ForbiddenOperation);
        assert!(!deny.request_id.is_empty());
    }

    #[tokio::test]
    async fn test_choco_status_bad_id_uses_deny_contract() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/choco/status/not-a-uuid")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let deny: RocoServerDenyResponse =
            serde_json::from_slice(response.body()).expect("deny contract payload");
        assert_eq!(deny.schema_version, ROCO_SERVER_SCHEMA_VERSION);
        assert_eq!(deny.code, RocoServerDenyCode::InvalidRequest);
        assert!(!deny.request_id.is_empty());
    }

    #[tokio::test]
    async fn test_choco_status_not_found_uses_not_found_deny_code() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/choco/status/00000000-0000-0000-0000-000000000000")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let deny: RocoServerDenyResponse =
            serde_json::from_slice(response.body()).expect("deny contract payload");
        assert_eq!(deny.schema_version, ROCO_SERVER_SCHEMA_VERSION);
        assert_eq!(deny.code, RocoServerDenyCode::ResourceNotFound);
        assert!(!deny.request_id.is_empty());
    }
}

/// Phase 7: Integration tests for deny contracts, request correlation, and error codes.
#[cfg(test)]
mod phase7_integration_tests {
    use super::*;
    use warp::test::request;

    #[tokio::test]
    async fn deny_response_always_has_uuid_request_id() {
        let warp_filter = create_warp_filter();
        let payload = serde_json::json!({ "command": "invalid-command", "args": ["git"] });

        let response = request()
            .method("POST")
            .path("/rocolatey/choco")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&payload).unwrap())
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let deny: RocoServerDenyResponse = serde_json::from_slice(response.body()).unwrap();
        // UUID format: 36 chars with 4 hyphens
        assert_eq!(deny.request_id.len(), 36);
        assert_eq!(deny.request_id.chars().filter(|c| *c == '-').count(), 4);
    }

    #[tokio::test]
    async fn deny_response_has_schema_version_1() {
        let warp_filter = create_warp_filter();
        let payload = serde_json::json!({ "command": "invalid-command", "args": [] });

        let response = request()
            .method("POST")
            .path("/rocolatey/choco")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&payload).unwrap())
            .reply(&warp_filter)
            .await;

        let deny: RocoServerDenyResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(deny.schema_version, ROCO_SERVER_SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn forbidden_choco_command_denial_has_no_enrollment_hint() {
        let warp_filter = create_warp_filter();
        let payload = serde_json::json!({ "command": "invalid-command", "args": ["chocolatey"] });

        let response = request()
            .method("POST")
            .path("/rocolatey/choco")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&payload).unwrap())
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let deny: RocoServerDenyResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(deny.code, RocoServerDenyCode::ForbiddenOperation);
        assert!(deny.enrollment_hint.is_none());
    }

    #[tokio::test]
    async fn deny_codes_serialize_snake_case() {
        let code = RocoServerDenyCode::NotEnrolledClient;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"not_enrolled_client\"");

        let code = RocoServerDenyCode::TimeSkewExceeded;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"time_skew_exceeded\"");

        let code = RocoServerDenyCode::ForbiddenOperation;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"forbidden_operation\"");

        let code = RocoServerDenyCode::InvalidRequest;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"invalid_request\"");

        let code = RocoServerDenyCode::ResourceNotFound;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"resource_not_found\"");
    }

    #[tokio::test]
    async fn two_denials_have_distinct_request_ids() {
        let warp_filter = create_warp_filter();
        let payload = serde_json::json!({ "command": "invalid-command", "args": ["git"] });

        let r1 = request()
            .method("POST")
            .path("/rocolatey/choco")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&payload).unwrap())
            .reply(&warp_filter)
            .await;

        let r2 = request()
            .method("POST")
            .path("/rocolatey/choco")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&payload).unwrap())
            .reply(&warp_filter)
            .await;

        let d1: RocoServerDenyResponse = serde_json::from_slice(r1.body()).unwrap();
        let d2: RocoServerDenyResponse = serde_json::from_slice(r2.body()).unwrap();
        assert_ne!(d1.request_id, d2.request_id);
    }

    #[tokio::test]
    async fn inbound_request_id_header_is_ignored() {
        let warp_filter = create_warp_filter();
        let payload = serde_json::json!({ "command": "invalid-command", "args": ["git"] });
        let supplied = "client-supplied-request-id";

        let response = request()
            .method("POST")
            .path("/rocolatey/choco")
            .header("content-type", "application/json")
            .header("x-request-id", supplied)
            .body(serde_json::to_vec(&payload).unwrap())
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let deny: RocoServerDenyResponse = serde_json::from_slice(response.body()).unwrap();
        assert_ne!(deny.request_id, supplied);
        assert_eq!(deny.request_id.len(), 36);
        assert_eq!(deny.request_id.chars().filter(|c| *c == '-').count(), 4);
    }

    #[tokio::test]
    async fn trust_state_has_all_required_fields() {
        let warp_filter = create_warp_filter();

        let response = request()
            .method("GET")
            .path("/rocolatey/trust/state/json")
            .reply(&warp_filter)
            .await;

        assert_eq!(response.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(response.body()).unwrap();

        for field in &[
            "schema_version",
            "request_id",
            "trust_mode",
            "server_key_present",
            "server_cert_present",
            "authorized_key_count",
        ] {
            assert!(body.get(field).is_some(), "missing field: {}", field);
        }
    }

    #[tokio::test]
    async fn protected_json_routes_ok_in_test_mode() {
        let warp_filter = create_warp_filter();

        let routes = vec![
            "/rocolatey/local/json",
            "/rocolatey/bad/json",
            "/rocolatey/source/json",
        ];

        for path in routes {
            let response = request()
                .method("GET")
                .path(path)
                .reply(&warp_filter)
                .await;

            assert_eq!(
                response.status(),
                200,
                "route {} should return 200 in test mode",
                path
            );
        }
    }

    #[tokio::test]
    async fn renewal_endpoint_returns_valid_json() {
        let filter = create_renewal_filter(
            "new-cert-pem".to_string(),
            "proof-abc->def".to_string(),
            "old-fingerprint-aaa".to_string(),
            "new-fingerprint-bbb".to_string(),
            "2025-06-15T12:00:00Z".to_string(),
        );

        let resp = request()
            .method("GET")
            .path("/rocolatey/trust/renew")
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
        assert_eq!(body["schema_version"], 1);
        assert_eq!(body["new_server_cert_pem"], "new-cert-pem");
        assert_eq!(body["continuity_proof"], "proof-abc->def");
        assert_eq!(body["previous_fingerprint"], "old-fingerprint-aaa");
        assert_eq!(body["new_fingerprint"], "new-fingerprint-bbb");
        assert_eq!(body["issued_at_utc"], "2025-06-15T12:00:00Z");
    }

    #[tokio::test]
    async fn renewal_endpoint_returns_404_on_wrong_path() {
        let filter = create_renewal_filter(
            "cert".to_string(),
            "proof".to_string(),
            "old".to_string(),
            "new".to_string(),
            "2025-01-01T00:00:00Z".to_string(),
        );

        let resp = request()
            .method("GET")
            .path("/rocolatey/trust/wrong")
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn renewal_endpoint_rejects_post() {
        let filter = create_renewal_filter(
            "cert".to_string(),
            "proof".to_string(),
            "old".to_string(),
            "new".to_string(),
            "2025-01-01T00:00:00Z".to_string(),
        );

        let resp = request()
            .method("POST")
            .path("/rocolatey/trust/renew")
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn renewal_endpoint_deserializes_to_trust_renew_response() {
        let filter = create_renewal_filter(
            "new-cert-pem-data".to_string(),
            "continuity-proof-data".to_string(),
            "prev-fp-data".to_string(),
            "new-fp-data".to_string(),
            "2025-06-15T12:00:00Z".to_string(),
        );

        let resp = request()
            .method("GET")
            .path("/rocolatey/trust/renew")
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), 200);
        let parsed: rocolatey_lib::server::RocoServerTrustRenewResponse =
            serde_json::from_slice(resp.body()).expect("must deserialize to RocoServerTrustRenewResponse");
        assert_eq!(parsed.schema_version, rocolatey_lib::server::ROCO_SERVER_SCHEMA_VERSION);
        assert_eq!(parsed.new_server_cert_pem, "new-cert-pem-data");
        assert_eq!(parsed.continuity_proof, "continuity-proof-data");
        assert_eq!(parsed.previous_fingerprint, "prev-fp-data");
        assert_eq!(parsed.new_fingerprint, "new-fp-data");
    }
}
