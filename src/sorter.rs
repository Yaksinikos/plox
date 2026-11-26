use std::collections::HashMap;

use log::warn;
use petgraph::{graph::NodeIndex, stable_graph::StableGraph};

use crate::{
    EOrderRule, ESupportedGame, EWarningRule, GRAPH_FILE, PluginData,
    get_ordering_from_order_rules, nearend2, nearstart2, wild_contains,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ESortType {
    Unstable,
    StableOpt,
    StableFull,
}

#[must_use]
pub const fn new_unstable_sorter() -> Sorter {
    Sorter::new(ESortType::Unstable, 0)
}

#[must_use]
pub const fn new_stable_sorter() -> Sorter {
    Sorter::new(ESortType::StableOpt, 100)
}

pub struct GraphData {
    pub index_dict: HashMap<String, usize>,
    pub index_dict_rev: HashMap<usize, String>,
    pub edges: Vec<(usize, usize)>,
    /// Order rule edges dropped because they would close a cycle
    pub skipped: Vec<(usize, usize)>,
}

pub struct Sorter {
    pub sort_type: ESortType,
    pub max_iterations: usize,
}

impl Sorter {
    #[must_use]
    pub const fn new(sort_type: ESortType, max_iterations: usize) -> Self {
        Self {
            sort_type,
            max_iterations,
        }
    }

    /// Sorts the input mods topologically. Mods input is case sensitive!
    ///
    /// # Errors
    ///
    /// This function will return an error if any parsing fails
    pub fn topo_sort(
        &mut self,
        game: ESupportedGame,
        plugins: &[PluginData],
        order_rules: &[EOrderRule],
        warn_rules: &[EWarningRule],
    ) -> Result<Vec<String>, &'static str> {
        // early out
        if order_rules.is_empty() {
            log::info!("No order rules found, nothing to sort");
            return Err("No order rules found");
        }

        let data = get_graph_data(plugins, order_rules, warn_rules);
        let g = build_graph(&data);

        let GraphData {
            index_dict,
            index_dict_rev,
            mut edges,
            ..
        } = data;

        // cycle check
        if self.sort_type == ESortType::Unstable {
            let sort = petgraph::algo::toposort(&g, None).map_err(|_| "Graph contains a cycle")?;
            // map sorted index back to mods
            return Ok(sort
                .into_iter()
                .map(|idx| plugins[idx.index()].name.clone())
                .collect());
        }

        let mut mods = plugins
            .iter()
            .map(|f| f.name.to_lowercase())
            .collect::<Vec<String>>();
        apply_near_rules(&mut mods, order_rules);

        let n = plugins.len();
        let mut index = 0;
        edges.sort_by_key(|k| k.0);

        for i in 1..self.max_iterations {
            let any_change = self.stable_topo_sort_inner(
                n,
                &edges,
                &index_dict,
                &index_dict_rev,
                &mut mods,
                &mut index,
            );

            if !any_change {
                if game == ESupportedGame::Morrowind || game == ESupportedGame::Openmw {
                    move_masters_first(&mut mods);
                }
                // map sorted index back to mods
                return Ok(mods
                    .iter()
                    .map(|name| plugins[index_dict[name]].name.clone())
                    .collect());
            }

            if let Some(edge) = edges.get(index) {
                let resolved_0 = &index_dict_rev[&edge.0];
                let resolved_1 = &index_dict_rev[&edge.1];
                log::debug!("{i}, index {index} ({resolved_0}, {resolved_1})");
            } else {
                log::debug!("{i}, index {index}");
            }
        }
        // TODO find cycle
        {
            let viz = petgraph::dot::Dot::with_config(&g, &[petgraph::dot::Config::EdgeNoLabel]);
            if let Err(e) = std::fs::write(GRAPH_FILE, format!("{viz:?}")) {
                log::error!("Could not write {GRAPH_FILE}: {e}");
            }
        }

        log::error!("Out of iterations");
        Err("Out of iterations")
    }

    pub fn stable_topo_sort_inner(
        &self,
        n: usize,
        edges: &[(usize, usize)],
        index_dict: &HashMap<String, usize>,
        index_dict_rev: &HashMap<usize, String>,
        result: &mut Vec<String>,
        last_index: &mut usize,
    ) -> bool {
        match self.sort_type {
            // the unstable sort is done by petgraph and never iterates
            ESortType::Unstable => false,
            ESortType::StableOpt => {
                Self::stable_topo_sort_opt2(n, edges, index_dict_rev, result, last_index)
            }
            ESortType::StableFull => {
                Self::stable_topo_sort_full(n, edges, index_dict, result, last_index)
            }
        }
    }

    pub fn stable_topo_sort_full(
        n: usize,
        edges: &[(usize, usize)],
        index_dict: &HashMap<String, usize>,
        result: &mut Vec<String>,
        last_index: &mut usize,
    ) -> bool {
        for i in 0..n {
            for j in 0..i {
                let x = index_dict[result[i].as_str()];
                let y = index_dict[result[j].as_str()];
                if edges.contains(&(x, y)) {
                    let t = result[i].clone();
                    result.remove(i);
                    result.insert(j, t);

                    *last_index = j;

                    return true;
                }
            }
        }
        false
    }

    pub fn stable_topo_sort_opt2(
        _n: usize,
        edges: &[(usize, usize)],
        index_dict_rev: &HashMap<usize, String>,
        result: &mut Vec<String>,
        last_index: &mut usize,
    ) -> bool {
        // optimize B: only check edges
        let mut changed = false;
        for (edge_idx, (from, to)) in edges.iter().enumerate() {
            let before = &index_dict_rev[from];
            let after = &index_dict_rev[to];

            let (Some(pos_before), Some(pos_after)) = (
                result.iter().position(|f| f == before),
                result.iter().position(|f| f == after),
            ) else {
                continue;
            };

            // `before` must come before `after`
            if pos_before > pos_after {
                let element = result.remove(pos_before);
                result.insert(pos_after, element);
                *last_index = edge_idx;
                changed = true;
            }
        }

        changed
    }
}

#[must_use]
pub fn get_graph_data(
    plugins: &[PluginData],
    order_rules: &[EOrderRule],
    _warn_rules: &[EWarningRule],
) -> GraphData {
    // build hashmaps for lookup
    let mut index_dict: HashMap<String, usize> = HashMap::new();
    let mut index_dict_rev: HashMap<usize, String> = HashMap::default();

    for (i, plugin_data) in plugins.iter().enumerate() {
        let lower_case = plugin_data.name.to_lowercase();

        index_dict.insert(lower_case.clone(), i);
        index_dict_rev.insert(i, lower_case);
    }

    // add edges from order rules
    let mods = plugins
        .iter()
        .map(|f| f.name.to_lowercase())
        .collect::<Vec<String>>();

    let order_pairs = get_ordering_from_order_rules(order_rules);
    let mut edges: Vec<(usize, usize)> = vec![];
    for (a, b) in order_pairs {
        if let Some(results_for_a) = wild_contains(&mods, &a)
            && let Some(results_for_b) = wild_contains(&mods, &b)
        {
            // foreach esp i, add an edge to all esps j
            for i in &results_for_a {
                for j in &results_for_b {
                    if i == j {
                        warn!("Skipping circular edge: {i}");
                        continue;
                    }
                    let idx_a = index_dict[i.as_str()];
                    let idx_b = index_dict[j.as_str()];

                    if !edges.contains(&(idx_a, idx_b)) {
                        edges.push((idx_a, idx_b));
                    }
                }
            }
        }
    }

    // add edges from masters
    let master_start = edges.len();
    for mod_data in plugins {
        // add an edge from the mod to all its masters
        let idx = index_dict[&mod_data.name.to_lowercase()];
        if let Some(masters) = &mod_data.masters {
            for (master, _hash) in masters {
                let master = master.to_lowercase();
                if let Some(results) = wild_contains(&mods, &master) {
                    for result in results {
                        let idx_master = index_dict[&result];
                        let edge = (idx_master, idx);
                        if !edges.contains(&edge) {
                            edges.push(edge);
                        }
                    }
                }
            }
        }
    }

    let skipped = break_cycles(index_dict_rev.len(), &mut edges, master_start);
    for (a, b) in &skipped {
        warn!(
            "Ignoring order rule {} -> {} because it contradicts other rules",
            index_dict_rev[a], index_dict_rev[b]
        );
    }

    GraphData {
        index_dict,
        index_dict_rev,
        edges,
        skipped,
    }
}

/// Removes edges that would close a cycle, like mlox does. Returns the removed edges.
///
/// Master edges (from `master_start` on) are kept first, then order rule edges in rule order.
/// Only edges inside a strongly connected component can be part of a cycle, so all other
/// edges are kept without further checks.
fn break_cycles(
    node_count: usize,
    edges: &mut Vec<(usize, usize)>,
    master_start: usize,
) -> Vec<(usize, usize)> {
    let mut full = StableGraph::<(), ()>::with_capacity(node_count, edges.len());
    for _ in 0..node_count {
        full.add_node(());
    }
    for (a, b) in edges.iter() {
        full.add_edge(NodeIndex::new(*a), NodeIndex::new(*b), ());
    }

    let mut component = vec![usize::MAX; node_count];
    let mut cyclic = false;
    for (id, scc) in petgraph::algo::tarjan_scc(&full).iter().enumerate() {
        if scc.len() > 1 {
            cyclic = true;
            for n in scc {
                component[n.index()] = id;
            }
        }
    }
    if !cyclic {
        return vec![];
    }
    let in_cycle =
        |(a, b): &(usize, usize)| component[*a] != usize::MAX && component[*a] == component[*b];

    // the acyclic part
    let mut graph = StableGraph::<(), ()>::with_capacity(node_count, edges.len());
    for _ in 0..node_count {
        graph.add_node(());
    }
    for edge in edges.iter().filter(|e| !in_cycle(e)) {
        graph.add_edge(NodeIndex::new(edge.0), NodeIndex::new(edge.1), ());
    }

    let candidates = edges[master_start..]
        .iter()
        .chain(&edges[..master_start])
        .copied()
        .filter(in_cycle)
        .collect::<Vec<_>>();
    let mut space = petgraph::algo::DfsSpace::new(&graph);
    let mut skipped = vec![];
    for (a, b) in candidates {
        let (from, to) = (NodeIndex::new(a), NodeIndex::new(b));
        if petgraph::algo::has_path_connecting(&graph, to, from, Some(&mut space)) {
            skipped.push((a, b));
        } else {
            graph.add_edge(from, to, ());
        }
    }

    edges.retain(|e| !skipped.contains(e));
    skipped
}

/// Checks that `result` satisfies every edge of the graph
#[must_use]
pub fn check_graph_order(result: &[String], data: &GraphData) -> bool {
    let position: HashMap<String, usize> = result
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_lowercase(), i))
        .collect();
    data.edges.iter().all(|(a, b)| {
        match (
            position.get(&data.index_dict_rev[a]),
            position.get(&data.index_dict_rev[b]),
        ) {
            (Some(pos_a), Some(pos_b)) => pos_a < pos_b,
            _ => true,
        }
    })
}

#[must_use]
pub fn build_graph(data: &GraphData) -> StableGraph<String, ()> {
    let GraphData {
        index_dict_rev,
        edges,
        ..
    } = data;

    // create graph from edges
    let mut g = StableGraph::<String, ()>::with_capacity(index_dict_rev.len(), edges.len());
    for n in 0..index_dict_rev.len() {
        let name = index_dict_rev[&n].clone();
        g.add_node(name);
    }
    // add edges
    for edge in edges {
        g.add_edge(NodeIndex::new(edge.0), NodeIndex::new(edge.1), ());
    }

    g
}

/// Moves plugins matching `NearStart` rules to the start and `NearEnd` rules to the end
fn apply_near_rules(mods: &mut Vec<String>, order_rules: &[EOrderRule]) {
    for nearstart in order_rules
        .iter()
        .filter_map(nearstart2)
        .flat_map(|f| f.names)
        .rev()
    {
        for r in wild_contains(mods, &nearstart).unwrap_or_default() {
            if let Some(index) = mods.iter().position(|f| f == &r) {
                let element = mods.remove(index);
                mods.insert(0, element);
            }
        }
    }

    for nearend in order_rules
        .iter()
        .filter_map(nearend2)
        .flat_map(|f| f.names)
        .rev()
    {
        for r in wild_contains(mods, &nearend).unwrap_or_default() {
            if let Some(index) = mods.iter().position(|f| f == &r) {
                let element = mods.remove(index);
                mods.push(element);
            }
        }
    }
}

/// Puts master files (`.esm`, `.omwgame`) first, keeping their order, with `Morrowind.esm` at the very start
fn move_masters_first(mods: &mut Vec<String>) {
    let is_master = |m: &String| {
        let ext = std::path::Path::new(m).extension();
        ext.is_some_and(|e| e.eq_ignore_ascii_case("esm") || e.eq_ignore_ascii_case("omwgame"))
    };
    let (mut masters, rest): (Vec<String>, Vec<String>) = mods.drain(..).partition(is_master);
    if let Some(index) = masters.iter().position(|f| f == "morrowind.esm") {
        let element = masters.remove(index);
        masters.insert(0, element);
    }
    mods.extend(masters);
    mods.extend(rest);
}
