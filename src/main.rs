mod app;

use crate::app::{AppConfig, AppError, Result, app};
use axum::extract::State;
use axum::{Json, Router, routing::get, routing::post};
use log::info;
use serde::{Deserialize, Serialize};
use serde_email::Email;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing_subscriber::EnvFilter;
use url::Url;

#[derive(Deserialize)]
struct Config {
    endpoint: String,
    email: String,
    password: String,
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
        .with_state(state)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(
            TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::new().include_headers(true)),
        )
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    Ok(router)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=warn")),
        )
        .init();

    let config =
        envy::from_env::<Config>().unwrap_or_else(|error| panic!("could not read env: {}", error));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Starting server on {addr:#?}");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap_or_else(|error| panic!("{error:#?}"));

    let server = server(&config).await?;

    axum::serve(listener, server).await?;

    Ok(())
}
