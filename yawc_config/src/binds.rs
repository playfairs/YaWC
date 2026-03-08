#[derive(knuffel::Decode, Debug, Default)]
pub struct Binds {
    #[knuffel(children)]
    pub binds: Vec<Bind>,
}

#[derive(knuffel::Decode, Debug, PartialEq, Default)]
pub struct Bind {
    #[knuffel(argument)]
    pub something: String,
}
