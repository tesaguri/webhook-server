use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU64;

use serde::{de, Deserialize};

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_bind")]
    pub bind: Option<SocketAddr>,
    #[serde(default = "default_timeout")]
    pub timeout: Option<NonZeroU64>,
    #[serde(deserialize_with = "deserialize_hook")]
    pub hook: HashMap<Box<str>, Hook>,
}

fn default_timeout() -> Option<NonZeroU64> {
    NonZeroU64::new(60)
}

pub struct Hook {
    pub program: Box<str>,
    pub args: Option<Box<[Box<str>]>>,
}

pub(crate) struct DisplayHookCommand<'a>(pub &'a Hook);

fn deserialize_bind<'de, D>(d: D) -> Result<Option<SocketAddr>, D::Error>
where
    D: de::Deserializer<'de>,
{
    Option::<(IpAddr, u16)>::deserialize(d).map(|option| option.map(Into::into))
}

fn deserialize_hook<'de, D>(d: D) -> Result<HashMap<Box<str>, Hook>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = HashMap<Box<str>, Hook>;

        fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("a sequence")
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
            let mut ret = HashMap::with_capacity(a.size_hint().unwrap_or(0));
            #[derive(Deserialize)]
            struct HookPrototype {
                location: Box<str>,
                program: Box<str>,
                #[serde(default)]
                args: Option<Box<[Box<str>]>>,
            }
            while let Some(HookPrototype {
                location,
                program,
                args,
            }) = a.next_element()?
            {
                ret.insert(location, Hook { program, args });
            }
            Ok(ret)
        }
    }

    d.deserialize_seq(Visitor)
}

impl<'a> Display for DisplayHookCommand<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.program)?;
        for arg in self.0.args.as_deref().into_iter().flatten() {
            f.write_str(" ")?;
            f.write_str(arg)?;
        }
        Ok(())
    }
}
