use std::{
    rc::Rc,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use atomic_time::AtomicInstant;
use gpui::{App, RenderImage};
use image::{imageops::FilterType, Frame};
use intrusive_collections::{LinkedList, LinkedListLink, intrusive_adapter};
use rustc_hash::FxHashMap;
use schema::unique_bytes::UniqueBytes;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum ImageTransformation {
    None,
    Resize {
        width: u32,
        height: u32,
    },
    ResizeToWidth {
        width: u32,
    },
    CropAndScale {
        min_x: u32,
        min_y: u32,
        width: u32,
        height: u32,
        scale: u32,
    }
}

struct CacheEntry {
    link: LinkedListLink,
    source: UniqueBytes,
    transform: ImageTransformation,
    expiring: AtomicInstant,
    value: Option<Arc<RenderImage>>,
}

intrusive_adapter!(CacheEntryAdapter = Rc<CacheEntry>: CacheEntry { link: LinkedListLink });

#[derive(Default)]
struct PngRenderCache {
    map: FxHashMap<(UniqueBytes, ImageTransformation), Rc<CacheEntry>>,
    expiring: LinkedList<CacheEntryAdapter>,
    submitted_cleanup: bool,
}

impl gpui::Global for PngRenderCache {}

const EXPIRY_SECONDS: u64 = 1;

pub fn render(image: UniqueBytes, cx: &mut App) -> gpui::Img {
    render_with_transform(image, ImageTransformation::None, cx)
}

pub fn render_with_transform(image: UniqueBytes, transform: ImageTransformation, cx: &mut App) -> gpui::Img {
    let cache = cx.default_global::<PngRenderCache>();

    let result = if let Some(result) = cache.get_or_create(image, transform) {
        gpui::img(result)
    } else {
        gpui::img(gpui::ImageSource::Resource(gpui::Resource::Embedded("images/missing.png".into())))
    };

    if !cache.submitted_cleanup {
        cache.submitted_cleanup = true;
        cx.spawn(async |cx| {
            let _ = cx.update_global(|cache: &mut PngRenderCache, cx| {
                let now = Instant::now();
                let mut cursor = cache.expiring.front_mut();
                while let Some(entry) = cursor.get() {
                    if now > entry.expiring.load(Ordering::Relaxed) {
                        let entry = cursor.remove().expect("present");
                        cache.map.remove(&(entry.source.clone(), entry.transform)).expect("present");

                        debug_assert_eq!(Rc::strong_count(&entry), 1);

                        if let Some(image) = &entry.value {
                            cx.drop_image(image.clone(), None);
                        }
                    } else {
                        break;
                    }
                }
                cache.submitted_cleanup = false;
            });
        }).detach();
    }

    result
}

impl PngRenderCache {
    fn get_or_create(&mut self, image: UniqueBytes, transform: ImageTransformation) -> Option<Arc<RenderImage>> {
        let key = (image, transform);

        if let Some(result) = self.map.get(&key) {
            // Update expiry
            result.expiring.store(Instant::now() + Duration::from_secs(EXPIRY_SECONDS), Ordering::Relaxed);
            unsafe {
                self.expiring.cursor_mut_from_ptr(Rc::as_ptr(result)).remove();
            }
            self.expiring.push_back(result.clone());

            return result.value.clone();
        }

        let result = image::load_from_memory_with_format(&key.0, image::ImageFormat::Png).map(|mut image| {
            match transform {
                ImageTransformation::None => {},
                ImageTransformation::Resize { width, height } => {
                    let old_width = image.width();
                    let old_height = image.height();
                    if old_width != width || old_height != height {
                        let filter = if old_width > width || old_height > height {
                            FilterType::Lanczos3
                        } else {
                            FilterType::Nearest
                        };
                        image = image.resize(width, height, filter);
                    }
                },
                ImageTransformation::ResizeToWidth { width } => {
                    let old_width = image.width();
                    let old_height = image.height();
                    let ratio = width as f64 / old_width as f64;
                    let height = ((old_height as f64 * ratio).round() as u32).max(1);
                    if old_width != width || old_height != height {
                        let filter = if old_width > width || old_height > height {
                            FilterType::Lanczos3
                        } else {
                            FilterType::Nearest
                        };
                        image = image.resize_exact(width, height, filter);
                    }
                },
                ImageTransformation::CropAndScale { min_x, min_y, width, height, scale } => {
                    let cropped = image.crop_imm(min_x, min_y, width, height);
                    image = cropped.resize_exact(width*scale, height*scale, FilterType::Nearest);
                }
            }

            let mut data = image.into_rgba8();

            // Convert from RGBA to BGRA.
            for pixel in data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }

            RenderImage::new([Frame::new(data)])
        });

        let render_image = match result {
            Ok(render_image) => Some(Arc::new(render_image)),
            Err(error) => {
                log::warn!("Error loading png: {error:?}");
                None
            },
        };

        let entry = Rc::new(CacheEntry {
            link: LinkedListLink::new(),
            source: key.0.clone(),
            transform,
            expiring: AtomicInstant::new(Instant::now() + Duration::from_secs(EXPIRY_SECONDS)),
            value: render_image.clone(),
        });
        self.map.insert(key, entry.clone());
        self.expiring.push_back(entry);

        render_image
    }
}
