use super::Html;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, fmt, str::FromStr};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(transparent)]
pub struct Markdown(String);

impl Markdown {
    pub fn to_html(&self) -> Html {
        conv::md_to_html(&self.0).parse().unwrap()
    }

    pub fn update_local_link(&mut self, external_link: &str, local_link: &str) -> &mut Self {
        self.0 = self.0.replace(
            &format!("+{external_link}"),
            &format!("{external_link} ([local copy]({local_link}))"),
        );
        self
    }
}

impl FromStr for Markdown {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Markdown(s.to_owned()))
    }
}

impl fmt::Display for Markdown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

mod conv {

    pub fn md_to_html(md: &str) -> String {
        let mut options = Options::default();
        options.extension.strikethrough = true;
        options.extension.tagfilter = true;
        options.extension.table = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
        options.extension.superscript = true;
        options.render.unsafe_ = true;
        let html = markdown_to_html(md, &options);
        html.parse().unwrap()
    }

    use comrak::{markdown_to_html, Options};
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_update_local_link() {
        assert_eq!(
            Markdown("+http://foo.bar".to_owned())
                .update_local_link("http://foo.bar", "foo/bar")
                .0,
            "http://foo.bar ([local copy](foo/bar))"
        );

        assert_eq!(
            Markdown("+http://foo.bar".to_owned())
                .update_local_link("http://bar.baz", "bar/baz")
                .0,
            "+http://foo.bar"
        );
    }

    use super::*;
}
