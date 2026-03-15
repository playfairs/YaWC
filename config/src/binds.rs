use core::ops::Deref;
use core::str::FromStr;
use smithay::input::keyboard::Keysym;
use smithay::input::keyboard::xkb;

#[derive(knus::Decode, Debug)]
pub struct Binds(#[knus(children)] pub Vec<Bind>);

impl Deref for Binds {
    type Target = [Bind];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for Binds {
    type Item = Bind;
    type IntoIter = std::vec::IntoIter<Bind>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Binds {
    type Item = &'a Bind;
    type IntoIter = std::slice::Iter<'a, Bind>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(knus::Decode, Debug, PartialEq)]
pub struct Bind {
    #[knus(node_name)]
    pub key_register: KeyBind,

    #[knus(children)]
    pub actions: Vec<Actions>,
}

#[derive(knus::Decode, Debug, Clone, PartialEq)]
pub enum Actions {
    Quit,
    CloseWindow,
    Spawn(#[knus(arguments)] Vec<String>),
    SpawnSh(#[knus(argument)] String),
    /// Trigger a vt-switch
    VtSwitch(#[knus(argument)] i32),
    /// Switch the current screen
    Screen(#[knus(argument)] usize),
    ScaleUp,
    ScaleDown,
    TogglePreview,
    RotateOutput,
    ToggleTint,
    ToggleDecorations,
    /// Do nothing more
    None,
}

bitflags::bitflags! {
    #[derive(Debug, PartialEq)]
    pub struct ModMask: u32 {
        const SHIFT = 1 << 0;
        const CTRL  = 1 << 1;
        const ALT   = 1 << 2;
        const SUPER = 1 << 3;
    }
}

#[derive(Debug, PartialEq)]
pub struct KeyBind {
    pub mods: ModMask,
    pub sym: Keysym,
}

impl FromStr for KeyBind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut mods = ModMask::empty();
        let mut key = None;

        for part in s.split('+') {
            match part {
                "Shift" => mods |= ModMask::SHIFT,
                "Ctrl" | "Control" => mods |= ModMask::CTRL,
                "Alt" => mods |= ModMask::ALT,
                "Mod" | "Super" | "Logo" => mods |= ModMask::SUPER,
                k => {
                    if key.is_some() {
                        return Err(format!("multiple keys in bind: {s}"));
                    }

                    let sym = xkb::keysym_from_name(k, xkb::KEYSYM_NO_FLAGS);

                    key = Some(sym);
                }
            }
        }

        Ok(KeyBind {
            mods,
            sym: key.ok_or("no key specified")?,
        })
    }
}
