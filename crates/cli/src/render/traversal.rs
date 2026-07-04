//! `openlore graph traverse` — the bounded edge-traversal tree.

use super::*;

/// Content-frozen Gate-5 honesty notice for the `--traverse` view: every
/// displayed edge maps to exactly one signed claim; the recursive CTE selects
/// FROM existing rows only and never fabricates or interpolates an edge
/// (I-GRAPH-5). Do NOT paraphrase — the exact string is the user-visible
/// contract (US-GRAPH-004 Example 1 / Gate 5).
pub const TRAVERSAL_INVENTS_NO_EDGES_NOTICE: &str = "Traversal does not invent edges.";

/// Content-frozen line the renderer emits when a traversal surfaces NO
/// connecting (cross-project) edges within the bound (GQE-21 / US-GRAPH-004
/// Example 2). Emitted in two honest cases: (a) the seed reaches zero edges at
/// all, and (b) the seed reaches edges but NO contributor's claims span more
/// than one project (a lone author on a lone project triangulates with
/// nothing — there is no non-obvious connection to surface). The `{depth}`
/// placeholder is filled with the bound so the honest "nothing found, nothing
/// fabricated" message names the depth searched. Do NOT paraphrase.
const TRAVERSAL_NO_EDGES_TEMPLATE: &str = "No connecting edges found at depth {depth}.";

/// Render the `graph query --object <philosophy> --traverse` result: a bounded,
/// cycle-safe tree from the queried philosophy to the projects that embody it
/// and the contributors who authored those claims, plus a "Connections found"
/// callout naming any contributor whose claims span MORE THAN ONE project (the
/// non-obvious cross-project connection — KPI-GRAPH-1). Pure function — no I/O,
/// no storage access.
///
/// ## Gate 5 / I-GRAPH-5 (traversal invents no edges)
///
/// Every [`GraphEdge`] in `result.edges` carries its backing `claim_cid`
/// (non-`Option`) and `author_did` (non-`Option`). The renderer surfaces BOTH
/// on each edge row, so an operator can trace any displayed edge to exactly one
/// signed claim, and emits the content-frozen
/// [`TRAVERSAL_INVENTS_NO_EDGES_NOTICE`]. Two claims about the same project by
/// different authors render as TWO edge rows (never merged; anti-merging WD-73).
///
/// `max_depth` is the bound the walk used (WD-76); it labels the
/// no-edges-found message when the seed is isolated and frames the
/// omitted-edge report.
pub fn render_traversal_tree(object: &str, result: &TraversalResult, max_depth: u8) -> String {
    render_traversal_from_seed(&format!("philosophy: {object}"), result, max_depth)
}

/// Render the `graph query --contributor <did> --traverse` result (GQE-22 /
/// US-GRAPH-004 Example 3): the SAME bounded, cycle-safe tree as
/// [`render_traversal_tree`], but seeded at a contributor rather than a
/// philosophy. The walk anchors on the contributor's own claims (depth 1) and
/// fans across the projects those claims share with other contributors; the
/// header names the seed contributor. The WD-76 omitted-edge report
/// ("Showing depth N; M edge(s) omitted. Use `--depth N+1` to go deeper.") and
/// the content-frozen Gate-5 notice render identically. Pure function.
pub fn render_traversal_contributor_tree(
    contributor: &str,
    result: &TraversalResult,
    max_depth: u8,
) -> String {
    render_traversal_from_seed(&format!("contributor: {contributor}"), result, max_depth)
}

/// Shared traversal-tree renderer core, parameterized by the seed `header` line
/// (a philosophy seed renders `philosophy: <object>`, a contributor seed
/// `contributor: <did>`). The tree body, the WD-76 omitted-edge report, the
/// KPI-GRAPH-1 "Connections found" callout, and the content-frozen Gate-5
/// honesty notice are identical regardless of seed. Pure helper.
pub(crate) fn render_traversal_from_seed(
    header: &str,
    result: &TraversalResult,
    max_depth: u8,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{header}\n\n"));

    if result.edges.is_empty() {
        // Honest empty (GQE-21): name the depth searched; fabricate no
        // connection. The Gate-5 notice still frames the (empty) result.
        out.push_str(&format!(
            "{}\n\n",
            TRAVERSAL_NO_EDGES_TEMPLATE.replace("{depth}", &max_depth.to_string())
        ));
        out.push_str(&format!("{TRAVERSAL_INVENTS_NO_EDGES_NOTICE}\n"));
        return out;
    }

    // The tree, grouped by project (first-seen order — stable output). Each
    // project heads a group; under it, one edge row per backing signed claim
    // carrying its author DID + claim_cid (every edge independently auditable).
    for (subject, project_edges) in &group_edges_by_project(&result.edges) {
        out.push_str(&format!("  project: {subject}\n"));
        for edge in project_edges {
            out.push_str(&render_one_traversal_edge(edge));
        }
        out.push('\n');
    }

    // WD-76: if the bound omitted deeper edges, report how many + how to widen.
    if result.omitted_edge_count > 0 {
        out.push_str(&format!(
            "Showing depth {max_depth}; {} edge(s) omitted. Use `--depth {}` to go deeper.\n\n",
            result.omitted_edge_count,
            max_depth.saturating_add(1)
        ));
    }

    // KPI-GRAPH-1: the non-obvious connection — any contributor whose claims
    // span MORE THAN ONE project. The callout names each such contributor and
    // the exact projects they triangulate across. When NO contributor spans
    // multiple projects (e.g. a lone author on a lone project — GQE-21), there
    // is no non-obvious connection to surface: state that HONESTLY rather than
    // silently omitting the callout, and fabricate nothing (Gate 5 / I-GRAPH-5).
    let callout = render_connections_callout(&result.edges);
    if callout.is_empty() {
        out.push_str(&format!(
            "{}\n\n",
            TRAVERSAL_NO_EDGES_TEMPLATE.replace("{depth}", &max_depth.to_string())
        ));
    } else {
        out.push_str(&callout);
    }

    // Gate 5: the content-frozen honesty notice.
    out.push_str(&format!("{TRAVERSAL_INVENTS_NO_EDGES_NOTICE}\n"));
    out
}

/// Group traversal edges by their project (`to` node subject), preserving
/// first-seen order. Pure helper.
pub(crate) fn group_edges_by_project(edges: &[GraphEdge]) -> Vec<(String, Vec<&GraphEdge>)> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: Vec<(String, Vec<&GraphEdge>)> = Vec::new();
    for edge in edges {
        let subject = project_subject(edge);
        match order.iter().position(|s| s == &subject) {
            Some(pos) => grouped[pos].1.push(edge),
            None => {
                order.push(subject.clone());
                grouped.push((subject, vec![edge]));
            }
        }
    }
    grouped
}

/// Render one traversal edge under its project group: the contributor (author
/// DID) who authored the backing claim, and the backing `claim_cid` (Gate 5 —
/// every edge maps to exactly one signed claim). Pure helper.
pub(crate) fn render_one_traversal_edge(edge: &GraphEdge) -> String {
    let mut out = String::new();
    out.push_str(&format!("    author_did: {}\n", edge.author_did.0));
    out.push_str(&format!("    claim_cid:  {}\n", edge.claim_cid.0));
    out
}

/// Render the "Connections found" callout (KPI-GRAPH-1 north star): for every
/// contributor whose edges span MORE THAN ONE distinct project, emit a line
/// naming the contributor and the projects they triangulate across — the
/// non-obvious cross-project connection the traversal surfaces. When no
/// contributor spans multiple projects, the callout is omitted entirely (there
/// is no non-obvious connection to surface). Pure helper.
pub(crate) fn render_connections_callout(edges: &[GraphEdge]) -> String {
    let spanning = contributors_spanning_multiple_projects(edges);
    if spanning.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("Connections found:\n");
    for (author_did, projects) in spanning {
        out.push_str(&format!(
            "  {author_did} spans {} of these projects ({}) -> a contributor whose \
             dependency-pinning claims triangulate across projects.\n",
            projects.len(),
            projects.join(", ")
        ));
    }
    out.push('\n');
    out
}

/// The contributors whose traversal edges span MORE THAN ONE distinct project,
/// each paired with the projects (first-seen order) they span. The
/// cross-project connection KPI-GRAPH-1 surfaces. Pure helper.
pub(crate) fn contributors_spanning_multiple_projects(
    edges: &[GraphEdge],
) -> Vec<(String, Vec<String>)> {
    let mut order: Vec<String> = Vec::new();
    let mut by_author: Vec<(String, Vec<String>)> = Vec::new();
    for edge in edges {
        let author = edge.author_did.0.clone();
        let subject = project_subject(edge);
        let pos = match order.iter().position(|a| a == &author) {
            Some(pos) => pos,
            None => {
                order.push(author.clone());
                by_author.push((author, Vec::new()));
                by_author.len() - 1
            }
        };
        if !by_author[pos].1.contains(&subject) {
            by_author[pos].1.push(subject);
        }
    }
    by_author
        .into_iter()
        .filter(|(_, projects)| projects.len() > 1)
        .collect()
}

/// The project subject an edge points at (its `to` node). Defensive fallback to
/// the empty string for a non-project `to` node (the traversal only emits
/// philosophy→project edges, but the match stays total). Pure helper.
pub(crate) fn project_subject(edge: &GraphEdge) -> String {
    match &edge.to {
        GraphNode::Project { subject } => subject.clone(),
        GraphNode::Philosophy { object } => object.clone(),
        GraphNode::Contributor { author_did } => author_did.0.clone(),
    }
}
