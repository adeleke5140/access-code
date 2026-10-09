mod app;

use axum::extract::State;
use axum::response::Result;
use axum::{Json, Router, routing::post};
use serde::{Deserialize, Serialize};
use serde_email::Email;
use std::{error::Error, sync::Arc};
use url::Url;

use crate::app::{AppConfig, app};

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
) -> Result<Json<u32>> {
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
    .await;

    Ok(Json::from(code))
}

struct AppState {
    email: Email,
    endpoint: Url,
    password: String,
}

#[tokio::main]
// the reason I return a Result here is because of error propagation
// if I handled the error, there is no need for the caller to handle it again
// strictly saying: there should be no need for the caller to handle the error
async fn server(config: Config) -> Result<Router> {
    // If there is no email, endpoint or password, should the program be recoverable or not?
    let email = Email::new(config.email)?;
    let endpoint = Url::parse(&config.endpoint)?;
    let password = String::from(config.password);

    if password.is_empty() {
        //w also should just return an error here tbh
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
        .with_state(state);

    Ok(router)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config =
        envy::from_env::<Config>().unwrap_or_else(|error| panic!("could not read env: {}", error));

    let listener = tokio::net::TcpListener::bind(&config.server_address)
        .await
        .unwrap_or_else(|error| panic!("{error:#?}"));

    if let Ok(server) = server(config) {
        let axum_server = axum::serve(listener, server).await;

        if let Err(err) = axum_server {
            // here is where we should panic
            panic!("An error occured from the server: {}", err)
        }
    } else {
        // log the respective config error
        // how do we match the error here
        panic!("Could not start the server")
    }
    Ok(())
}
