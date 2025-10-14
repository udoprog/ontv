use std::collections::{HashMap, HashSet, VecDeque};

use iced::advanced::image::Handle;
use iced::Theme;

use crate::cache::ImageHint;
use crate::model::{ImageHash, ImageV2};

static MISSING_POSTER_DARK: &[u8] = include_bytes!("../assets/missing_poster_dark.png");
static MISSING_POSTER_LIGHT: &[u8] = include_bytes!("../assets/missing_poster_light.png");
static MISSING_BANNER: &[u8] = include_bytes!("../assets/missing_banner.png");
static MISSING_SCRENCAP: &[u8] = include_bytes!("../assets/missing_screencap.png");

/// They key identifying an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ImageKey {
    pub(crate) id: ImageHash,
    pub(crate) hint: Option<ImageHint>,
}

/// Keeping track of assets that needs to be stored in-memory or loaded from the
/// filesystem.
pub(crate) struct Assets {
    /// Handle for missing posters.
    missing_poster_dark: Handle,
    missing_poster_light: Handle,
    /// Handle for missing banners.
    missing_banner: Handle,
    /// Handle for missing screen caps.
    missing_screencap: Handle,
    /// Set to clear image cache on next commit.
    clear: bool,
    /// Image queue to load.
    image_queue: VecDeque<(ImageKey, ImageV2)>,
    /// Images marked for loading.
    marked: Vec<(ImageKey, ImageV2)>,
    /// Images stored in-memory.
    images: HashMap<ImageKey, Handle>,
    /// Assets to remove.
    to_remove: HashSet<ImageKey>,
}

impl Assets {
    pub(crate) fn new() -> Self {
        let missing_poster_dark = Handle::from_bytes(MISSING_POSTER_DARK);
        let missing_poster_light = Handle::from_bytes(MISSING_POSTER_LIGHT);
        let missing_banner = Handle::from_bytes(MISSING_BANNER);
        let missing_screencap = Handle::from_bytes(MISSING_SCRENCAP);

        Self {
            missing_poster_dark,
            missing_poster_light,
            missing_banner,
            missing_screencap,
            clear: false,
            image_queue: VecDeque::new(),
            marked: Vec::new(),
            images: HashMap::new(),
            to_remove: HashSet::new(),
        }
    }

    /// Clear in-memory assets.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.clear = true;
    }

    /// If assets have been cleared.
    #[inline]
    pub(crate) fn is_cleared(&self) -> bool {
        self.clear
    }

    /// Setup images to load task.
    #[allow(unused)]
    pub(crate) fn mark<'a, I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = &'a ImageV2>,
    {
        for id in ids {
            tracing::trace!(?id, "mark");

            let key = ImageKey {
                id: id.hash(),
                hint: None,
            };

            self.marked.push((key, id.clone()));
        }
    }

    /// Setup images to load task.
    pub(crate) fn mark_with_hint<'a, I>(&mut self, ids: I, hint: ImageHint)
    where
        I: IntoIterator<Item = &'a ImageV2>,
    {
        for id in ids {
            tracing::trace!(?id, ?hint, "mark");

            let key = ImageKey {
                id: id.hash(),
                hint: Some(hint),
            };

            self.marked.push((key, id.clone()));
        }
    }

    /// Commit new images to load.
    pub(crate) fn commit(&mut self) {
        // Intersect already loaded assets with assets marked for loading.
        if self.clear {
            self.to_remove
                .extend(self.images.keys().copied().collect::<HashSet<_>>());

            for (key, _) in &self.marked {
                self.to_remove.remove(key);
            }

            // Remove assets which are no longer used.
            for image in &self.to_remove {
                tracing::trace!(?image, "Unloading");
                let _ = self.images.remove(image);
            }

            // Clear set of images to remove.
            self.to_remove.clear();
            // Clear current queue.
            self.image_queue.clear();
            self.clear = false;
        }

        for (key, image) in self.marked.drain(..) {
            if !self.images.contains_key(&key) {
                self.image_queue.push_back((key, image));
            }
        }

        self.marked.clear();
    }

    /// Insert loaded images.
    pub(crate) fn insert_images(&mut self, loaded: Vec<(ImageKey, Handle)>) {
        for (id, handle) in loaded {
            self.images.insert(id, handle);
        }
    }

    /// Get a placeholder image for a missing poster.
    pub(crate) fn missing_poster(&self, theme: &Theme) -> Handle {
        match theme {
            Theme::Dark => self.missing_poster_dark.clone(),
            _ => self.missing_poster_light.clone(),
        }
    }

    /// Get an image without a hint.
    #[allow(unused)]
    pub(crate) fn image(&self, id: &ImageV2) -> Option<Handle> {
        let key = ImageKey {
            id: id.hash(),
            hint: None,
        };

        self.images.get(&key).cloned()
    }

    /// Get an image with the specified hint.
    pub(crate) fn image_with_hint(&self, id: &ImageV2, hint: ImageHint) -> Option<Handle> {
        let key = ImageKey {
            id: id.hash(),
            hint: Some(hint),
        };

        self.images.get(&key).cloned()
    }

    /// Get a placeholder image for a missing banner.
    pub(crate) fn missing_banner(&self) -> Handle {
        self.missing_banner.clone()
    }

    /// Get a placeholder image for a missing screencap.
    pub(crate) fn missing_screen_capture(&self) -> Handle {
        self.missing_screencap.clone()
    }

    /// Get the next image to load.
    pub(crate) fn next_image(&mut self) -> Option<(ImageKey, ImageV2)> {
        loop {
            let (key, image) = self.image_queue.pop_front()?;

            if self.images.contains_key(&key) {
                continue;
            }

            return Some((key, image));
        }
    }
}
