//! Horizon uses libnx rather than a desktop window server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;
impl DeviceId { pub const fn dummy() -> Self { Self } }
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyEventExtra;
pub type OsError = std::io::Error;
