#[derive(Clone, Debug)]
pub struct Session {
    pub server_name: String,

    pub username: String,
    pub user_id: String,
    pub password: String,
    pub access_token: String,

    pub device_id: String,
    pub room_id: String,
    pub filter_id: String,
    pub next_batch_token: String,
    pub prev_batch_token: String,
}

impl Session {
    pub fn empty() -> Self {
        Session {
            server_name: String::new(),

            username: String::new(),
            user_id: String::new(),
            password: String::new(),
            access_token: String::new(),

            device_id: String::new(),
            room_id: String::new(),
            filter_id: String::new(),
            next_batch_token: String::new(),
            prev_batch_token: String::new(),
        }
    }
}
