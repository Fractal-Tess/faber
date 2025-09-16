use crate::utils::generate_random_string;

pub struct Container {
    id: String,
}

impl Container {
    pub fn new() -> Self {
        let id = generate_random_string(12);
        Self { id }
    }
}
