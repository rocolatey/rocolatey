use rocolatey_lib::roco::nuget3;
use rocolatey_lib::roco::{Feed, FeedType, Package};
use serde_json::json;

#[tokio::test]
async fn test_nuget3_extract_packages() {
    let resp = r#"{\"data\":[{\"id\":\"testpkg\",\"version\":\"1.2.3\"}]}"#;
    // Fix: use a valid JSON string (no escaping)
    let resp = "{\"data\":[{\"id\":\"testpkg\",\"version\":\"1.2.3\"}]}".replace("\\\"", "\"");
    let mut pkgs = Vec::new();
    nuget3::extract_packages(&mut pkgs, &resp);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].id, "testpkg");
    assert_eq!(pkgs[0].version, "1.2.3");
}

#[tokio::test]
async fn test_nuget3_read_service_index() {
    let index_json = json!({
        "resources": [
            {"@id": "https://example.org/query", "@type": "SearchQueryService"}
        ]
    });
    let idx = nuget3::read_service_index(index_json).unwrap();
    assert!(idx.resources.is_some());
    let resources = idx.resources.unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id, "https://example.org/query");
    assert_eq!(resources[0].resource_type, "SearchQueryService");
}
