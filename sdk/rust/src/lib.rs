pub struct Client {
    pub host: String,
}

impl Client {
    pub fn new(host: &str) -> Self {
        Self {
            host: host.to_string(),
        }
    }

    pub fn test() {
        println!("test");
    }
}
