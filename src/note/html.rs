use serde::{Deserialize, Serialize};
use std::{convert::Infallible, fmt, str::FromStr};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(transparent)]
pub struct Html(String);

impl FromStr for Html {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Html(s.to_owned()))
    }
}

impl fmt::Display for Html {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
