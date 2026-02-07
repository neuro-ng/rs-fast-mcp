use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use rs_fast_mcp::mcp::types::JsonRpcRequest;
use rs_fast_mcp::server::auth::AuthProvider;
use rs_fast_mcp::server::auth::providers::google::GoogleProvider;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

async fn mock_google_tokeninfo(req: actix_web::HttpRequest) -> impl Responder {
    let query_str = req.query_string();
    if query_str.contains("access_token=valid-token") {
        HttpResponse::Ok().json(json!({
            "sub": "user-123",
            "aud": "test-client-id",
            "scope": "email profile",
            "email": "test@example.com",
            "email_verified": "true"
        }))
    } else {
        HttpResponse::BadRequest().json(json!({
            "error": "invalid_token"
        }))
    }
}

#[tokio::test]
async fn test_google_provider_verification() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
    let port = listener.local_addr().expect("Failed to get port").port();

    let server =
        HttpServer::new(|| App::new().route("/tokeninfo", web::get().to(mock_google_tokeninfo)))
            .listen(listener)
            .expect("Failed to listen")
            .run();

    let server_handle = server.handle();
    tokio::spawn(server);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let validation_url = format!("http://127.0.0.1:{}/tokeninfo", port);
    let provider = GoogleProvider::new("test-client-id").with_validation_url(&validation_url);

    // Test Valid Token
    let mut metadata = HashMap::new();
    metadata.insert(
        "Authorization".to_string(),
        "Bearer valid-token".to_string(),
    );

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(),
        params: None,
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        transport_metadata: Some(metadata),
    };

    let context = provider.verify(&req).await.expect("Verification failed");
    assert_eq!(context.user_id.unwrap(), "user-123");
    assert_eq!(context.client_id.unwrap(), "test-client-id");

    // Test Invalid Token
    let mut metadata_invalid = HashMap::new();
    metadata_invalid.insert(
        "Authorization".to_string(),
        "Bearer invalid-token".to_string(),
    );

    let req_invalid = JsonRpcRequest {
        transport_metadata: Some(metadata_invalid),
        ..req.clone()
    };

    let err = provider.verify(&req_invalid).await;
    assert!(err.is_err());

    server_handle.stop(true).await;
}

async fn mock_oidc_discovery(req: actix_web::HttpRequest) -> impl Responder {
    let host = req.headers().get("host").unwrap().to_str().unwrap();
    let port = host.split(':').next_back().unwrap();
    let base_url = format!("http://127.0.0.1:{}", port);

    HttpResponse::Ok().json(json!({
        "issuer": base_url,
        "jwks_uri": format!("{}/jwks.json", base_url),
        "response_types_supported": ["id_token"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"]
    }))
}

async fn mock_jwks() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "keys": [{
            "kty": "RSA",
            "use": "sig",
            "kid": "test-key-id",
            "alg": "RS256",
            "n": "tH2nwXjcNZc9Ohlqgv6SkuW9p8N2Un_8ns2yVTh_8wI0TAqJ1sMpFcmWyeq-bcaAX2Zmbwwqyhg-J6FVbTYSB_7K66_ZWqiq_GXv-eQNmyc4sg1Z71N1JwZuayjWFbJf_020t-dZ1lwTGhS9R0ytONybG-Lj4KPD6dH7FNRb5ReaRT2Y80rCqG2ZNMdz1vl36KTKf7x3y81cxLEjq3i5OzIRy3GMRySVlwiYH1wVK5W2ogKk8Y-u7eSUt7CbHIeqEoP7HoKxDY5e0-s0Cl7n2kyuYBID01LK66oSbzC8IFou5ufduvw0iBC-m-_DVtu_q5K7ffLE_3_2g7G-7Yl2SQ",
            "e": "AQAB"
        }]
    }))
}

#[tokio::test]
async fn test_oidc_provider_verification() {
    use jsonwebtoken::{EncodingKey, Header, encode};
    use rs_fast_mcp::server::auth::oidc::OIDCProvider;
    use serde::Serialize;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
    let port = listener.local_addr().expect("Failed to get port").port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let server = HttpServer::new(|| {
        App::new()
            .route(
                "/.well-known/openid-configuration",
                web::get().to(mock_oidc_discovery),
            )
            .route("/jwks.json", web::get().to(mock_jwks))
    })
    .listen(listener)
    .expect("Failed to listen")
    .run();

    let server_handle = server.handle();
    tokio::spawn(server);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Initialize Provider (Discovery happens here)
    let provider = OIDCProvider::new(&base_url, "test-client-id")
        .await
        .expect("Failed to create provider");

    // Generate Configured JWT
    #[derive(Debug, Serialize)]
    struct Claims {
        sub: String,
        iss: String,
        aud: String,
        exp: usize,
    }

    let my_claims = Claims {
        sub: "user-456".to_string(),
        iss: base_url.clone(),
        aud: "test-client-id".to_string(),
        exp: 10000000000,
    };

    let priv_pem = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC0fafBeNw1lz06
GWqC/pKS5b2nw3ZSf/yezbJVOH/zAjRMConWwykVyZbJ6r5txoBfZmZvDCrKGD4n
oVVtNhIH/srrr9laqKr8Ze/55A2bJziyDVnvU3UnBm5rKNYVsl//TbS351nWXBMa
FL1HTK043Jsb4uPgo8Pp0fsU1FvlF5pFPZjzSsKobZk0x3PW+XfopMp/vHfLzVzE
sSOreLk7MhHLcYxHJJWXCJgfXBUrlbaiAqTxj67t5JS3sJsch6oSg/segrENjl7T
6zQKXufaTK5gEgPTUsrrqhJvMLwgWi7m5926/DSIEL6b78NW27+rkrt98sT/f/aD
sb7tiXZJAgMBAAECggEABad6Ezj/Yl5nEjmKI9yssK7/H5mBTL+IWJAojCrnnZ8h
H3tgxHYdfC3HdNk1e1Lk4hg0d20LfEA0kkNjVS6m2fR4GIMhfmdwJOZv0Q4fWal6
wKxjd82+TyDMLt6uOoBO9l6m8vAv6igBVGNNIzTNl1XlE+nDstUQqh6Gao/VC8/n
+n6E/gRXt8Iae0ZO9yFOhl6OOEinpRrvyl/Jpa8ULeYu0n3aTcpzUskvZJ0SXHWM
z4yp+0gn3lwLyOyozksB3LsOplLCPew0dOpQG0hyra8/wS0paQifdnLShR/WDAlv
f/GJbWOChSOl632IoVFST8G2OCdXP9vsFSchUNhd0QKBgQDlq5TL0bh8SqQiGqCq
9X4S8bMUXPQm1moXQAbrlQQ1cOgbGTulbhOWBmFOst2WrtVcJobCr+eQiUFFRpdj
Q8KYqumDMi6KmlBo+JSkpS8IWirbNyUgJAXMxlAqOp1AtlDDeV7SZfpJFtZBGS0q
9lYADt53rbhbkzK1u9frq2YXcwKBgQDJLr62FncgDG1hN6V9wOUxRC9JFYXKLAen
JWuBQY7ImQP63kAkxclWGK/LHIip6/y+y/eWHkbumNHLP12y+Rz25yxAfNybztfI
S9BMQwkXKbR7bHod3cxZVhySMQYRoAjYfFX3SFrgsBGTZczST4VDQG31X8+vDQH6
2kU3toG0UwKBgCZjOWmf0iAkMa7pmHU6tynfcDk1GDHtoKnmL8HslFmCV6k/3HJY
JbnrsxP+XX80FcFjRx7/W8sSxfAYTnFu//WYi5M8Lf9Ir6v78Ixcd7IDsCoX24K1
wqppczi7t1D7qCAkBy9PkDfrM0CKPrxxlApKcfC6/pd/0PgDP6HKcjP1AoGAcyDN
AmbYxP5Xmcq+abh5cDgU1z350jhgKMbBPrbFfwYRwP5utpx5G0wFTbaGfrcNbCJN
DRtGfEP3ytf4RvNIIAMqz7ykgoVb9sNr8Dhse1Tic78gIvdKedVNhFuJnYx3g2uj
xl8honMfm7ol/DSFjnbQdhrePs6y01sVQUyv7QECgYEAjocnSkWHS3cpZOio76pS
L0XX1vzA+6SSSIkTdsVRDHODIm0Koky7QZpRNoOcu2BxeF+qa+320p8Z7/wI21hB
8I+6pr+hyL+7HwhFwBS2ytj/aglsGcINZMFBomGlAAVQhWvvpwuewfCYskpfcIuS
7xMzpWeSgPO80wAoesWdIYg=
-----END PRIVATE KEY-----"#;

    let key = EncodingKey::from_rsa_pem(priv_pem.as_bytes()).expect("Failed to load key");
    let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key-id".to_string());

    let token = encode(&header, &my_claims, &key).expect("Failed to sign token");

    // Verify
    let mut metadata = HashMap::new();
    metadata.insert("Authorization".to_string(), format!("Bearer {}", token));

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(),
        params: None,
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        transport_metadata: Some(metadata),
    };

    let context = provider.verify(&req).await.expect("Verification failed");
    assert_eq!(context.user_id.unwrap(), "user-456");
    assert_eq!(context.client_id.unwrap(), "test-client-id");

    server_handle.stop(true).await;
}
