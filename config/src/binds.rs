use core::str::FromStr;
use smithay::input::keyboard::Keysym;
use smithay::input::keyboard::xkb;

#[derive(knuffel::Decode, Debug)]
pub struct Binds {
    #[knuffel(children)]
    pub binds: Vec<Bind>,
}

#[derive(knuffel::Decode, Debug, PartialEq)]
pub struct Bind {
    #[knuffel(node_name)]
    pub key_register: KeyBind,

    #[knuffel(children)]
    pub actions: Vec<Actions>,
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq)]
pub enum Actions {
    Quit,
    CloseWindow,
    Spawn(#[knuffel(arguments)] Vec<String>),
    SpawnSh(#[knuffel(argument)] String),
    /// Trigger a vt-switch
    VtSwitch(#[knuffel(argument)] i32),
    /// Switch the current screen
    Screen(#[knuffel(argument)] usize),
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
