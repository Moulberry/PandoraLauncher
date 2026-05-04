use std::{collections::VecDeque, sync::Arc};

use gpui::{App, AppContext, Context, Entity, RenderImage, Task};
use image::{Frame, RgbaImage as SkinImage};
use rustc_hash::FxHashMap;
use schema::{minecraft_profile::SkinVariant, unique_bytes::UniqueBytes};

const THUMB_YAW: f64 = 22.5;
const THUMB_PITCH: f64 = -8.0;
const THUMB_ANIMATION: f64 = 3.0/16.0;
const THUMB_Y_OFFSET: f64 = 6.0;
const THUMB_ZOOM: f64 = 1.4;
pub const THUMB_WIDTH: u32 = 128;
pub const THUMB_HEIGHT: u32 = 128;

const MAX_CACHE_SIZE: usize = 256;

enum ThumbnailState {
    Pending,
    Ready(Arc<RenderImage>),
}

struct QueueEntry {
    skin: UniqueBytes,
    variant: SkinVariant,
}

pub struct SkinThumbnailCache {
    cache: FxHashMap<UniqueBytes, ThumbnailState>,
    insertion_order: VecDeque<UniqueBytes>,
    queue: VecDeque<QueueEntry>,
    render_task: Option<Task<()>>,
}

impl SkinThumbnailCache {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|_| Self {
            cache: FxHashMap::default(),
            insertion_order: VecDeque::new(),
            queue: VecDeque::new(),
            render_task: None,
        });
        cx.observe_release(&entity, |this, cx| {
            for (_, state) in this.cache.drain() {
                if let ThumbnailState::Ready(image) = state {
                    cx.drop_image(image, None);
                }
            }
        }).detach();
        entity
    }

    pub fn get_or_queue(
        &mut self,
        skin: &UniqueBytes,
        variant: SkinVariant,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        match self.cache.get(skin) {
            Some(ThumbnailState::Ready(img)) => return Some(img.clone()),
            Some(ThumbnailState::Pending) => return None,
            None => {}
        }

        self.cache.insert(skin.clone(), ThumbnailState::Pending);
        self.queue.push_back(QueueEntry {
            skin: skin.clone(),
            variant,
        });
        self.maybe_start_render(cx);
        None
    }

    fn maybe_start_render(&mut self, cx: &mut Context<Self>) {
        if self.render_task.is_some() {
            return;
        }
        let Some(entry) = self.queue.pop_front() else {
            return;
        };

        let (tx, rx) = tokio::sync::oneshot::channel::<Option<SkinImage>>();

        cx.background_executor().spawn({
            let skin = entry.skin.clone();
            async move {
                let result = crate::skin_renderer::render_skin_3d(
                    &skin,
                    None,
                    entry.variant,
                    THUMB_WIDTH,
                    THUMB_HEIGHT,
                    THUMB_YAW,
                    THUMB_PITCH,
                    THUMB_ANIMATION,
                    THUMB_Y_OFFSET,
                    THUMB_ZOOM,
                );
                let result = result.map(|mut img| {
                    for px in img.chunks_exact_mut(4) {
                        px.swap(0, 2);
                    }
                    img
                });
                let _ = tx.send(result);
            }
        }).detach();

        self.render_task = Some(cx.spawn(async move |this, cx| {
            let result = rx.await;

            let _ = this.update(cx, |cache, cx| {
                if let Ok(Some(img)) = result {
                    let render_image = Arc::new(RenderImage::new([Frame::new(img)]));
                    cache.cache.insert(entry.skin.clone(), ThumbnailState::Ready(render_image));
                    cache.insertion_order.push_back(entry.skin.clone());

                    while cache.insertion_order.len() > MAX_CACHE_SIZE {
                        if let Some(oldest) = cache.insertion_order.pop_front() {
                            if let Some(ThumbnailState::Ready(image)) = cache.cache.remove(&oldest) {
                                cx.drop_image(image, None);
                            }
                        }
                    }
                } else {
                    cache.cache.remove(&entry.skin);
                }

                cache.render_task = None;
                cache.maybe_start_render(cx);
                cx.notify();
            });
        }));
    }
}
