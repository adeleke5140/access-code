mod app;

use axum::extract::State;
use axum::{Json, Router, routing::get, routing::post};
use serde::{Deserialize, Serialize};
use serde_email::Email;
use serde_json::{Value, json};
use std::sync::Arc;
use url::Url;

use crate::app::{AppConfig, AppError, Result, app};

#[derive(Deserialize)]
struct Config {
    endpoint: String,
    email: String,
    password: String,
    server_address: String,
}

#[derive(Serialize, Deserialize)]
struct PostBody {
    visitor: String,
    address: String,
}

async fn handle_access_code(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PostBody>,
) -> Result<Json<Value>> {
    let PostBody { visitor, address } = body;

    let code = app(AppConfig {
        email: state
            .email
            .clone(),
        endpoint: state
            .endpoint
            .clone(),
        password: state
            .password
            .clone(),
        visitor,
        address,
    })
    .await?;

    Ok(Json(json!({ "code": code })))
}

struct AppState {
    email: Email,
    endpoint: Url,
    password: String,
}

async fn json() -> Json<Value> {
    Json(json!({ "status": 200 }))
}

async fn server(config: &Config) -> Result<Router> {
    let Ok(email) = Email::new(&config.email) else {
        return Err(AppError::ParseError("Could not parse email".to_string()));
    };
    let Ok(endpoint) = Url::parse(&config.endpoint) else {
        return Err(AppError::InvalidBaseUrl(
            config
                .endpoint
                .to_owned(),
        ));
    };
    let password = String::from(&config.password);

    if password.is_empty() {
        panic!("a password must be set")
    };

    let state = AppState {
        email,
        endpoint,
        password,
    };

    let state = Arc::new(state);
    let router = Router::new()
        .route("/access-code", post(handle_access_code))
        .route("/", get(json))
        .with_state(state);

    Ok(router)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config =
        envy::from_env::<Config>().unwrap_or_else(|error| panic!("could not read env: {}", error));

    let listener = tokio::net::TcpListener::bind(&config.server_address)
        .await
        .unwrap_or_else(|error| panic!("{error:#?}"));

    let server = server(&config).await?;

    axum::serve(listener, server).await?;

    Ok(())
}
