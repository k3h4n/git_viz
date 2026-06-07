use crate::models::stats::AnalysisResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Overview,
    Timeline,
    Contributors,
    Hotspots,
    Branches,
}

impl ViewMode {
    pub const ALL: [ViewMode; 5] = [
        ViewMode::Overview,
        ViewMode::Timeline,
        ViewMode::Contributors,
        ViewMode::Hotspots,
        ViewMode::Branches,
    ];

    pub fn title(self) -> &'static str {
        match self {
            ViewMode::Overview => "Overview",
            ViewMode::Timeline => "Timeline",
            ViewMode::Contributors => "Contributors",
            ViewMode::Hotspots => "Hotspots",
            ViewMode::Branches => "Branches",
        }
    }

    pub fn next(self) -> ViewMode {
        let idx = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> ViewMode {
        let idx = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        if idx == 0 {
            Self::ALL[Self::ALL.len() - 1]
        } else {
            Self::ALL[idx - 1]
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
}

pub struct App {
    pub should_quit: bool,
    pub current_view: ViewMode,
    pub input_mode: InputMode,
    pub search_query: String,
    pub analysis: AnalysisResult,
    pub timeline_scroll: usize,
    pub contributors_scroll: usize,
    pub hotspots_scroll: usize,
    pub branches_scroll: usize,
    pub repo_path: String,
}

impl App {
    pub fn new(analysis: AnalysisResult, repo_path: String) -> Self {
        App {
            should_quit: false,
            current_view: ViewMode::Overview,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            analysis,
            timeline_scroll: 0,
            contributors_scroll: 0,
            hotspots_scroll: 0,
            branches_scroll: 0,
            repo_path,
        }
    }

    pub fn scroll_up(&mut self) {
        match self.current_view {
            ViewMode::Timeline => {
                self.timeline_scroll = self.timeline_scroll.saturating_sub(1);
            }
            ViewMode::Contributors => {
                self.contributors_scroll = self.contributors_scroll.saturating_sub(1);
            }
            ViewMode::Hotspots => {
                self.hotspots_scroll = self.hotspots_scroll.saturating_sub(1);
            }
            ViewMode::Branches => {
                self.branches_scroll = self.branches_scroll.saturating_sub(1);
            }
            ViewMode::Overview => {}
        }
    }

    pub fn scroll_down(&mut self) {
        let max = self.scroll_max();
        match self.current_view {
            ViewMode::Timeline => {
                self.timeline_scroll = self.timeline_scroll.saturating_add(1).min(max);
            }
            ViewMode::Contributors => {
                self.contributors_scroll = self.contributors_scroll.saturating_add(1).min(max);
            }
            ViewMode::Hotspots => {
                self.hotspots_scroll = self.hotspots_scroll.saturating_add(1).min(max);
            }
            ViewMode::Branches => {
                self.branches_scroll = self.branches_scroll.saturating_add(1).min(max);
            }
            ViewMode::Overview => {}
        }
    }

    pub fn scroll_top(&mut self) {
        match self.current_view {
            ViewMode::Timeline => self.timeline_scroll = 0,
            ViewMode::Contributors => self.contributors_scroll = 0,
            ViewMode::Hotspots => self.hotspots_scroll = 0,
            ViewMode::Branches => self.branches_scroll = 0,
            ViewMode::Overview => {}
        }
    }

    pub fn scroll_bottom(&mut self) {
        let max = self.scroll_max();
        match self.current_view {
            ViewMode::Timeline => self.timeline_scroll = max,
            ViewMode::Contributors => self.contributors_scroll = max,
            ViewMode::Hotspots => self.hotspots_scroll = max,
            ViewMode::Branches => self.branches_scroll = max,
            ViewMode::Overview => {}
        }
    }

    fn scroll_max(&self) -> usize {
        match self.current_view {
            ViewMode::Timeline => self.analysis.timeline.len().saturating_sub(1),
            ViewMode::Contributors => self.analysis.contributors.len().saturating_sub(1),
            ViewMode::Hotspots => self.analysis.hotspots.len().saturating_sub(1),
            ViewMode::Branches => self
                .analysis
                .branch_graph
                .as_ref()
                .map(|g| g.branches.len().saturating_sub(1))
                .unwrap_or(0),
            ViewMode::Overview => 0,
        }
    }

    pub fn current_scroll(&self) -> usize {
        match self.current_view {
            ViewMode::Timeline => self.timeline_scroll,
            ViewMode::Contributors => self.contributors_scroll,
            ViewMode::Hotspots => self.hotspots_scroll,
            ViewMode::Branches => self.branches_scroll,
            ViewMode::Overview => 0,
        }
    }

    pub fn filtered_timeline(&self) -> Vec<&crate::models::stats::CommitTimelineEntry> {
        if self.search_query.is_empty() {
            self.analysis.timeline.iter().collect()
        } else {
            let query = self.search_query.to_lowercase();
            self.analysis
                .timeline
                .iter()
                .filter(|e| {
                    e.message.to_lowercase().contains(&query)
                        || e.author.to_lowercase().contains(&query)
                        || e.hash.to_lowercase().contains(&query)
                })
                .collect()
        }
    }
}
