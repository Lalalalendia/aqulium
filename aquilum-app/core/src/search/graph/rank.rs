const ITERATIONS: usize = 20;
const DAMPING: f64 = 0.85;

pub struct OutLinks {
    offsets: Vec<u32>,
    targets: Vec<u32>,
}

pub fn out_links(node_count: usize, directed: &[(u32, u32)]) -> OutLinks {
    let mut offsets = vec![0u32; node_count + 1];
    for (source, _) in directed {
        offsets[*source as usize + 1] += 1;
    }
    for node in 0..node_count {
        offsets[node + 1] += offsets[node];
    }
    let mut cursor = offsets.clone();
    let mut targets = vec![0u32; directed.len()];
    for (source, target) in directed {
        let slot = &mut cursor[*source as usize];
        targets[*slot as usize] = *target;
        *slot += 1;
    }
    OutLinks { offsets, targets }
}

pub fn pagerank(node_count: usize, links: &OutLinks) -> Vec<f32> {
    if node_count == 0 {
        return Vec::new();
    }
    let count = node_count as f64;
    let mut rank = vec![1.0 / count; node_count];
    let mut received = vec![0.0; node_count];
    for _ in 0..ITERATIONS {
        received.fill(0.0);
        let mut dangling = 0.0;
        for (node, current) in rank.iter().enumerate() {
            let start = links.offsets[node] as usize;
            let end = links.offsets[node + 1] as usize;
            if start == end {
                dangling += current;
                continue;
            }
            let share = current / (end - start) as f64;
            for slot in start..end {
                received[links.targets[slot] as usize] += share;
            }
        }
        let base = (1.0 - DAMPING) / count + DAMPING * dangling / count;
        for (value, incoming) in rank.iter_mut().zip(&received) {
            *value = base + DAMPING * incoming;
        }
    }
    rank.into_iter().map(|value| value as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::{out_links, pagerank};

    #[test]
    fn a_linked_note_outranks_the_notes_linking_to_it() {
        let directed = [(1, 0), (2, 0), (3, 0)];
        let links = out_links(4, &directed);

        let rank = pagerank(4, &links);

        assert!(rank[0] > rank[1]);
        assert!((rank[1] - rank[3]).abs() < 1e-9);
    }

    #[test]
    fn ranks_sum_to_one_even_with_dangling_notes() {
        let directed = [(0, 1)];
        let links = out_links(3, &directed);

        let total: f32 = pagerank(3, &links).iter().sum();

        assert!((total - 1.0).abs() < 1e-4);
    }

    #[test]
    fn an_empty_graph_yields_no_ranks() {
        assert!(pagerank(0, &out_links(0, &[])).is_empty());
    }
}
