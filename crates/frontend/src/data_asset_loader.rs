use std::sync::Arc;

use futures::AsyncReadExt;
use gpui::{App, Asset, Resource};

#[derive(Clone)]
pub enum DataAssetLoader {}

impl Asset for DataAssetLoader {
    type Source = Resource;
    type Output = Option<Arc<[u8]>>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = cx.http_client();
        let asset_source = cx.asset_source().clone();
        async move {
            match source.clone() {
                Resource::Path(uri) => Some(std::fs::read(uri.as_ref()).ok()?.into()),
                Resource::Uri(uri) => {
                    let mut response = client
                        .get(uri.as_ref(), ().into(), true)
                        .await.ok()?;
                    if !response.status().is_success() {
                        return None;
                    }
                    let mut body = Vec::new();
                    response.body_mut().read_to_end(&mut body).await.ok()?;
                    Some(body.into())
                }
                Resource::Embedded(path) => {
                    Some(asset_source.load(&path).ok().flatten()?.into())
                }
            }
        }
    }
}
