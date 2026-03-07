#[derive(knuffel::Decode, Debug, Default, PartialEq)]
pub struct Envs {
    #[knuffel(children)]
    pub vars: Vec<EnvVar>,
}

#[derive(knuffel::Decode, Debug, PartialEq)]
pub struct EnvVar {
    #[knuffel(node_name)]
    pub name: String,

    #[knuffel(argument)]
    pub value: String,
}
