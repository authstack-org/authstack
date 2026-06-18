use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "identity_policy", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum IdentityPolicy {
    ApplicationSilo,
    SharedDirectory,
}

impl IdentityPolicy {
    pub fn is_shared(self) -> bool {
        matches!(self, Self::SharedDirectory)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ApplicationSilo => "Application silo",
            Self::SharedDirectory => "Shared directory",
        }
    }
}

impl std::str::FromStr for IdentityPolicy {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "application_silo" => Ok(Self::ApplicationSilo),
            "shared_directory" => Ok(Self::SharedDirectory),
            _ => Err(()),
        }
    }
}
