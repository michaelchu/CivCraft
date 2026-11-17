//! Structured errors shared by asset configuration, lookup, decoding and playback.
use std::{
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
};

pub(crate) type AssetResult<T> = Result<T, AssetError>;
#[derive(Debug)]
pub(crate) enum AssetError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Configuration {
        path: PathBuf,
        source: toml::de::Error,
    },
    Missing {
        requested: String,
        searched: Vec<PathBuf>,
    },
    Invalid {
        message: String,
    },
    Decode {
        path: PathBuf,
        message: String,
    },
    #[cfg(feature = "desktop-audio")]
    UnknownCue {
        key: String,
    },
    Context {
        message: String,
        source: Box<AssetError>,
    },
}
impl AssetError {
    pub fn io(operation: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.to_owned(),
            source,
        }
    }
    pub fn decode(path: &Path, message: impl ToString) -> Self {
        Self::Decode {
            path: path.to_owned(),
            message: message.to_string(),
        }
    }
    pub fn context(self, message: impl Into<String>) -> Self {
        Self::Context {
            message: message.into(),
            source: Box::new(self),
        }
    }
}
impl From<String> for AssetError {
    fn from(message: String) -> Self {
        Self::Invalid { message }
    }
}
impl From<&str> for AssetError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "{operation} {}: {source}", path.display()),
            Self::Configuration { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Missing {
                requested,
                searched,
            } => write!(
                f,
                "Media '{requested}' not found; searched: {}",
                searched
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Invalid { message } => f.write_str(message),
            Self::Decode { path, message } => write!(f, "Decode {}: {message}", path.display()),
            #[cfg(feature = "desktop-audio")]
            Self::UnknownCue { key } => write!(f, "Unknown audio key '{key}'"),
            Self::Context { message, source } => write!(f, "{message}: {source}"),
        }
    }
}
impl Error for AssetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Configuration { source, .. } => Some(source),
            Self::Context { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
