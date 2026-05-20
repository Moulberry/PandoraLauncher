use std::{collections::VecDeque, sync::Arc};

use gpui::{App, AppContext, Context, Entity, RenderImage, Task};
use rustc_hash::FxHashMap;
use schema::{minecraft_profile::SkinVariant, unique_bytes::UniqueBytes};

const THUMB_YAW: f32 = 0.3;
const THUMB_BACK_YAW: f32 = std::f32::consts::PI + THUMB_YAW;
const THUMB_PITCH: f32 = 0.05;
pub const THUMB_WIDTH: u32 = 128;
pub const THUMB_HEIGHT: u32 = 128;

const MAX_CACHE_SIZE: usize = 256;

enum ThumbnailState {
    Pending,
    Ready(Arc<RenderImage>, Arc<RenderImage>), // (front, back)
}

struct QueueEntry {
    skin: UniqueBytes,
    variant: SkinVariant,
}

#[derive(Clone, Eq, PartialEq, Hash)]
struct CapeThumbnailKey {
    skin: UniqueBytes,
    cape_url: Arc<str>,
}

struct CapeQueueEntry {
    key: CapeThumbnailKey,
    variant: SkinVariant,
    cape: UniqueBytes,
}

pub struct SkinThumbnailCache {
    cache: FxHashMap<UniqueBytes, ThumbnailState>,
    cape_cache: FxHashMap<CapeThumbnailKey, ThumbnailState>,
    insertion_order: VecDeque<UniqueBytes>,
    cape_insertion_order: VecDeque<CapeThumbnailKey>,
    queue: VecDeque<QueueEntry>,
    cape_queue: VecDeque<CapeQueueEntry>,
    render_task: Option<Task<()>>,
    cape_render_task: Option<Task<()>>,
}

impl SkinThumbnailCache {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|_| Self {
            cache: FxHashMap::default(),
            cape_cache: FxHashMap::default(),
            insertion_order: VecDeque::new(),
            cape_insertion_order: VecDeque::new(),
            queue: VecDeque::new(),
            cape_queue: VecDeque::new(),
            render_task: None,
            cape_render_task: None,
        });
        cx.observe_release(&entity, |this, cx| {
            for (_, state) in this.cache.drain() {
                if let ThumbnailState::Ready(front, back) = state {
                    cx.drop_image(front, None);
                    cx.drop_image(back, None);
                }
            }
            for (_, state) in this.cape_cache.drain() {
                if let ThumbnailState::Ready(front, back) = state {
                    cx.drop_image(front, None);
                    cx.drop_image(back, None);
                }
            }
        })
        .detach();
        entity
    }

    pub fn get_or_queue(
        &mut self,
        skin: &UniqueBytes,
        variant: SkinVariant,
        cx: &mut Context<Self>,
    ) -> Option<(Arc<RenderImage>, Arc<RenderImage>)> {
        match self.cache.get(skin) {
            Some(ThumbnailState::Ready(front, back)) => return Some((front.clone(), back.clone())),
            Some(ThumbnailState::Pending) => return None,
            None => {},
        }

        self.cache.insert(skin.clone(), ThumbnailState::Pending);
        self.queue.push_back(QueueEntry {
            skin: skin.clone(),
            variant,
        });
        self.maybe_start_render(cx);
        None
    }

    pub fn get_cape_or_queue(
        &mut self,
        skin: &UniqueBytes,
        variant: SkinVariant,
        cape_url: Arc<str>,
        cape: &UniqueBytes,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let key = CapeThumbnailKey {
            skin: skin.clone(),
            cape_url,
        };

        match self.cape_cache.get(&key) {
            Some(ThumbnailState::Ready(front, _back)) => return Some(front.clone()),
            Some(ThumbnailState::Pending) => return None,
            None => {},
        }

        self.cape_cache.insert(key.clone(), ThumbnailState::Pending);
        self.cape_queue.push_back(CapeQueueEntry {
            key,
            variant,
            cape: cape.clone(),
        });
        self.maybe_start_cape_render(cx);
        None
    }

    fn maybe_start_render(&mut self, cx: &mut Context<Self>) {
        if self.render_task.is_some() {
            return;
        }
        let Some(entry) = self.queue.pop_front() else {
            return;
        };

        let (tx, rx) = tokio::sync::oneshot::channel::<Option<(Arc<RenderImage>, Arc<RenderImage>)>>();

        cx.background_executor()
            .spawn({
                let skin = entry.skin.clone();
                async move {
                    let renderer = crate::component::skin_renderer::SkinRenderer::new(
                        Some(Arc::from(skin.to_vec().into_boxed_slice())),
                        entry.variant == SkinVariant::Slim,
                    );
                    let front =
                        renderer.render_to_buffer_with_params(THUMB_WIDTH, THUMB_HEIGHT, THUMB_YAW, THUMB_PITCH, true);
                    let back = renderer.render_to_buffer_with_params(
                        THUMB_WIDTH,
                        THUMB_HEIGHT,
                        THUMB_BACK_YAW,
                        THUMB_PITCH,
                        true,
                    );
                    let _ = tx.send(front.zip(back));
                }
            })
            .detach();

        self.render_task = Some(cx.spawn(async move |this, cx| {
            let result = rx.await;

            let _ = this.update(cx, |cache, cx| {
                if let Ok(Some((front, back))) = result {
                    cache.cache.insert(entry.skin.clone(), ThumbnailState::Ready(front, back));
                    cache.insertion_order.push_back(entry.skin.clone());

                    while cache.insertion_order.len() > MAX_CACHE_SIZE {
                        if let Some(oldest) = cache.insertion_order.pop_front() {
                            if let Some(ThumbnailState::Ready(front, back)) = cache.cache.remove(&oldest) {
                                cx.drop_image(front, None);
                                cx.drop_image(back, None);
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

    fn maybe_start_cape_render(&mut self, cx: &mut Context<Self>) {
        if self.cape_render_task.is_some() {
            return;
        }
        let Some(entry) = self.cape_queue.pop_front() else {
            return;
        };

        let (tx, rx) = tokio::sync::oneshot::channel::<Option<Arc<RenderImage>>>();

        cx.background_executor()
            .spawn({
                let skin = entry.key.skin.clone();
                let cape = entry.cape.clone();
                async move {
                    let mut renderer = crate::component::skin_renderer::SkinRenderer::new(
                        Some(Arc::from(skin.to_vec().into_boxed_slice())),
                        entry.variant == SkinVariant::Slim,
                    );
                    renderer.update_cape(Some(Arc::from(cape.to_vec().into_boxed_slice())));
                    let result = renderer.render_to_buffer_with_params_ext(
                        THUMB_WIDTH,
                        THUMB_HEIGHT,
                        std::f32::consts::PI + THUMB_YAW,
                        THUMB_PITCH,
                        true,
                        true,
                    );
                    let _ = tx.send(result);
                }
            })
            .detach();

        self.cape_render_task = Some(cx.spawn(async move |this, cx| {
            let result = rx.await;

            let _ = this.update(cx, |cache, cx| {
                if let Ok(Some(img)) = result {
                    // Cape thumbnails only need the back view (cape is always shown from behind)
                    // Store a clone as the "front" slot so ThumbnailState is satisfied
                    cache.cape_cache.insert(entry.key.clone(), ThumbnailState::Ready(img.clone(), img));
                    cache.cape_insertion_order.push_back(entry.key.clone());

                    while cache.cape_insertion_order.len() > MAX_CACHE_SIZE {
                        if let Some(oldest) = cache.cape_insertion_order.pop_front() {
                            if let Some(ThumbnailState::Ready(front, back)) = cache.cape_cache.remove(&oldest) {
                                cx.drop_image(front, None);
                                cx.drop_image(back, None);
                            }
                        }
                    }
                } else {
                    cache.cape_cache.remove(&entry.key);
                }

                cache.cape_render_task = None;
                cache.maybe_start_cape_render(cx);
                cx.notify();
            });
        }));
    }
}
