#[derive(knuffel::Decode, Debug)]
pub struct Envs {
    #[knuffel(children)]
    pub vars: Vec<Env>,
}

#[derive(knuffel::Decode, Debug, PartialEq)]
pub struct Env {
    #[knuffel(node_name)]
    pub name: String,
    #[knuffel(argument)]
    pub value: String,
}
