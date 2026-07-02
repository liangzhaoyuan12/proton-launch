#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn from_str(s: &str) -> Self {
        match s {
            "en" => Lang::En,
            _ => Lang::Zh,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }
}

macro_rules! t {
    ($lang:expr, $zh:expr, $en:expr) => {
        match $lang {
            Lang::Zh => $zh,
            Lang::En => $en,
        }
    };
}

pub(crate) use t;
