use core::ops::Deref;

#[derive(knuffel::Decode, Debug)]
pub struct Envs(#[knuffel(children)] pub Vec<Env>);

impl Deref for Envs {
    type Target = [Env];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for Envs {
    type Item = Env;
    type IntoIter = std::vec::IntoIter<Env>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Envs {
    type Item = &'a Env;
    type IntoIter = std::slice::Iter<'a, Env>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(knuffel::Decode, Debug, PartialEq)]
pub struct Env {
    #[knuffel(node_name)]
    pub name: String,
    #[knuffel(argument)]
    pub value: String,
}
