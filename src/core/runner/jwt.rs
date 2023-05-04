#[derive(Clone)]
pub enum Signature {
    Hmac { secret: Vec<u8> },
    None,
}

#[derive(Clone)]
pub struct Jwt {
    header: String,
    payload: String,
    cmd_arg_target: String,
}
