#[derive(Debug, Default, PartialEq)]
pub struct Envs(pub Vec<Env>);

/// Type for a ENV
/// e.g: name:  "GTK_THEME"
///      value: "rose-pine"
#[derive(Debug, Clone, PartialEq)]
pub struct Env {
    name: String,
    value: String,
}
