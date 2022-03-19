// TODO: ~~user creation endpoint~~
// TODO: ~~refactor API crate to only accept local and remove auth~~
// TODO: ~~API crate should use shared secret for its control plane~~
// TODO: ~~API crate should expose the active deployed port for a service~~
// TODO: ~~gateway crate should poll active deployment port for proxy~~
// TODO: ~~gateway crate should rewrite the projects -> services route~~
// TODO: client should create project then push new deployment (refactor endpoint)
// TODO: ~~rename API crate~~
// TODO: move common things to the common crate
// TODO: AccountName and ProjectName validation logic?

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate log;
extern crate core;

use std::error::Error as StdError;
use std::fmt::Formatter;
use std::io;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{
    IntoResponse,
    Response
};
use bollard::Docker;
use serde::{
    Deserialize,
    Deserializer,
    Serialize
};
use serde_json::json;

use crate::api::make_api;
use crate::proxy::serve_proxy;
use crate::service::GatewayService;

pub mod api;
pub mod project;
pub mod proxy;
pub mod service;
pub mod auth;
pub mod client;

pub const API_PORT: &'static str = "8001";
pub const CLIENT_PORT: &'static str = "8000";

#[derive(Debug)]
pub enum ErrorKind {
    Missing,
    Malformed,
    Unauthorized,
    Internal,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn StdError>>
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self.kind {
            ErrorKind::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            ErrorKind::Missing => (StatusCode::BAD_REQUEST, "Missing an API key"),
            ErrorKind::Malformed => (StatusCode::BAD_REQUEST, "Invalid API key"),
            ErrorKind::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized")
        };
        (status, Json(json!({ "error": error_message }))).into_response()
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl StdError for Error {}

#[derive(Debug, sqlx::Type, Serialize, Clone, PartialEq, Eq)]
#[sqlx(transparent)]
pub struct ProjectName(pub String);

impl<'de> Deserialize<'de> for ProjectName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|_err| todo!())
    }
}

impl FromStr for ProjectName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: re correct?
        let re = regex::Regex::new("^[a-zA-Z0-9-_]{3,64}$").unwrap();
        if re.is_match(s) {
            Ok(Self(s.to_string()))
        } else {
            todo!()
        }
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, sqlx::Type, Serialize)]
#[sqlx(transparent)]
pub struct AccountName(pub String);

impl FromStr for AccountName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl std::fmt::Display for AccountName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for AccountName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|_err| todo!())
    }
}

pub trait Context<'c>: Send + Sync {
    fn docker(&self) -> &'c Docker;
    fn client(&self, target: IpAddr) -> client::Client;
}

/// Assumes that state matches world at init and service is only source of mutation to world
#[async_trait]
pub trait State<'c> {
    type Next: State<'c>;

    type Error: StdError;

    async fn next<C: Context<'c>>(self, ctx: &C) -> Result<Self::Next, Self::Error>;
}

// TODO may not want to have Refresh for variants of Project as this may drift OOS
#[async_trait]
pub trait Refresh: Sized {
    type Error: StdError;
    async fn refresh<'c, C: Context<'c>>(self, ctx: &C) -> Result<Self, Self::Error>;
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let gateway = GatewayService::init().await;

    let api = make_api(Arc::clone(&gateway));
    let api_handle = tokio::spawn(
        axum::Server::bind(&format!("0.0.0.0:{}", API_PORT).parse().unwrap())
            .serve(api.into_make_service())
    );

    let proxy_handle = tokio::spawn(serve_proxy(gateway));

    tokio::join!(api_handle, proxy_handle);

    Ok(())
}