mod html;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::{extract::FromRequest, http::StatusCode, response::IntoResponse};
use html::get_html_content;
use regex::Regex;
use reqwest::{self, Client};
use serde::{Deserialize, Serialize};
use serde_email::Email;
use std::{
    fmt::Display,
    io,
    sync::{Arc, LazyLock},
};
use tracing::{Instrument, info_span};
use url::Url;

#[derive(Deserialize)]
struct ApiError {
    message: String,
    code: u16,
}

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

    #[error("Search error: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
struct AppJson<T>(T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, err) = match self {
            AppError::InvalidBaseUrl(_) => (
                StatusCode::BAD_REQUEST,
                "bad request".to_owned(),
                Some(self),
            ),
            AppError::ParseError(_)
            | AppError::Io(_)
            | AppError::Reqwest(_)
            | AppError::ApiError {
                message: _,
                code: _,
            } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong".to_owned(),
                Some(self),
            ),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "not found".to_owned(), Some(self)),
        };

        let mut response = (status, AppJson(ErrorResponse { message })).into_response();
        if let Some(err) = err {
            response
                .extensions_mut()
                .insert(Arc::new(err));
        }
        response
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

pub async fn log_app_errors(req: Request, next: Next) -> Response {
    let response = next
        .run(req)
        .await;
    if let Some(err) = response
        .extensions()
        .get::<Arc<AppError>>()
    {
        tracing::error!(?err, "An unexpected error occured inside the handler")
    }
    response
}

#[derive(Serialize, Debug)]
pub struct AccessCode(pub String, pub u32);

pub fn extract_name_and_number(el: &str) -> Result<AccessCode> {
    static NAME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*(?<name>\D+?)\s+\d+\s*$").unwrap());
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?<number>\d{5})").unwrap());

    let Some(caps) = NAME.captures(el) else {
        return Err(AppError::ParseError(String::from(
            "Could not find the name",
        )));
    };
    let Some(num_caps) = NUMBER.captures(el) else {
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

pub type Result<T = ()> = std::result::Result<T, AppError>;

struct ProxyClient {
    base_url: String,
    http_client: Client,
}

impl ProxyClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let endpoint = Url::parse(&base_url)
            .map_err(|_| AppError::InvalidBaseUrl("Base url is invalid".to_string()))?;

        Ok(Self {
            base_url: endpoint.to_string(),
            http_client: Client::builder()
                .cookie_store(true)
                .build()?,
        })
    }

    fn get_url<P: Display>(&self, path: P) -> String {
        format!(
            "{}/{}",
            self.base_url
                .trim_end_matches("/"),
            path
        )
    }

    pub async fn get_to_home(&self) -> Result<()> {
        let res = self
            .http_client
            .get(&self.base_url)
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

        Ok(())
    }

    pub async fn login(&self, form_data: FormData) -> Result<()> {
        let url = self.get_url("login");

        let res = self
            .http_client
            .post(url)
            .form(&form_data)
            .send()
            .instrument(info_span!("api", step = "logging in..."))
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

        Ok(())
    }

    pub async fn go_to_create(&self) -> Result<()> {
        let url = self.get_url("create");

        let res = self
            .http_client
            .get(url)
            .send()
            .instrument(info_span!("api", step = "opening create page"))
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
        Ok(())
    }

    pub async fn get_access_code(&self, guest: &Guest, retries: u32) -> Result<AccessCode> {
        log::info!("{:?}", retries);
        if retries < 1 {
            return Err(AppError::ApiError {
                message: "could not get code".to_string(),
                code: 500,
            });
        }

        let url = self.get_url("create");
        let res = self
            .http_client
            .post(url)
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
        let AccessCode(name, number) = extract_name_and_number(&guest_with_code)?;

        if name
            .trim()
            .to_lowercase()
            != guest
                .guest_name
                .trim()
                .to_lowercase()
        {
            log::info!("{:?} retrie(s) left", retries);
            let str = format!(
                "returned={} expected={} name mismatch, retrying",
                name, guest.guest_name
            );
            return Box::pin(self.get_access_code(guest, retries - 1))
                .instrument(info_span!("api", span = str))
                .await;
        }

        Ok(AccessCode(name, number))
    }
}

pub async fn app(config: AppConfig) -> Result<u32> {
    let client = ProxyClient::new(
        &config
            .endpoint
            .to_string(),
    )?;

    client
        .get_to_home()
        .await?;

    let form_data = FormData {
        user_name: config
            .email
            .to_string(),
        user_psswrd: config.password,
    };

    client
        .login(form_data)
        .await?;

    client
        .go_to_create()
        .await?;

    let guest = Guest {
        guest_name: config
            .visitor
            .trim()
            .to_string(),
        host_address: config
            .address
            .trim()
            .to_string(),
    };

    let AccessCode(name, code) = client
        .get_access_code(&guest, 5)
        .await?;

    log::info!("name: {:?} and code: {:?}", name, code);

    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::html::get_nested_html_content;

    const SAMPLE_HTML: &str = r#"
     <!DOCTYPE html>
    <meta charset="utf-8">
    <title>Hello, world!</title>
    <h1 class="foo">Hello, <i>world!</i></h1>
    <h2>mr obinna 33588 </h2>
    <h5><p>opeyemi adeleke </p></h5>
        "#;

    #[test]
    fn it_gets_text_from_html() {
        let text = get_html_content("h1", SAMPLE_HTML);
        assert_eq!(text, "Hello, world!")
    }

    #[test]
    fn it_gets_space_separated_text_from_html() {
        let text = get_html_content("h2", SAMPLE_HTML);
        assert_eq!(text, "mr obinna 33588")
    }

    #[test]
    fn it_gets_nested_text_from_html() {
        let text = get_nested_html_content("h5", "p", SAMPLE_HTML).unwrap();
        assert_eq!(text, "opeyemi adeleke")
    }

    #[test]
    fn it_extracts_name_and_number_from_text() {
        let access_code = extract_name_and_number("Opeyemi 51402");
        let AccessCode(name, number) = access_code.unwrap();
        assert_eq!(name, "Opeyemi");
        assert_eq!(number, 51402)
    }

    #[test]
    fn it_extracts_name_when_honorific_is_added() {
        let access_code = extract_name_and_number("mr obinna 33588");
        let AccessCode(name, number) = access_code.unwrap();
        assert_eq!(name, "mr obinna");
        assert_eq!(number, 33588)
    }
}
