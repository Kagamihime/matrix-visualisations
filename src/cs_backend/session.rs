/// Holds every informations allowing the application to communicate with the homeserver and
/// retrieve the events of the room to observe.
#[derive(Clone, Debug)]
pub struct Session {
    pub server_name: String,
    pub room_id: String,

    pub username: String,
    pub user_id: String,
    pub password: String,
    pub access_token: Option<String>,

    pub device_id: Option<String>,
    pub filter_id: Option<String>,
    pub next_batch_token: Option<String>,
    pub prev_batch_token: Option<String>,
}

impl Session {
    pub fn empty() -> Self {
        Session {
            server_name: String::new(),
            room_id: String::new(),

            username: String::new(),
            user_id: String::new(),
            password: String::new(),

            access_token: None,

            device_id: None,
            filter_id: None,
            next_batch_token: None,
            prev_batch_token: None,
        }
    }
}
