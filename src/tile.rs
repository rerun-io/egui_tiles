use crate::{Container, ContainerKind};

/// An identifier for a [`Tile`] in the tree, be it a [`Container`] or a pane.
#[derive(Clone, Copy, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TileId(u64);

/// [`TileId`] is a high-entropy random id, so this is fine:
impl nohash_hasher::IsEnabled for TileId {}

impl TileId {
    /// Generate a new random [`TileId`].
    pub fn random() -> Self {
        use rand::Rng as _;
        Self(rand::thread_rng().gen())
    }

    /// Corresponding [`egui::Id`], used for dragging.
    pub fn id(&self) -> egui::Id {
        egui::Id::new(self)
    }
}

impl std::fmt::Debug for TileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08X}", self.0 as u32)
    }
}

// ----------------------------------------------------------------------------

/// A tile in the tree. Either a pane (leaf) or a [`Container`] of more tiles.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Tile<Pane> {
    /// A leaf. This is where the user puts their UI, using the [`crate::Behavior`] trait.
    Pane(Pane),

    /// A container of more tiles, e.g. a horizontal layout or a tab layout.
    Container(Container),
}

impl<T> From<Container> for Tile<T> {
    #[inline]
    fn from(container: Container) -> Self {
        Self::Container(container)
    }
}

impl<Pane> Tile<Pane> {
    /// Returns `None` if this is a [`Self::Pane`].
    #[inline]
    pub fn kind(&self) -> Option<ContainerKind> {
        match self {
            Tile::Pane(_) => None,
            Tile::Container(container) => Some(container.kind()),
        }
    }

    #[inline]
    pub fn is_pane(&self) -> bool {
        matches!(self, Self::Pane(_))
    }

    #[inline]
    pub fn is_container(&self) -> bool {
        matches!(self, Self::Container(_))
    }
}
