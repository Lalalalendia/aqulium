use super::models::AnalysisResult;
use crate::search::paths::identity;
use crate::search::error::SearchError;
use crate::search::wiki;
use rusqlite::Connection;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct GraphSnapshot {
    neighbours: HashMap<String, HashSet<String>>,
    outgoing: HashMap<String, HashSet<String>>,
    paths: HashMap<String, String>,
}

struct CandidateScore {
    raw_score: f64,
    common_neighbours: HashSet<String>,
}

impl GraphSnapshot {
    pub fn load(connection: &Connection, root: &Path) -> Result<Self, SearchError> {
        let documents = wiki::read_documents(connection)?;
        let mut neighbours = HashMap::<String, HashSet<String>>::new();
        let mut outgoing = HashMap::<String, HashSet<String>>::new();
        let mut paths = HashMap::<String, String>::new();
        wiki::for_each_link(
            connection,
            root,
            documents.iter().map(|document| {
                (
                    document.path.as_str(),
                    document.relative_key.as_str(),
                    document.title_key.as_str(),
                )
            }),
            |source, target| {
                let source_path = source;
                let target_path = target;
                let source = identity(Path::new(source_path));
                let target = identity(Path::new(target_path));
                if source == target {
                    return;
                }
                paths
                    .entry(source.clone())
                    .or_insert_with(|| source_path.to_owned());
                paths
                    .entry(target.clone())
                    .or_insert_with(|| target_path.to_owned());
                neighbours
                    .entry(source.clone())
                    .or_default()
                    .insert(target.clone());
                neighbours
                    .entry(target.clone())
                    .or_default()
                    .insert(source.clone());
                outgoing.entry(source).or_default().insert(target);
            },
        )?;
        Ok(Self {
            neighbours,
            outgoing,
            paths,
        })
    }

    pub fn incoming_counts(&self, documents: &[PathBuf]) -> Vec<usize> {
        let mut counts = HashMap::<&str, usize>::new();
        for targets in self.outgoing.values() {
            for target in targets {
                *counts.entry(target.as_str()).or_default() += 1;
            }
        }
        documents
            .iter()
            .map(|document| counts.get(identity(document).as_str()).copied().unwrap_or(0))
            .collect()
    }

    pub fn adamic_adar(&self, document: &Path, limit: usize) -> Vec<AnalysisResult> {
        let current = identity(document);
        let Some(current_neighbours) = self.neighbours.get(&current) else {
            return Vec::new();
        };
        let mut shared = HashMap::<String, CandidateScore>::new();
        for neighbour in current_neighbours {
            let weight = self.neighbour_weight(neighbour);
            if weight == 0.0 {
                continue;
            }
            if let Some(candidates) = self.neighbours.get(neighbour) {
                for candidate in candidates {
                    if candidate != &current {
                        let entry =
                            shared
                                .entry(candidate.clone())
                                .or_insert_with(|| CandidateScore {
                                    raw_score: 0.0,
                                    common_neighbours: HashSet::new(),
                                });
                        entry.raw_score += weight;
                        entry.common_neighbours.insert(neighbour.clone());
                    }
                }
            }
        }
        self.rank(shared, limit)
    }

    fn neighbour_weight(&self, neighbour: &str) -> f64 {
        let out_degree = self.outgoing.get(neighbour).map_or(0, HashSet::len);
        if out_degree == 0 {
            return 0.0;
        }
        1.0 / (out_degree.max(2) as f64).ln()
    }

    fn rank(&self, values: HashMap<String, CandidateScore>, limit: usize) -> Vec<AnalysisResult> {
        let mut results = values
            .into_iter()
            .filter(|(_, candidate)| candidate.raw_score > 0.0)
            .filter_map(|(identity, candidate)| {
                let path = self.paths.get(&identity)?.clone();
                let mut reasons = candidate
                    .common_neighbours
                    .into_iter()
                    .filter_map(|identity| self.paths.get(&identity))
                    .map(|path| title(path))
                    .collect::<Vec<_>>();
                reasons.sort();
                Some(AnalysisResult {
                    title: title(&path),
                    path,
                    raw_score: candidate.raw_score,
                    similarity: None,
                    confidence: None,
                    reasons,
                })
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .raw_score
                .partial_cmp(&left.raw_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| right.reasons.len().cmp(&left.reasons.len()))
                .then_with(|| left.title.cmp(&right.title))
        });
        results.truncate(limit);
        results
    }
}

fn title(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{identity, GraphSnapshot};
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    fn node(name: &str) -> String {
        identity(Path::new(&format!("C:/vault/{name}.md")))
    }

    #[test]
    fn returns_candidates_and_their_common_neighbours() {
        let mut neighbours = HashMap::<String, HashSet<String>>::new();
        let mut connect = |left: &str, right: &str| {
            neighbours
                .entry(node(left))
                .or_default()
                .insert(node(right));
            neighbours
                .entry(node(right))
                .or_default()
                .insert(node(left));
        };

        connect("A", "X");
        connect("A", "Y");
        connect("A", "B");
        connect("A", "C");
        connect("X", "C");
        connect("Y", "C");
        connect("X", "B");

        let paths = ["A", "B", "C", "X", "Y"]
            .into_iter()
            .map(|name| (node(name), format!("C:/vault/{name}.md")))
            .collect();
        let outgoing = HashMap::from([
            (node("X"), HashSet::from([node("A"), node("B"), node("C")])),
            (node("Y"), HashSet::from([node("A"), node("C")])),
        ]);
        let graph = GraphSnapshot {
            neighbours,
            outgoing,
            paths,
        };

        let results = graph.adamic_adar(Path::new("C:/vault/A.md"), 10);

        let result = results.iter().find(|result| result.title == "C").unwrap();
        assert_eq!(result.reasons, vec!["X".to_owned(), "Y".to_owned()]);
        let expected = 1.0 / 3_f64.ln() + 1.0 / 2_f64.ln();
        assert!((result.raw_score - expected).abs() < f64::EPSILON);
    }

    #[test]
    fn keeps_candidates_sharing_a_neighbour_with_a_single_outgoing_link() {
        let a = node("A");
        let b = node("B");
        let x = node("X");
        let neighbours = HashMap::from([
            (a.clone(), HashSet::from([x.clone()])),
            (b.clone(), HashSet::from([x.clone()])),
            (x.clone(), HashSet::from([a.clone(), b.clone()])),
        ]);
        let outgoing = HashMap::from([(x, HashSet::from([b.clone()]))]);
        let paths = ["A", "B", "X"]
            .into_iter()
            .map(|name| (node(name), format!("C:/vault/{name}.md")))
            .collect();
        let graph = GraphSnapshot {
            neighbours,
            outgoing,
            paths,
        };

        let result = graph
            .adamic_adar(Path::new("C:/vault/A.md"), 10)
            .into_iter()
            .find(|result| result.title == "B")
            .expect("candidate behind a single-link neighbour must survive ranking");

        assert!((result.raw_score - 1.0 / 2_f64.ln()).abs() < f64::EPSILON);
    }

    #[test]
    fn weights_common_neighbours_by_outgoing_links_like_obsidian_graph_analysis() {
        let a = node("A");
        let b = node("B");
        let x = node("X");
        let y = node("Y");
        let z = node("Z");
        let neighbours = HashMap::from([
            (a.clone(), HashSet::from([x.clone()])),
            (b.clone(), HashSet::from([x.clone()])),
            (
                x.clone(),
                HashSet::from([a.clone(), b.clone(), y.clone(), z.clone()]),
            ),
        ]);
        let outgoing = HashMap::from([(x, HashSet::from([node("A"), node("B")]))]);
        let paths = ["A", "B", "X", "Y", "Z"]
            .into_iter()
            .map(|name| (node(name), format!("C:/vault/{name}.md")))
            .collect();
        let graph = GraphSnapshot {
            neighbours,
            outgoing,
            paths,
        };

        let result = graph
            .adamic_adar(Path::new("C:/vault/A.md"), 10)
            .into_iter()
            .find(|result| result.title == "B")
            .unwrap();

        assert!((result.raw_score - 1.0 / 2_f64.ln()).abs() < f64::EPSILON);
    }

    #[test]
    fn counts_distinct_notes_linking_to_each_document() {
        let outgoing = HashMap::from([
            (node("X"), HashSet::from([node("A"), node("B")])),
            (node("Y"), HashSet::from([node("A")])),
        ]);
        let graph = GraphSnapshot {
            neighbours: HashMap::new(),
            outgoing,
            paths: HashMap::new(),
        };
        let documents = ["A", "B", "X"].map(|name| std::path::PathBuf::from(format!("C:/vault/{name}.md")));
        assert_eq!(graph.incoming_counts(&documents), vec![2, 1, 0]);
    }
}
