#[derive(Clone, Debug)]
pub struct Session {
    pub server_name: String,
    pub room_id: String,
}

impl Session {
    pub fn empty() -> Self {
        Session {
            server_name: String::new(),
            room_id: String::new(),
        }
    }
}
