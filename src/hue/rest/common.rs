use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct Owner {
    pub rid: String,
    pub rtype: String,
}
