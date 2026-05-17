use rocolatey_lib::server::RocoServerDenyResponse;

pub fn print_deny_if_present(endpoint: &str, body: &str) -> bool {
    let deny = match serde_json::from_str::<RocoServerDenyResponse>(body) {
        Ok(v) => v,
        Err(_) => return false,
    };

    eprintln!(
        "Server deny on {}: code={:?} request_id={} message={}",
        endpoint, deny.code, deny.request_id, deny.message
    );

    if let Some(hint) = deny.enrollment_hint {
        eprintln!("Hint: {}", hint);
    }

    if let Some(fp) = deny.short_fingerprint {
        eprintln!("Fingerprint: {}", fp);
    }

    true
}
