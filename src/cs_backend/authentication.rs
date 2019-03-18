use failure::{format_err, Error};
use serde_derive::{Deserialize, Serialize};
use yew::callback::Callback;
use yew::format::{Json, Nothing};
use yew::services::fetch::{FetchTask, Request, Response};

use crate::Model;

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectionRequest {
    #[serde(rename = "type")]
    typo: String,
    identifier: Identifier,
    password: String,
    initial_device_display_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Identifier {
    #[serde(rename = "type")]
    typo: String,
    user: String,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionResponse {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
}

impl Model {
    pub fn connect(&mut self, callback: Callback<Result<ConnectionResponse, Error>>) -> FetchTask {
        let body = ConnectionRequest {
            typo: String::from("m.login.password"),
            identifier: Identifier {
                typo: String::from("m.id.user"),
                user: self.session.username.clone(),
            },
            password: self.session.password.clone(),
            initial_device_display_name: String::from("Matrix visualisations"),
        };

        let uri = format!(
            "https://{}/_matrix/client/r0/login",
            self.session.server_name
        );

        let request = Request::post(uri)
            .header("Content-Type", "application/json")
            .body(Json(&body))
            .expect("Failed to build request.");

        let handler = move |response: Response<Json<Result<ConnectionResponse, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!("{}: error connecting", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    pub fn disconnect(&mut self, callback: Callback<Result<(), Error>>) -> FetchTask {
        let uri = format!(
            "https://{}/_matrix/client/r0/logout",
            self.session.server_name
        );

        let request = Request::post(uri)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.session.access_token.as_ref().unwrap()),
            )
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Nothing>| {
            let (meta, _) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(Ok(()))
            } else {
                callback.emit(Err(format_err!("{}: error connecting", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }
}
