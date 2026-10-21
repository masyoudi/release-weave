/// Pure fuzzy filtering and scoring engine for the command palette.
/// Evaluates query against command titles and keywords using prefix, word-boundary,
/// and subsequence heuristics without any GPUI dependencies.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoredItem<'a, T> {
    pub item: &'a T,
    pub score: i32,
}

/// Calculate fuzzy match score of `query` against `target`.
/// Returns `None` if `query` is not a subsequence of `target`.
pub fn fuzzy_match(query: &str, target: &str) -> Option<i32> {
    let q = query.trim().to_lowercase();
    let t = target.to_lowercase();

    if q.is_empty() {
        return Some(0);
    }

    // Exact full match
    if t == q {
        return Some(1000);
    }

    // Prefix match on entire string
    if t.starts_with(&q) {
        let length_penalty = (t.len() - q.len()).min(50) as i32;
        return Some(800 - length_penalty);
    }

    // Prefix match on any individual word in target
    for word in t.split_whitespace() {
        if word.starts_with(&q) {
            let length_penalty = (word.len() - q.len()).min(40) as i32;
            return Some(650 - length_penalty);
        }
    }

    // Subsequence match with word boundary & consecutive bonuses
    let mut score = 0;
    let mut q_chars = q.chars().peekable();
    let mut last_match_idx: Option<usize> = None;
    let mut consecutive = 0;

    let chars: Vec<char> = t.chars().collect();
    let mut t_idx = 0;

    while let Some(&qc) = q_chars.peek() {
        let mut matched = false;
        while t_idx < chars.len() {
            let tc = chars[t_idx];
            if tc == qc {
                matched = true;
                q_chars.next();

                // Base match points
                score += 20;

                // Word boundary bonus: start of word (following whitespace, hyphen, underscore, slash)
                let is_boundary = t_idx == 0
                    || chars[t_idx - 1] == ' '
                    || chars[t_idx - 1] == '-'
                    || chars[t_idx - 1] == '_'
                    || chars[t_idx - 1] == '/';
                if is_boundary {
                    score += 50;
                }

                // Consecutive match bonus
                if let Some(prev) = last_match_idx {
                    if prev + 1 == t_idx {
                        consecutive += 1;
                        score += 30 * consecutive;
                    } else {
                        consecutive = 0;
                        // Distance penalty
                        let gap = (t_idx - prev - 1) as i32;
                        score -= gap * 2;
                    }
                }
                last_match_idx = Some(t_idx);
                t_idx += 1;
                break;
            }
            t_idx += 1;
        }

        if !matched {
            return None;
        }
    }

    Some(score)
}

/// Computes best score of `query` against a command's `title` and its associated `keywords`.
pub fn score_command(query: &str, title: &str, keywords: &[String]) -> Option<i32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(0);
    }

    let mut highest_score: Option<i32> = None;

    // 1. Title match
    if let Some(s) = fuzzy_match(&q, title) {
        highest_score = Some(s);
    }

    // 2. Acronym / First-letter of words match against title (e.g. "rrc" -> "Run Readiness Checks")
    let initials: String = title
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .collect::<String>()
        .to_lowercase();
    if initials.starts_with(&q) {
        let acronym_score = 750 + (30 - (initials.len() - q.len()).min(30) as i32);
        highest_score = Some(highest_score.map_or(acronym_score, |s| s.max(acronym_score)));
    }

    // 3. Keyword matches
    for kw in keywords {
        let kw_lower = kw.to_lowercase();
        if kw_lower == q {
            // Exact keyword match receives a major boost
            let kw_score = 900;
            // Extra boost if title also starts with same letter as query
            let start_boost = if title.to_lowercase().starts_with(&q[..1]) {
                50
            } else {
                0
            };
            let total = kw_score + start_boost;
            highest_score = Some(highest_score.map_or(total, |s| s.max(total)));
        } else if kw_lower.starts_with(&q) {
            let kw_score = 700 + (30 - (kw_lower.len() - q.len()).min(30) as i32);
            highest_score = Some(highest_score.map_or(kw_score, |s| s.max(kw_score)));
        } else if let Some(s) = fuzzy_match(&q, &kw_lower) {
            let kw_score = s + 50;
            highest_score = Some(highest_score.map_or(kw_score, |cur| cur.max(kw_score)));
        }
    }

    highest_score
}

/// Filter and sort candidates by score in descending order.
pub fn filter_items<'a, T, FTitle, FKeywords>(
    query: &str,
    items: &'a [T],
    get_title: FTitle,
    get_keywords: FKeywords,
) -> Vec<ScoredItem<'a, T>>
where
    FTitle: Fn(&T) -> &str,
    FKeywords: Fn(&T) -> &[String],
{
    let mut scored: Vec<ScoredItem<'a, T>> = items
        .iter()
        .filter_map(|item| {
            let title = get_title(item);
            let keywords = get_keywords(item);
            score_command(query, title, keywords).map(|score| ScoredItem { item, score })
        })
        .collect();

    // Sort by score descending, then alphabetically by title
    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| get_title(a.item).cmp(get_title(b.item)))
    });

    scored
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCommand {
        title: &'static str,
        keywords: Vec<String>,
    }

    #[test]
    fn exact_and_prefix_matching() {
        assert!(fuzzy_match("overview", "Overview").unwrap() >= 1000);
        assert!(fuzzy_match("over", "Overview").unwrap() >= 750);
        assert!(fuzzy_match("xyz", "Overview").is_none());
    }

    #[test]
    fn word_boundary_scoring() {
        // "rn" in "Release Notes" matches initials R and N
        let score_rn = fuzzy_match("rn", "Release Notes").unwrap();
        // "rn" in "Warning" does not have word boundaries
        let score_warn = fuzzy_match("rn", "Warning").unwrap();
        assert!(score_rn > score_warn);
    }

    #[test]
    fn rdy_finds_run_readiness_checks_first() {
        let commands = vec![
            MockCommand {
                title: "Go to Overview",
                keywords: vec!["overview".to_string(), "dashboard".to_string()],
            },
            MockCommand {
                title: "Go to Changes",
                keywords: vec!["changes".to_string(), "commits".to_string()],
            },
            MockCommand {
                title: "Go to Release Notes",
                keywords: vec!["notes".to_string(), "changelog".to_string()],
            },
            MockCommand {
                title: "Go to Readiness Checks",
                keywords: vec![
                    "readiness".to_string(),
                    "checks".to_string(),
                    "gates".to_string(),
                ],
            },
            MockCommand {
                title: "Run Readiness Checks",
                keywords: vec![
                    "readiness".to_string(),
                    "rdy".to_string(),
                    "checks".to_string(),
                    "run".to_string(),
                ],
            },
            MockCommand {
                title: "Toggle Color Theme",
                keywords: vec!["theme".to_string(), "mode".to_string()],
            },
        ];

        let results = filter_items("rdy", &commands, |c| c.title, |c| &c.keywords);

        assert!(
            !results.is_empty(),
            "Expected at least one matching command"
        );
        assert_eq!(
            results[0].item.title, "Run Readiness Checks",
            "Expected 'Run Readiness Checks' to rank first for query 'rdy'"
        );
    }
}
