pub trait Version<'a> {
    fn as_str(&self) -> &'a str;
}

pub struct V1;
pub struct V1_1;
pub struct UNSPECIFIED;
pub struct Dynamic<'a>(&'a str);

impl<'a> Version<'a> for V1 {
    fn as_str(&self) -> &'a str {
        "1.0"
    }
}

impl<'a> Version<'a> for V1_1 {
    fn as_str(&self) -> &'a str {
        "1.1"
    }
}

impl<'a> Version<'a> for UNSPECIFIED {
    fn as_str(&self) -> &'a str {
        ""
    }
}

impl<'a> Version<'a> for Dynamic<'a> {
    fn as_str(&self) -> &'a str {
        self.0
    }
}
