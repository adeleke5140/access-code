//TODO: what happens when things go wrong
// we need to handle errors properly
mod html;

use axum::response::Response;
use axum::{http::StatusCode, response::IntoResponse};
use regex::Regex;
use reqwest::{self, Client};
use serde::Serialize;
use serde_email::Email;

use url::Url;

use html::{get_html_content, get_nested_html_content};

// I don't think I've modelled all the possible type of errors
// that could occur
// Operations:
// GET /endpoint
// POST /endpoint/login
// GET /create
// POST /create
#[allow(dead_code)]
#[derive(thiserror::Error, Debug)]
enum AppError {
    //configuration is invalid
    #[error("Invalid_configuration: {0}")]
    ConfigInvalid(String),
    //our endpoint is invalid
    #[error("Invalid url: {0}")]
    UrlInvalid(Url),
    //our endpoint is wrong and returns a 404
    #[error("Url does not exist: {0}")]
    UrlNotFound(Url),
    //this is an error from trying to create the resource
    #[error("Create error: {0}")]
    CreateError(String),
    //this is for other errors that could occur
    #[error("Server error: {0}")]
    HttpError(reqwest::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::ConfigInvalid(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::CreateError(_) => StatusCode::BAD_REQUEST,
            AppError::UrlInvalid(_) | AppError::UrlNotFound(_) => StatusCode::BAD_REQUEST,
            AppError::HttpError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
        .into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        Self::HttpError(value)
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

struct AccessCode(String, u32);

pub fn get_name_and_number(el: &str) -> (String, u32) {
    let name = Regex::new(r"(?<name>\w+)").unwrap();
    let number = Regex::new(r"(?<number>\d{5})").unwrap();
    let Some(caps) = name.captures(el) else {
        //we are saying it should be nonrecoverable, should it though?
        // Ans: this should not be unrecoverable,
        panic!("No name match")
    };
    let Some(num_caps) = number.captures(el) else {
        //similarly here
        panic!("No number match")
    };
    let name = String::from(&caps["name"]);
    let number: u32 = num_caps["number"]
        .parse()
        .unwrap();
    (name, number)
}

async fn get_access_code(
    client: &Client,
    create_endpoint: &str,
    guest: &Guest,
) -> Result<AccessCode> {
    let html = client
        .post(create_endpoint)
        .form(&guest)
        .send()
        .await?
        .text()
        .await?;

    // if !res
    //    .status()
    //    .is_success()
    //{
    //    println!(
    //        "An error occured: {}",
    //        res.status()
    //            .as_str()
    //    )
    // } else {
    //   use::reqwest::Err(
    //        "Reached the create endpoint, got ok: {}",
    //        res.status()
    //            .as_str()
    //    )
    // }

    let guest_with_code = get_html_content("h2", &html);
    let _ = get_nested_html_content("h5", "p", &html);

    let (name, number) = get_name_and_number(&guest_with_code);

    Ok(AccessCode(name, number))
}

// type Result<T = ()> = std::result::Result<T, AppError>;

pub async fn app(config: AppConfig) -> Result<u32> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()?;

    // TODO: what happens if there's an error here?
    let res = client
        .get(
            config
                .endpoint
                .to_string(),
        )
        .send()
        .await?;

    if res
        .status()
        .is_success()
    {
        println!("{}", "successfully pinged")
    }

    let form_data = FormData {
        user_name: config
            .email
            .to_string(),
        user_psswrd: config.password,
    };

    let endpoint = format!("{}/{}", config.endpoint, "login");
    //TODO: what happens if there's an error here?
    let res = client
        .post(endpoint)
        .form(&form_data)
        .send()
        .await?;

    if !res
        .status()
        .is_success()
    {
        eprintln!(
            "{}",
            res.status()
                .as_str()
        )
    }

    let status = res
        .status()
        .as_str()
        .to_owned();
    println!("ok: {}", status);

    let create_endpoint = format!("{}/{}", config.endpoint, "create");
    //TODO: what happens if there's an error here
    let res = client
        .get(create_endpoint.to_string())
        .send()
        .await?;

    if res
        .status()
        .is_success()
    {
        println!(
            "Ok: {}",
            res.status()
                .as_str()
        )
    }

    let guest = Guest {
        guest_name: "Opeyemi".to_string(),
        host_address: "26, wakati adura street".to_string(),
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
        let (name, number) = get_name_and_number("Opeyemi 51402");
        assert_eq!(name, "Opeyemi");
        assert_eq!(number, 51402)
    }
}
