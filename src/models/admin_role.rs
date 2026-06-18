use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminRole {
    InstanceAdmin,
    AppAdmin,
}

impl AdminRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InstanceAdmin => "instance_admin",
            Self::AppAdmin => "app_admin",
        }
    }
}

impl FromStr for AdminRole {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "instance_admin" => Ok(Self::InstanceAdmin),
            "app_admin" => Ok(Self::AppAdmin),
            _ => Err(()),
        }
    }
}
