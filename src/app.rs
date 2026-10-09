//TODO: what happens when things go wrong
// we need to handle errors properly
mod html;

use std::io;

use axum::response::Response;
use axum::{http::StatusCode, response::IntoResponse};
use regex::Regex;
use reqwest::{self, Client};
use serde::{Deserialize, Serialize};
use serde_email::Email;

use url::Url;

#[derive(Deserialize)]
struct ApiError {
    message: String,
    code: u16,
}

use html::{get_html_content, get_nested_html_content};

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Invalid base url: {0}")]
    InvalidBaseUrl(String),

    #[error("Http request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("API returned an error: {message} (code: {code})")]
    ApiError { message: String, code: u16 },

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::InvalidBaseUrl(_) => StatusCode::BAD_REQUEST,
            AppError::ParseError(_) | AppError::Io(_) | AppError::Reqwest(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::ApiError {
                message: _,
                code: _,
            } => StatusCode::NOT_FOUND,
        }
        .into_response()
    }
}

#[derive(Debug)]
pub struct AppConfig {
    pub email: Email,
    pub password: String,
    pub endpoint: Url,
    pub visitor: String,
    pub address: String,
}

#[derive(Serialize)]
struct FormData {
    user_name: String,
    user_psswrd: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Guest {
    pub guest_name: String,
    pub host_address: String,
}

pub struct AccessCode(String, u32);

pub fn get_name_and_number(el: &str) -> Result<AccessCode> {
    let name = Regex::new(r"(?<name>\w+)").unwrap();
    let number = Regex::new(r"(?<number>\d{5})").unwrap();
    let Some(caps) = name.captures(el) else {
        return Err(AppError::ParseError(String::from(
            "Could not find the name",
        )));
    };
    let Some(num_caps) = number.captures(el) else {
        return Err(AppError::ParseError(String::from(
            "Could not find the number",
        )));
    };
    let name = String::from(&caps["name"]);
    let number: u32 = num_caps["number"]
        .parse()
        .unwrap();
    Ok(AccessCode(name, number))
}

async fn get_access_code(
    client: &Client,
    create_endpoint: &str,
    guest: &Guest,
) -> Result<AccessCode> {
    let res = client
        .post(create_endpoint)
        .form(&guest)
        .send()
        .await?;

    if !res
        .status()
        .is_success()
    {
        let api_error: ApiError = res
            .json()
            .await?;
        return Err(AppError::ApiError {
            message: api_error.message,
            code: api_error.code,
        });
    }

    let html = res
        .text()
        .await?;

    let guest_with_code = get_html_content("h2", &html);
    let _ = get_nested_html_content("h5", "p", &html);

    let AccessCode(name, number) = get_name_and_number(&guest_with_code)?;

    Ok(AccessCode(name, number))
}

pub type Result<T = ()> = std::result::Result<T, AppError>;

pub async fn app(config: AppConfig) -> Result<u32> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()?;

    let res = client
        .get(
            config
                .endpoint
                .to_string(),
        )
        .send()
        .await?;

    if !res
        .status()
        .is_success()
    {
        let api_error: ApiError = res
            .json()
            .await?;
        return Err(AppError::ApiError {
            message: api_error.message,
            code: api_error.code,
        });
    }

    let form_data = FormData {
        user_name: config
            .email
            .to_string(),
        user_psswrd: config.password,
    };

    let endpoint = format!("{}/{}", config.endpoint, "login");

    let res = client
        .post(endpoint)
        .form(&form_data)
        .send()
        .await?;

    if !res
        .status()
        .is_success()
    {
        let api_error: ApiError = res
            .json()
            .await?;
        return Err(AppError::ApiError {
            message: api_error.message,
            code: api_error.code,
        });
    }

    let create_endpoint = format!("{}/{}", config.endpoint, "create");
    let res = client
        .get(create_endpoint.to_string())
        .send()
        .await?;

    if !res
        .status()
        .is_success()
    {
        let api_error: ApiError = res
            .json()
            .await?;
        return Err(AppError::ApiError {
            message: api_error.message,
            code: api_error.code,
        });
    }

    let guest = Guest {
        guest_name: config
            .visitor
            .to_string(),
        host_address: config
            .address
            .to_string(),
    };

    let mut state = String::new();

    let AccessCode(name, code) = get_access_code(&client, &create_endpoint, &guest).await?;

    state.push_str(&name);

    while state.to_lowercase()
        != guest
            .guest_name
            .to_lowercase()
    {
        let AccessCode(name, _) = get_access_code(&client, &create_endpoint, &guest).await?;
        state = name;
    }

    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HTML: &str = r#"
     !DOCTYPE html>
    <meta charset="utf-8">
    <title>Hello, world!</title>
    <h1 class="foo">Hello, <i>world!</i></h1>
    <h5><p>opeyemi adeleke </p></h5>
        "#;

    #[test]
    fn it_gets_text_from_html() {
        let text = get_html_content("h1", SAMPLE_HTML);
        assert_eq!(text, "Hello, world!")
    }

    #[test]
    fn it_gets_nested_text_from_html() {
        let text = get_nested_html_content("h5", "p", SAMPLE_HTML);
        assert_eq!(text, "opeyemi adeleke")
    }

    #[test]
    fn it_extracts_name_and_number_from_text() {
        let access_code = get_name_and_number("Opeyemi 51402");
        // Not sure tests should be checking the enum
        if let Ok(AccessCode(name, number)) = access_code {
            assert_eq!(name, "Opeyemi");
            assert_eq!(number, 51402)
        }
    }
}
