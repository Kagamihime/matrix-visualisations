use failure::{format_err, Error};
use serde_derive::{Deserialize, Serialize};
use yew::callback::Callback;
use yew::format::{Json, Nothing};
use yew::services::fetch::{FetchTask, Request, Response, Uri};

use crate::Model;

#[derive(Debug, Deserialize)]
pub struct JoinedRooms {
    pub joined_rooms: Vec<String>,
}

impl Model {
    pub fn list_rooms(&mut self, callback: Callback<Result<JoinedRooms, Error>>) -> FetchTask {
        let uri = format!(
            "http://{}/_matrix/client/r0/joined_rooms",
            self.session.server_name
        );

        let request = Request::get(uri)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!(
                    "Bearer {}",
                    self.session.access_token.as_ref().expect("No access token")
                ),
            )
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Json<Result<JoinedRooms, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!(
                    "{}: error listing joined rooms",
                    meta.status
                )))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    pub fn join_room(&mut self, callback: Callback<Result<(), Error>>) -> FetchTask {
        let uri = Uri::builder()
            .scheme("https")
            .authority(self.session.server_name.as_str())
            .path_and_query(
                format!("/_matrix/client/r0/rooms/{}/join", self.session.room_id).as_str(),
            )
            .build()
            .expect("Failed to build URI.");

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
                callback.emit(Err(format_err!("{}: error joining the room", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }
}
