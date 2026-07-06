use std::fmt;

/// The currently focused UI panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Branches,
    Search,
    Scope,
    Commits,
    Diff,
}

impl Panel {
    pub const ALL: [Panel; 5] = [
        Panel::Branches,
        Panel::Search,
        Panel::Scope,
        Panel::Commits,
        Panel::Diff,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

impl fmt::Display for Panel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Panel::Branches => write!(f, "Branches"),
            Panel::Search => write!(f, "Search"),
            Panel::Scope => write!(f, "Scope"),
            Panel::Commits => write!(f, "Commits"),
            Panel::Diff => write!(f, "Diff"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_next_cycles_forward() {
        assert_eq!(Panel::Branches.next(), Panel::Search);
        assert_eq!(Panel::Search.next(), Panel::Scope);
        assert_eq!(Panel::Scope.next(), Panel::Commits);
        assert_eq!(Panel::Commits.next(), Panel::Diff);
        assert_eq!(Panel::Diff.next(), Panel::Branches);
    }

    #[test]
    fn test_panel_prev_cycles_backward() {
        assert_eq!(Panel::Branches.prev(), Panel::Diff);
        assert_eq!(Panel::Diff.prev(), Panel::Commits);
        assert_eq!(Panel::Commits.prev(), Panel::Scope);
        assert_eq!(Panel::Scope.prev(), Panel::Search);
        assert_eq!(Panel::Search.prev(), Panel::Branches);
    }

    #[test]
    fn test_panel_display() {
        assert_eq!(Panel::Branches.to_string(), "Branches");
        assert_eq!(Panel::Search.to_string(), "Search");
        assert_eq!(Panel::Scope.to_string(), "Scope");
        assert_eq!(Panel::Commits.to_string(), "Commits");
        assert_eq!(Panel::Diff.to_string(), "Diff");
    }
}
