/// The strongly connected components of a graph given as each node's successors, callees
/// first: a component comes after every component it points to. Nodes in a component are
/// in ascending order. Iterative, so a long chain cannot overflow the stack.
pub fn components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    const UNSEEN: usize = usize::MAX;
    let n = edges.len();
    let (mut index, mut low) = (vec![UNSEEN; n], vec![0; n]);
    let (mut open, mut stack, mut out) = (vec![false; n], Vec::new(), Vec::new());
    let mut counter = 0;
    for root in 0..n {
        if index[root] != UNSEEN {
            continue;
        }
        let mut work = vec![(root, 0)];
        (index[root], low[root], open[root]) = (counter, counter, true);
        counter += 1;
        stack.push(root);
        while let Some(&(v, next)) = work.last() {
            let Some(&w) = edges[v].get(next) else {
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == index[v] {
                    out.push(close(&mut stack, &mut open, v));
                }
                continue;
            };
            if let Some(top) = work.last_mut() {
                top.1 += 1;
            }
            if index[w] == UNSEEN {
                (index[w], low[w], open[w]) = (counter, counter, true);
                counter += 1;
                stack.push(w);
                work.push((w, 0));
            } else if open[w] {
                low[v] = low[v].min(index[w]);
            }
        }
    }
    out
}

/// Pops the component that `root` heads off the stack.
fn close(stack: &mut Vec<usize>, open: &mut [bool], root: usize) -> Vec<usize> {
    let mut component = Vec::new();
    while let Some(w) = stack.pop() {
        open[w] = false;
        component.push(w);
        if w == root {
            break;
        }
    }
    component.sort_unstable();
    component
}

#[cfg(test)]
mod tests {
    use super::components;

    #[test]
    fn callees_come_first_and_cycles_group() {
        let edges = vec![vec![1, 3], vec![2], vec![1], vec![], vec![]];
        let found = components(&edges);
        let pos = |n: usize| found.iter().position(|c| c.contains(&n)).unwrap();
        assert!(found.contains(&vec![1, 2]));
        assert!(pos(1) < pos(0) && pos(3) < pos(0));
        assert_eq!(found.len(), 4);
    }
}
