use axum::{Router, routing::get};
use reqwest;
use serde::Deserialize;
use serde_email::Email;
use std::error::Error;
use url::Url;

#[derive(Deserialize)]
struct Config {
    endpoint: String,
    email: String,
    password: String,
    server_address: String,
}

#[derive(Debug)]
struct AppConfig {
    email: Email,
    password: String,
    endpoint: Url,
}

async fn verify_endpoint_login(config: AppConfig) -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let json_data = format!(
        r#"{{ "user_name": {}, user_psswrd: {}}}"#,
        config.email, config.password
    );

    let handshake = client.get(&config.endpoint.to_string()).send().await?;

    if handshake.status().is_success() {
        println!("reached server Successfully")
    }

    // let response = client
    //     .post(config.endpoint)
    //     .header("Content-Type", "x-www-form-urlencoded")
    //     .body(json_data)
    //     .send()
    //     .await?;

    // if response.status().is_success() {
    //    println!("Successfully logged in")
    //} else {
    //    match response.error_for_status() {
    //        Ok(_) => (),
    //        Err(e) => {
    //            eprintln!("error is: {}", e)
    //        }
    //    }
    // }

    Ok(())
}

#[tokio::main]
async fn app() -> Result<Router, Box<dyn Error>> {
    let router = Router::new().route("/", get(|| async { "Welcome to the access code server" }));
    Ok(router)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // TODO: there might be something better than panic!
    let config = envy::from_env::<Config>()
        .unwrap_or_else(|error| panic!("could not read config: {}", error));

    let email = Email::new(config.email).unwrap_or_else(|error| panic!("email error: {}", error));
    let endpoint =
        Url::parse(&config.endpoint).unwrap_or_else(|error| panic!("endpoint error: {}", error));
    let password = String::from(config.password);

    if password.is_empty() {
        panic!("a password must be set")
    };

    let listener = tokio::net::TcpListener::bind(&config.server_address)
        .await
        .unwrap_or_else(|error| panic!("{}", error));

    verify_endpoint_login(AppConfig {
        email,
        endpoint,
        password,
    })
    .await?;

    // axum::serve(listener, app()).await?;
    Ok(())
}
