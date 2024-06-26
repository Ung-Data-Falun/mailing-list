#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Error {
    InvalidCommand(Option<String>),
    Quit,
    InvalidMail,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self))
    }
}
impl std::error::Error for Error {}
