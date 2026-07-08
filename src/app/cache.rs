use crate::view::CommitRow as Commit;

/// Cache for full and simplified commit graph modes, enabling instant toggling.
pub struct CommitCache {
    full: Option<Vec<Commit>>,
    simplified: Option<Vec<Commit>>,
}

impl CommitCache {
    pub fn new() -> Self {
        CommitCache {
            full: None,
            simplified: None,
        }
    }

    /// Store the current commit list for the given mode.
    pub fn set(&mut self, simplified: bool, commits: Vec<Commit>) {
        if simplified {
            self.simplified = Some(commits);
        } else {
            self.full = Some(commits);
        }
    }

    /// Try to retrieve a cached commit list for the given mode.
    pub fn get(&mut self, simplified: bool) -> Option<Vec<Commit>> {
        if simplified {
            self.simplified.take()
        } else {
            self.full.take()
        }
    }

    /// Invalidate all cached commit lists.
    pub fn invalidate(&mut self) {
        self.full = None;
        self.simplified = None;
    }

    /// Returns true if both caches are empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.full.is_none() && self.simplified.is_none()
    }
}

impl Default for CommitCache {
    fn default() -> Self {
        Self::new()
    }
}
