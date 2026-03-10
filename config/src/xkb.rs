#[derive(knuffel::Decode, Debug)]
pub struct RawXkb {
    #[knuffel(child, unwrap(argument))]
    pub layout: Option<String>,
    #[knuffel(child, unwrap(argument))]
    pub variant: Option<String>,
    #[knuffel(child, unwrap(argument))]
    pub options: Option<String>,
    #[knuffel(child, unwrap(argument))]
    pub repeat_rate: Option<String>,
    #[knuffel(child, unwrap(argument))]
    pub repeat_delay: Option<String>,
}

#[derive(Debug)]
pub struct Xkb {
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
    pub repeat_rate: i32,
    pub repeat_delay: i32,
}

impl From<RawXkb> for Xkb {
    fn from(raw: RawXkb) -> Self {
        Self {
            layout: raw.layout.unwrap_or_else(|| "us".into()),
            variant: raw.variant.unwrap_or_else(|| "".into()),
            options: raw.options,
            repeat_rate: raw
                .repeat_rate
                .unwrap_or_else(|| "200".into())
                .parse::<i32>()
                .expect("repeat_rate is meant to represent an i32"),
            repeat_delay: raw
                .repeat_delay
                .unwrap_or_else(|| "50".into())
                .parse::<i32>()
                .expect("repeat_delay is meant to represent an i32"),
        }
    }
}
