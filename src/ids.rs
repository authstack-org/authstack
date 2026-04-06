use std::str::FromStr;

use type_safe_id::{StaticType, TypeSafeId};

/// Macro that produces a newtype ID wrapper with serde + sqlx (TEXT) support.
macro_rules! define_id {
    ($name:ident, $marker:ident, $prefix:literal) => {
        #[derive(Default, Clone, Copy, PartialEq, Eq)]
        pub struct $marker;

        impl StaticType for $marker {
            const TYPE: &'static str = $prefix;
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub TypeSafeId<$marker>);

        impl $name {
            pub fn new() -> Self {
                Self(TypeSafeId::new())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = type_safe_id::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                TypeSafeId::from_str(s).map($name)
            }
        }

        // ── sqlx: store/load as TEXT ─────────────────────────────────────────

        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <String as sqlx::Type<sqlx::Postgres>>::type_info()
            }
            fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
                <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <String as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(
                    &self.0.to_string(),
                    buf,
                )
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
                s.parse()
                    .map_err(|e: type_safe_id::Error| Box::new(e) as sqlx::error::BoxDynError)
            }
        }
    };
}

define_id!(ApplicationId, ApplicationType, "app");
define_id!(UserId, UserType, "usr");
define_id!(OrganizationId, OrganizationType, "org");
define_id!(MemberId, MemberType, "mbr");
define_id!(AccountId, AccountType, "acct");
define_id!(RefreshSessionId, RefreshSessionType, "rsess");
define_id!(AdminUserId, AdminUserType, "adm");
