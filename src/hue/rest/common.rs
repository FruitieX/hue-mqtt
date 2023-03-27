use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Owner {
    pub rid: String,
    pub rtype: String,
}
