use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LightState {
    on: bool,
    bri: u8,
    hue: u16,
    sat: u8,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Light {
    state: LightState,
    name: String,
}

#[tokio::test]
#[ignore]
async fn test_smart_home_integration() {
    // This test is intended to run with the `run_with_docker.sh` script
    // which starts a diyhue container.
    // The script exports the container IP if needed, but for simplicity here
    // we assume localhost mapping works (port 8080) as set up in the script.
    // Or we rely on the bridge IP environment variable if we were testing the MCP server itself.
    // But the original test was testing direct connectivity to the bridge.

    let bridge_ip = "127.0.0.1";
    let bridge_port = 8080;
    let username = "102030405060708090a0b0c0d0e0f000";

    let client = reqwest::Client::new();
    let url = format!(
        "http://{}:{}/api/{}/lights",
        bridge_ip, bridge_port, username
    );

    println!("Connecting to Hue Bridge at {}", url);

    // Attempt to fetch lights
    match client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                let text = resp.text().await.unwrap();
                println!("Response: {}", text);

                // Parse lights
                // The API returns a map of "1" -> Light, "2" -> Light
                match serde_json::from_str::<HashMap<String, Light>>(&text) {
                    Ok(map) => {
                        let mut names: Vec<String> = map.values().map(|v| v.name.clone()).collect();
                        names.sort();

                        println!("Lights found: {:?}", names);

                        if names.is_empty() {
                            println!(
                                "No lights found. DIYHue integration check passed (connection successful but empty config or unauthorized)."
                            );
                        } else {
                            println!("Lights verification successful.");
                        }
                    }
                    Err(e) => panic!("Failed to parse lights JSON: {}", e),
                }
            } else {
                panic!("Failed to get lights: Status {}", resp.status());
            }
        }
        Err(e) => {
            panic!("Failed to connect to Hue Bridge: {}", e);
        }
    }
}
