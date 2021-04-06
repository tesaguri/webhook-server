use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use serde::{de, Deserialize};

#[derive(Deserialize)]
#[non_exhaustive]
pub struct Config {
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_bind")]
    pub bind: Option<SocketAddr>,
    #[serde(default = "default_timeout")]
    #[serde(deserialize_with = "deserialize_timeout")]
    pub timeout: Duration,
    #[serde(deserialize_with = "deserialize_hook")]
    pub hook: HashMap<Box<str>, Hook>,
}

fn default_timeout() -> Duration {
    Duration::from_secs(60)
}

#[non_exhaustive]
pub struct Hook {
    pub program: Box<str>,
    pub args: Option<Box<[Box<str>]>>,
    pub secret: Option<Box<str>>,
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
                #[serde(default)]
                secret: Option<Box<str>>,
            }
            while let Some(p) = a.next_element::<HookPrototype>()? {
                let hook = Hook {
                    program: p.program,
                    args: p.args,
                    secret: p.secret,
                };
                ret.insert(p.location, hook);
            }
            Ok(ret)
        }
    }

    d.deserialize_seq(Visitor)
}

fn deserialize_timeout<'de, D>(d: D) -> Result<Duration, D::Error>
where
    D: de::Deserializer<'de>,
{
    u64::deserialize(d).map(Duration::from_secs)
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
