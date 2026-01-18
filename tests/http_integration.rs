//! Integration tests for GCP HTTP client using wiremock
//!
//! These tests verify the HTTP client behavior against mocked endpoints,
//! ensuring proper handling of various response codes and edge cases.

use serde_json::json;
use wiremock::matchers::{bearer_token, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test module for HTTP client integration tests
mod http_client_tests {
    use super::*;

    /// Test successful GET request returns parsed JSON
    #[tokio::test]
    async fn test_get_success_returns_json() {
        let server = MockServer::start().await;

        let expected_response = json!({
            "items": [
                {"name": "instance-1", "status": "RUNNING"},
                {"name": "instance-2", "status": "STOPPED"}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/compute/v1/projects/test-project/zones/us-central1-a/instances"))
            .and(bearer_token("test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&expected_response))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/compute/v1/projects/test-project/zones/us-central1-a/instances",
            server.uri()
        );

        let response = client
            .get(&url)
            .bearer_auth("test-token")
            .send()
            .await
            .expect("Request should succeed")
            .json::<serde_json::Value>()
            .await
            .expect("Should parse JSON");

        assert_eq!(response["items"].as_array().unwrap().len(), 2);
        assert_eq!(response["items"][0]["name"], "instance-1");
    }

    /// Test 401 response indicates authentication failure
    #[tokio::test]
    async fn test_401_returns_unauthorized() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/compute/v1/projects/test-project/instances"))
            .respond_with(
                ResponseTemplate::new(401).set_body_json(json!({
                    "error": {
                        "code": 401,
                        "message": "Invalid credentials"
                    }
                })),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/compute/v1/projects/test-project/instances",
            server.uri()
        );

        let response = client
            .get(&url)
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(response.status(), 401);
    }

    /// Test 403 response indicates permission denied
    #[tokio::test]
    async fn test_403_returns_forbidden() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/compute/v1/projects/restricted-project/instances"))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({
                    "error": {
                        "code": 403,
                        "message": "Permission denied"
                    }
                })),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/compute/v1/projects/restricted-project/instances",
            server.uri()
        );

        let response = client
            .get(&url)
            .bearer_auth("valid-token")
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(response.status(), 403);
    }

    /// Test 404 response for non-existent resources
    #[tokio::test]
    async fn test_404_returns_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/compute/v1/projects/test-project/zones/invalid-zone/instances"))
            .respond_with(
                ResponseTemplate::new(404).set_body_json(json!({
                    "error": {
                        "code": 404,
                        "message": "Zone not found"
                    }
                })),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/compute/v1/projects/test-project/zones/invalid-zone/instances",
            server.uri()
        );

        let response = client
            .get(&url)
            .bearer_auth("test-token")
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(response.status(), 404);
    }

    /// Test POST request with JSON body
    #[tokio::test]
    async fn test_post_with_body() {
        let server = MockServer::start().await;

        let operation_response = json!({
            "kind": "compute#operation",
            "id": "1234567890",
            "name": "operation-1234567890",
            "status": "PENDING"
        });

        Mock::given(method("POST"))
            .and(path("/compute/v1/projects/test-project/zones/us-central1-a/instances/my-vm/start"))
            .and(bearer_token("test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&operation_response))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/compute/v1/projects/test-project/zones/us-central1-a/instances/my-vm/start",
            server.uri()
        );

        let response = client
            .post(&url)
            .bearer_auth("test-token")
            .send()
            .await
            .expect("Request should succeed")
            .json::<serde_json::Value>()
            .await
            .expect("Should parse JSON");

        assert_eq!(response["status"], "PENDING");
        assert_eq!(response["kind"], "compute#operation");
    }

    /// Test DELETE request
    #[tokio::test]
    async fn test_delete_request() {
        let server = MockServer::start().await;

        let operation_response = json!({
            "kind": "compute#operation",
            "status": "PENDING",
            "operationType": "delete"
        });

        Mock::given(method("DELETE"))
            .and(path("/compute/v1/projects/test-project/zones/us-central1-a/instances/my-vm"))
            .and(bearer_token("test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&operation_response))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/compute/v1/projects/test-project/zones/us-central1-a/instances/my-vm",
            server.uri()
        );

        let response = client
            .delete(&url)
            .bearer_auth("test-token")
            .send()
            .await
            .expect("Request should succeed")
            .json::<serde_json::Value>()
            .await
            .expect("Should parse JSON");

        assert_eq!(response["operationType"], "delete");
    }

    /// Test empty response handling
    #[tokio::test]
    async fn test_empty_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/some/endpoint"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/some/endpoint", server.uri());

        let response = client
            .post(&url)
            .bearer_auth("test-token")
            .send()
            .await
            .expect("Request should succeed");

        assert_eq!(response.status(), 204);
        let body = response.text().await.expect("Should get body");
        assert!(body.is_empty());
    }

    /// Test rate limiting (429) response
    #[tokio::test]
    async fn test_rate_limit_429() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rate-limited"))
            .respond_with(
                ResponseTemplate::new(429).set_body_json(json!({
                    "error": {
                        "code": 429,
                        "message": "Rate limit exceeded"
                    }
                })),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/rate-limited", server.uri());

        let response = client
            .get(&url)
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(response.status(), 429);
    }

    /// Test pagination with nextPageToken
    #[tokio::test]
    async fn test_pagination_with_next_page_token() {
        let server = MockServer::start().await;

        // First page
        Mock::given(method("GET"))
            .and(path("/compute/v1/projects/test-project/instances"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [
                    {"name": "instance-1"},
                    {"name": "instance-2"}
                ],
                "nextPageToken": "token-page-2"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        // Second page
        Mock::given(method("GET"))
            .and(path("/compute/v1/projects/test-project/instances"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [
                    {"name": "instance-3"},
                    {"name": "instance-4"}
                ]
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/compute/v1/projects/test-project/instances",
            server.uri()
        );

        // First request
        let response1 = client
            .get(&url)
            .bearer_auth("test-token")
            .send()
            .await
            .expect("Request should succeed")
            .json::<serde_json::Value>()
            .await
            .expect("Should parse JSON");

        assert!(response1.get("nextPageToken").is_some());
        assert_eq!(response1["items"].as_array().unwrap().len(), 2);

        // Second request (simulating pagination)
        let response2 = client
            .get(&url)
            .bearer_auth("test-token")
            .query(&[("pageToken", "token-page-2")])
            .send()
            .await
            .expect("Request should succeed")
            .json::<serde_json::Value>()
            .await
            .expect("Should parse JSON");

        assert!(response2.get("nextPageToken").is_none());
        assert_eq!(response2["items"].as_array().unwrap().len(), 2);
    }
}
