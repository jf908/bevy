use crate::io::{AssetReader, AssetReaderError, Reader};
use crate::io::{AssetSource, PathStream};
use crate::AssetApp;
use alloc::{borrow::ToOwned, boxed::Box, string::String, vec::Vec};
use bevy_app::{App, Plugin};
use bevy_tasks::ConditionalSendFuture;
use blocking::unblock;
use std::path::{Path, PathBuf};
use url::{Origin, Url};

/// Adds the `http` and `https` asset sources to the app.
///
/// NOTE: Make sure to add this plugin *before* `AssetPlugin` to properly register http asset sources.
///
/// Any asset path that begins with `http` (when the `http` feature is enabled) or `https` (when the
/// `https` feature is enabled) will be loaded from the web via `fetch`(wasm) or `ureq`(native).
///
/// By default, `ureq`'s HTTP compression is disabled. To enable gzip and brotli decompression, add
/// the following dependency and features to your Cargo.toml. This will improve bandwidth
/// utilization when its supported by the server.
///
/// ```toml
/// [target.'cfg(not(target_family = "wasm"))'.dev-dependencies]
/// ureq = { version = "3", default-features = false, features = ["gzip", "brotli"] }
/// ```
pub struct HttpSourcePlugin {
    /// The allowed origins for HTTP(S) requests.
    pub allowed_origins: AllowedOrigins,
}

impl Plugin for HttpSourcePlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "http")]
        {
            let origins = self.allowed_origins.clone();
            let processed_origins = self.allowed_origins.clone();

            app.register_asset_source(
                "http",
                AssetSource::build()
                    .with_reader(|| {
                        Box::new(HttpSourceAssetReader {
                            secure: false,
                            allowed_origins: origins,
                        })
                    })
                    .with_processed_reader(|| {
                        Box::new(HttpSourceAssetReader {
                            secure: false,
                            allowed_origins: processed_origins,
                        })
                    }),
            );
        }

        #[cfg(feature = "https")]
        {
            let origins = self.allowed_origins.clone();
            let processed_origins = self.allowed_origins.clone();

            app.register_asset_source(
                "https",
                AssetSource::build()
                    .with_reader(move || {
                        Box::new(HttpSourceAssetReader {
                            secure: false,
                            allowed_origins: origins.clone(),
                        })
                    })
                    .with_processed_reader(move || {
                        Box::new(HttpSourceAssetReader {
                            secure: false,
                            allowed_origins: processed_origins.clone(),
                        })
                    }),
            );
        }
    }
}

#[derive(Clone)]
pub enum AllowedOrigins {
    /// Allow all origins.
    All,
    /// Allow only the specified origins.
    Only(Vec<Origin>),
}

impl AllowedOrigins {
    pub fn new(origins: impl IntoIterator<Item = String>) -> Self {
        Self::Only(
            origins
                .into_iter()
                .map(|origin| {
                    Url::parse(&origin)
                        .expect("AllowedOrigins is not properly formatted")
                        .origin()
                })
                .collect::<Vec<_>>(),
        )
    }

    fn is_allowed(&self, url: Url) -> bool {
        match self {
            AllowedOrigins::All => true,
            AllowedOrigins::Only(origins) => {
                let origin = url.origin();
                origins.iter().any(|allowed| allowed == &origin)
            }
        }
    }
}

/// Asset reader that treats paths as urls to load assets from.
#[derive(Clone)]
pub struct HttpSourceAssetReader {
    pub secure: bool,
    pub allowed_origins: AllowedOrigins,
}

impl HttpSourceAssetReader {
    fn make_uri(&self, path: &Path) -> PathBuf {
        PathBuf::from(if self.secure { "https://" } else { "http://" }).join(path)
    }

    /// See [`crate::io::get_meta_path`]
    fn make_meta_uri(&self, path: &Path) -> PathBuf {
        let meta_path = crate::io::get_meta_path(path);
        self.make_uri(&meta_path)
    }
}

#[cfg(target_arch = "wasm32")]
async fn get<'a>(path: PathBuf) -> Result<Box<dyn Reader>, AssetReaderError> {
    use crate::io::wasm::HttpWasmAssetReader;

    HttpWasmAssetReader::new("")
        .fetch_bytes(path)
        .await
        .map(|r| Box::new(r) as Box<dyn Reader>)
}

#[cfg(not(target_arch = "wasm32"))]
async fn get(path: PathBuf) -> Result<Box<dyn Reader>, AssetReaderError> {
    use crate::io::VecReader;
    use alloc::{boxed::Box, vec::Vec};
    use bevy_platform::sync::LazyLock;
    use std::io::{self, BufReader, Read};

    let str_path = path.to_str().ok_or_else(|| {
        AssetReaderError::Io(
            io::Error::other(std::format!("non-utf8 path: {}", path.display())).into(),
        )
    })?;

    #[cfg(all(not(target_arch = "wasm32"), feature = "http_source_cache"))]
    if let Some(data) = http_asset_cache::try_load_from_cache(str_path).await? {
        return Ok(Box::new(VecReader::new(data)));
    }
    use ureq::Agent;

    static AGENT: LazyLock<Agent> = LazyLock::new(|| Agent::config_builder().build().new_agent());

    let uri = str_path.to_owned();
    // Use [`unblock`] to run the http request on a separately spawned thread as to not block bevy's
    // async executor.
    let response = unblock(|| AGENT.get(uri).call()).await;

    match response {
        Ok(mut response) => {
            let mut reader = BufReader::new(response.body_mut().with_config().reader());

            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer)?;

            #[cfg(all(not(target_arch = "wasm32"), feature = "http_source_cache"))]
            http_asset_cache::save_to_cache(str_path, &buffer).await?;

            Ok(Box::new(VecReader::new(buffer)))
        }
        // ureq considers all >=400 status codes as errors
        Err(ureq::Error::StatusCode(code)) => {
            if code == 404 {
                Err(AssetReaderError::NotFound(path))
            } else {
                Err(AssetReaderError::HttpError(code))
            }
        }
        Err(err) => Err(AssetReaderError::Io(
            io::Error::other(std::format!(
                "unexpected error while loading asset {}: {}",
                path.display(),
                err
            ))
            .into(),
        )),
    }
}

impl AssetReader for HttpSourceAssetReader {
    fn read<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<dyn Reader>, AssetReaderError>> {
        return async {
            if let Some(url) = Url::parse(path.to_str().unwrap_or_default()).ok() {
                if !self.allowed_origins.is_allowed(url) {
                    return todo!("");
                }
            }

            get(self.make_uri(path)).await
        };
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader>, AssetReaderError> {
        if let Some(url) = Url::parse(path.to_str().unwrap_or_default()).ok() {
            if !self.allowed_origins.is_allowed(url) {
                return todo!("");
            }
        }

        let uri = self.make_meta_uri(path);
        get(uri).await
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(self.make_uri(path)))
    }
}

/// A naive implementation of an HTTP asset cache that never invalidates.
/// `ureq` currently does not support caching, so this is a simple workaround.
/// It should eventually be replaced by `http-cache` or similar, see [tracking issue](https://github.com/06chaynes/http-cache/issues/91)
#[cfg(all(not(target_arch = "wasm32"), feature = "http_source_cache"))]
mod http_asset_cache {
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::hash::{Hash, Hasher};
    use futures_lite::AsyncWriteExt;
    use std::collections::hash_map::DefaultHasher;
    use std::io;
    use std::path::PathBuf;

    use crate::io::Reader;

    const CACHE_DIR: &str = ".http-asset-cache";

    fn url_to_hash(url: &str) -> String {
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        std::format!("{:x}", hasher.finish())
    }

    pub async fn try_load_from_cache(url: &str) -> Result<Option<Vec<u8>>, io::Error> {
        let filename = url_to_hash(url);
        let cache_path = PathBuf::from(CACHE_DIR).join(&filename);

        if cache_path.exists() {
            let mut file = async_fs::File::open(&cache_path).await?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).await?;
            Ok(Some(buffer))
        } else {
            Ok(None)
        }
    }

    pub async fn save_to_cache(url: &str, data: &[u8]) -> Result<(), io::Error> {
        let filename = url_to_hash(url);
        let cache_path = PathBuf::from(CACHE_DIR).join(&filename);

        async_fs::create_dir_all(CACHE_DIR).await.ok();

        let mut cache_file = async_fs::File::create(&cache_path).await?;
        cache_file.write_all(data).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_http_uri() {
        assert_eq!(
            HttpSourceAssetReader::Http
                .make_uri(Path::new("example.com/favicon.png"))
                .to_str()
                .unwrap(),
            "http://example.com/favicon.png"
        );
    }

    #[test]
    fn make_https_uri() {
        assert_eq!(
            HttpSourceAssetReader {
                secure: true,
                allowed_origins: AllowedOrigins::All
            }
            .make_uri(Path::new("example.com/favicon.png"))
            .to_str()
            .unwrap(),
            "https://example.com/favicon.png"
        );
    }

    #[test]
    fn make_http_meta_uri() {
        assert_eq!(
            HttpSourceAssetReader {
                secure: true,
                allowed_origins: AllowedOrigins::All
            }
            .make_meta_uri(Path::new("example.com/favicon.png"))
            .to_str()
            .unwrap(),
            "http://example.com/favicon.png.meta"
        );
    }

    #[test]
    fn make_https_meta_uri() {
        assert_eq!(
            HttpSourceAssetReader {
                secure: true,
                allowed_origins: AllowedOrigins::All
            }
            .make_meta_uri(Path::new("example.com/favicon.png"))
            .to_str()
            .unwrap(),
            "https://example.com/favicon.png.meta"
        );
    }

    #[test]
    fn make_https_without_extension_meta_uri() {
        assert_eq!(
            HttpSourceAssetReader {
                secure: true,
                allowed_origins: AllowedOrigins::All
            }
            .make_meta_uri(Path::new("example.com/favicon"))
            .to_str()
            .unwrap(),
            "https://example.com/favicon.meta"
        );
    }
}
