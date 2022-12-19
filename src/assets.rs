use std::collections::{HashMap, HashSet, VecDeque};

use iced_native::image::Handle;

use crate::model::Image;

static MISSING_BANNER: &[u8] = include_bytes!("../assets/missing_banner.png");

/// Keeping track of assets that needs to be stored in-memory or loaded from the
/// filesystem.
pub(crate) struct Assets {
    missing_banner: Handle,
    missing_poster: Handle,
    missing_screencap: Handle,
    /// Set to clear image cache on next commit.
    clear: bool,
    /// Image queue to load.
    pub(crate) image_ids: VecDeque<Image>,
    /// Images marked for loading.
    pub(crate) marked: Vec<Image>,
    /// Images stored in-memory.
    images: HashMap<Image, Handle>,
    /// Assets to remove.
    to_remove: HashSet<Image>,
}

impl Assets {
    pub(crate) fn new() -> Self {
        let missing_banner = Handle::from_memory(MISSING_BANNER);
        let missing_screencap = Handle::from_memory(MISSING_BANNER);
        let missing_poster = Handle::from_memory(MISSING_BANNER);

        Self {
            missing_banner,
            missing_screencap,
            missing_poster,
            clear: false,
            image_ids: VecDeque::new(),
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
    pub(crate) fn mark<I>(&mut self, images: I)
    where
        I: IntoIterator<Item = Image>,
    {
        for image in images {
            log::trace!("mark: {image}");
            self.marked.push(image);
        }
    }

    /// Commit new images to load.
    pub(crate) fn commit(&mut self) {
        // Intersect already loaded assets with assets marked for loading.
        if self.clear {
            self.to_remove
                .extend(self.images.keys().copied().collect::<HashSet<_>>());

            for image in &self.marked {
                self.to_remove.remove(image);
            }

            // Remove assets which are no longer used.
            for image in &self.to_remove {
                log::trace!("unloading: {image}");
                let _ = self.images.remove(image);
            }

            // Clear set of images to remove.
            self.to_remove.clear();
            // Clear current queue.
            self.image_ids.clear();
            self.clear = false;
        }

        for image in &self.marked {
            if !self.images.contains_key(image) {
                self.image_ids.push_back(*image);
            }
        }

        self.marked.clear();
    }

    /// Insert loaded images.
    pub(crate) fn insert_images(&mut self, loaded: Vec<(Image, Handle)>) {
        for (id, handle) in loaded {
            self.images.insert(id, handle);
        }
    }

    /// Get an image, will return the default handle if the given image doesn't exist.
    pub(crate) fn image(&self, id: &Image) -> Option<Handle> {
        self.images.get(&id).cloned()
    }

    /// Get a placeholder image for a missing banner.
    pub(crate) fn missing_banner(&self) -> Handle {
        self.missing_banner.clone()
    }

    /// Get a placeholder image for a missing poster.
    pub(crate) fn missing_poster(&self) -> Handle {
        self.missing_poster.clone()
    }

    /// Get a placeholder image for a missing screencap.
    pub(crate) fn missing_screencap(&self) -> Handle {
        self.missing_screencap.clone()
    }
}
