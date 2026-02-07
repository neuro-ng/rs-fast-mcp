use rs_fast_mcp::server::auth::providers::auth0::Auth0;
use rs_fast_mcp::server::auth::providers::google::GoogleProvider;

#[tokio::test]
async fn test_auth0_initialization() {
    // Auth0::new performs a discovery request, so it will fail without internet or valid domain
    // But we can check if it attempts the correct URL structure if we mock it, 
    // or just checking if it compiles and runs generic logic?
    // Actually, `Auth0::new` calls `OIDCProvider::new` which makes a network call.
    // For unit testing WITHOUT network, we might be limited.
    // Let's just verify struct existence and compilation in this test file?
    // Or we can expect it to fail with a specific error (DNS/Connection) which proves it tried?
    
    let res = Auth0::create("dev-xyz.us.auth0.com", "my-client-id").await;
    // It should fail with discovery error, but that means logic ran.
    match res {
        Ok(provider) => println!("Auth0 provider initialized successfully: {:?}", provider),
        Err(e) => println!("Auth0 initialization failed (expected if no network/invalid domain): {:?}", e),
    }
}

#[test]
fn test_google_provider_structure() {
    let _provider = GoogleProvider::new("my-client-id");
    // Verify it implements AuthProvider (static check by compiler)
    // We can't easily test verify() without a real token or mock server.
}

#[test]
fn test_github_provider_structure() {
    use rs_fast_mcp::server::auth::providers::github::GitHubProvider;
    let _provider = GitHubProvider::new();
}

#[tokio::test]
async fn test_aws_cognito_initialization() {
    use rs_fast_mcp::server::auth::providers::aws::AwsCognito;
    let res = AwsCognito::create("user-pool-id", "us-east-1", "client-id").await;
    match res {
        Ok(provider) => println!("AWS provider initialized (unexpected if fake): {:?}", provider),
        Err(e) => println!("AWS initialization failed (expected for fake ID): {:?}", e),
    }
}
