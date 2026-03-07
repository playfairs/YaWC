#[derive(knuffel::Decode, Debug, Default)]
pub struct Binds {
    #[knuffel(children(name = "bind"))]
    pub register: Vec<Bind>,
}

#[derive(knuffel::Decode, Debug, Default)]
pub struct Bind {
    #[knuffel(property)]
    pub feild: String,
}
