//! Helpers that flatten the accesskit tree into MCP-friendly shapes.
//!
//! Note: `accesskit_consumer::NodeId` is a private composite (tree-index + local-id) and
//! can't be constructed from outside the crate. We project everything externally as the
//! original `accesskit::NodeId` (a `pub u64`), and look up by walking the tree.

use accesskit_consumer::{Node, Tree};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `accesskit::Role::Unknown`, as `{:?}` spells it. What egui reports for a widget that
/// declares no kind (`WidgetType::Other`), so it tells an agent nothing.
const UNKNOWN_ROLE: &str = "Unknown";

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NodeView {
    /// Node id, used with `click`, `type_text`, and `get_node`.
    pub id: String,
    pub role: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub bounds: Option<RectF>,
    pub focused: bool,
    pub disabled: bool,
    pub hidden: bool,
    pub parent_id: Option<String>,
}

/// A node in `query_tree`'s hierarchical result.
///
/// Only matching nodes appear; each one nests the matches found in its own subtree. A
/// non-matching node contributes its matches to its nearest matching ancestor, so a `children`
/// entry is not necessarily a direct child in the app's tree.
///
/// Deliberately leaner than [`NodeView`]: a whole tree of `bounds` is a lot of numbers for an
/// agent to read past, and actions take an `id` anyway. `get_node` has the full detail.
///
/// A node that says nothing — no children, no `label`, no `value` and no role — is dropped
/// entirely.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TreeNode {
    /// Node id, used with `click`, `type_text`, and `get_node`.
    pub id: String,

    /// Omitted for a role of `Unknown`, which carries no information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Each flag is omitted when false — that is the state of nearly every node.
    #[serde(default, skip_serializing_if = "is_false")]
    pub focused: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,

    /// `default` keeps the derived schema honest: a leaf omits the field entirely.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Self>,
}

impl TreeNode {
    /// A childless node with no `label`, no `value` and no role says nothing an agent can act
    /// on or read — it is layout scaffolding that survived the filter. `query_tree` drops it.
    fn is_noise(&self) -> bool {
        self.children.is_empty()
            && self.label.is_none()
            && self.value.is_none()
            && self.role.is_none()
    }
}

/// For `#[serde(skip_serializing_if)]`.
#[expect(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl RectF {
    /// Build from `AccessKit` physical-pixel bounds, scaling to logical points (the consumer's
    /// `bounding_box` applies egui's root `scale(pixels_per_point)` transform, so bounds arrive
    /// in physical pixels — divide them back out).
    fn from_physical(r: accesskit::Rect, pixels_per_point: f32) -> Self {
        let s = 1.0 / f64::from(pixels_per_point);
        Self {
            x: r.x0 * s,
            y: r.y0 * s,
            w: (r.x1 - r.x0) * s,
            h: (r.y1 - r.y0) * s,
        }
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// The widget-matching constraints shared by `query_tree`'s filter and the action `Target`s.
///
/// An optional `role` plus up to one text predicate. All are case-insensitive and combined with
/// logical AND; an all-`None` `Query` matches every node.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct Query {
    /// Case-insensitive substring match against *either* `label` or `value`. Prefer this when you
    /// just want "the widget showing this text" and don't care which field holds it — it's the
    /// most robust choice across widget kinds.
    pub content_contains: Option<String>,
    /// Role name, e.g. `Button`, `Label`, `TextInput` (case-insensitive).
    /// An unrecognized role is rejected with an error that lists the roles present in the tree.
    pub role: Option<String>,
    /// Case-insensitive substring match against the node's `label` (its accessible name) only.
    /// Note that `Label`/monospace widgets carry their text in `value`, not `label` — for those,
    /// use `content_contains` (or `value_contains`).
    pub label_contains: Option<String>,
    /// Case-insensitive substring match against the node's `value` only (e.g. a text field's
    /// contents, or a `Label`'s text).
    pub value_contains: Option<String>,
}

impl Query {
    /// True when no constraint is set (matches every node).
    pub fn is_empty(&self) -> bool {
        self.content_contains.is_none()
            && self.role.is_none()
            && self.label_contains.is_none()
            && self.value_contains.is_none()
    }

    /// Does `node` satisfy every set constraint? (Visibility is the caller's concern.)
    fn matches(&self, node: &Node<'_>) -> bool {
        if let Some(needle) = &self.content_contains
            && !contains_ci(&node.label().unwrap_or_default(), needle)
            && !contains_ci(&node.value().unwrap_or_default(), needle)
        {
            return false;
        }
        if let Some(role) = &self.role
            && !role.eq_ignore_ascii_case(&format!("{:?}", node.role()))
        {
            return false;
        }
        if let Some(needle) = &self.label_contains
            && !contains_ci(&node.label().unwrap_or_default(), needle)
        {
            return false;
        }
        if let Some(needle) = &self.value_contains
            && !contains_ci(&node.value().unwrap_or_default(), needle)
        {
            return false;
        }
        true
    }

    /// Human-readable description of the constraints, for "matched N nodes" / "no node" errors.
    fn describe(&self) -> String {
        let mut clauses = Vec::new();
        if let Some(c) = &self.content_contains {
            clauses.push(format!("label or value containing `{c}`"));
        }
        if let Some(r) = &self.role {
            clauses.push(format!("role `{r}`"));
        }
        if let Some(l) = &self.label_contains {
            clauses.push(format!("label containing `{l}`"));
        }
        if let Some(v) = &self.value_contains {
            clauses.push(format!("value containing `{v}`"));
        }
        if clauses.is_empty() {
            "the locator".to_owned()
        } else {
            clauses.join(" and ")
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct QueryFilter {
    #[serde(flatten)]
    pub query: Query,
    #[serde(default = "default_true")]
    pub visible_only: bool,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_true() -> bool {
    true
}

fn default_limit() -> usize {
    200
}

impl Default for QueryFilter {
    fn default() -> Self {
        Self {
            query: Query::default(),
            visible_only: true,
            limit: default_limit(),
        }
    }
}

pub fn query(tree: &Tree, filter: &QueryFilter, pixels_per_point: f32) -> Vec<TreeNode> {
    let root = tree.state().root();
    let mut nodes = walk(&root, filter, pixels_per_point);
    let mut budget = filter.limit;
    truncate(&mut nodes, &mut budget);
    nodes
}

/// The matches in `node`'s subtree (including `node` itself), nested by ancestry.
///
/// A non-matching node returns its descendants' matches, which its own parent then adopts.
fn walk(node: &Node<'_>, filter: &QueryFilter, pixels_per_point: f32) -> Vec<TreeNode> {
    let children: Vec<TreeNode> = node
        .children()
        .flat_map(|child| walk(&child, filter, pixels_per_point))
        .collect();
    if matches(node, filter) {
        let view = tree_node(node, children, pixels_per_point);
        // Pruning runs bottom-up, so a node left childless by it is reconsidered here in turn.
        if view.is_noise() {
            Vec::new()
        } else {
            vec![view]
        }
    } else {
        children
    }
}

/// Keep at most `budget` nodes, depth-first; a dropped node takes its subtree with it.
fn truncate(nodes: &mut Vec<TreeNode>, budget: &mut usize) {
    let mut kept = 0;
    for node in nodes.iter_mut() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        kept += 1;
        truncate(&mut node.children, budget);
    }
    nodes.truncate(kept);
}

/// Total number of nodes in a forest of [`TreeNode`]s.
pub fn count(nodes: &[TreeNode]) -> usize {
    nodes.iter().map(|node| 1 + count(&node.children)).sum()
}

/// Validate a `role` filter string against the full `AccessKit` role set.
///
/// Compared case-insensitively, the way `matches` compares. On failure the error lists the
/// distinct roles actually present in `tree`, so the agent learns what it can filter by instead of
/// getting a silent empty result. Validity is checked against *all* roles, not just those present,
/// so polling tools like `wait_for` can still wait for a valid role that hasn't appeared yet.
///
/// # Errors
/// If `role` is not a known `AccessKit` role name.
pub fn validate_role(role: &str, tree: Option<&Tree>) -> Result<(), String> {
    // `accesskit::Role` is `#[repr(u8)]` with `enumn::N`, so walking `n(0), n(1), …` until `None`
    // enumerates every variant; `{:?}` yields the same name `matches`/`node_view` expose.
    let valid = (0u8..=u8::MAX)
        .map_while(accesskit::Role::n)
        .any(|r| role.eq_ignore_ascii_case(&format!("{r:?}")));
    if valid {
        return Ok(());
    }
    let present = tree.map(roles_in_tree).unwrap_or_default();
    let hint = if present.is_empty() {
        "(no nodes in the current tree)".to_owned()
    } else {
        present.join(", ")
    };
    Err(format!(
        "unknown role `{role}` — roles present in the current tree: {hint}"
    ))
}

/// The distinct `AccessKit` roles present anywhere in `tree`, sorted, as their display names.
fn roles_in_tree(tree: &Tree) -> Vec<String> {
    let mut roles = std::collections::BTreeSet::new();
    collect_roles(&tree.state().root(), &mut roles);
    roles.into_iter().collect()
}

fn collect_roles(node: &Node<'_>, out: &mut std::collections::BTreeSet<String>) {
    out.insert(format!("{:?}", node.role()));
    for child in node.children() {
        collect_roles(&child, out);
    }
}

fn matches(node: &Node<'_>, filter: &QueryFilter) -> bool {
    if filter.visible_only && node.is_hidden() {
        return false;
    }
    filter.query.matches(node)
}

/// Case-insensitive substring test (ASCII-folded, matching how `role` is compared).
fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

pub fn node_view(node: &Node<'_>, pixels_per_point: f32) -> NodeView {
    NodeView {
        id: format_id(accesskit_id(node)),
        role: format!("{:?}", node.role()),
        label: node.label(),
        value: node.value(),
        bounds: node
            .bounding_box()
            .map(|r| RectF::from_physical(r, pixels_per_point)),
        focused: node.is_focused_in_tree(),
        disabled: node.is_disabled(),
        hidden: node.is_hidden(),
        parent_id: node.parent().map(|p| format_id(accesskit_id(&p))),
    }
}

fn tree_node(node: &Node<'_>, children: Vec<TreeNode>, pixels_per_point: f32) -> TreeNode {
    let NodeView {
        id,
        role,
        label,
        value,
        bounds: _,
        focused,
        disabled,
        hidden,
        parent_id: _,
    } = node_view(node, pixels_per_point);
    TreeNode {
        id,
        role: (role != UNKNOWN_ROLE).then_some(role),
        label,
        value,
        focused,
        disabled,
        hidden,
        children,
    }
}

/// Format a node id the way the tools expose it: lower-case hex, no prefix.
///
/// Hex keeps the ids short, which matters because a `query_tree` result is mostly ids.
pub fn format_id(id: u64) -> String {
    format!("{id:x}")
}

/// Parse a node id as produced by [`format_id`].
pub fn parse_id(id: &str) -> Option<u64> {
    u64::from_str_radix(id.trim(), 16).ok()
}

/// Project a consumer node to its original `accesskit::NodeId` as a `u64`.
pub fn accesskit_id(node: &Node<'_>) -> u64 {
    let (local, _tree) = node.locate();
    local.0
}

/// A resolved lookup target: a specific node `id`, or a role/text match.
/// Built directly by the tools from a `Target` — never deserialized.
#[derive(Debug, Clone)]
pub enum Locator {
    Id { id: u64 },
    Match { query: Query },
}

impl Locator {
    /// Build a locator from raw tool fields: a parseable `id` wins, else the `query` constraints.
    /// Returns `None` when neither an `id` nor any `query` constraint is set.
    pub fn from_fields(id: Option<&str>, query: Query) -> Option<Self> {
        if let Some(id) = id.and_then(parse_id) {
            return Some(Self::Id { id });
        }
        if !query.is_empty() {
            return Some(Self::Match { query });
        }
        None
    }
}

/// Resolve a locator to *exactly one* node for an action (click, focus, …).
///
/// Like kittest's `get_by_*`, this is strict: an ambiguous locator is an error, not a silent
/// "first match wins". A specific `id` resolves at most one node; a `role`/text match
/// errors if it hits zero or more than one node, listing the candidates so the caller can narrow
/// the filter or target a specific `id`. Use `query_tree` (which returns all matches) when you
/// genuinely expect several.
///
/// # Errors
/// If no node matches, or if more than one does.
pub fn resolve_unique<'a>(
    tree: &'a Tree,
    locator: &Locator,
    pixels_per_point: f32,
) -> Result<Node<'a>, String> {
    let root = tree.state().root();
    match locator {
        Locator::Id { id } => {
            let mut found = Vec::new();
            find_all(&root, &|n| accesskit_id(n) == *id, &mut found);
            one(found, pixels_per_point, &format!("id `{}`", format_id(*id)))
        }
        Locator::Match { query } => {
            let filter = QueryFilter {
                query: query.clone(),
                visible_only: true,
                limit: usize::MAX,
            };
            let mut found = Vec::new();
            find_all(&root, &|n| matches(n, &filter), &mut found);
            one(found, pixels_per_point, &query.describe())
        }
    }
}

/// Reduce a match list to the single node an action needs, or an error describing the miss.
fn one<'a>(
    mut found: Vec<Node<'a>>,
    pixels_per_point: f32,
    what: &str,
) -> Result<Node<'a>, String> {
    match found.len() {
        0 => Err(format!("no node found matching {what}")),
        1 => Ok(found.remove(0)),
        n => {
            let views: Vec<NodeView> = found
                .iter()
                .map(|node| node_view(node, pixels_per_point))
                .collect();
            let list =
                serde_json::to_string_pretty(&views).unwrap_or_else(|_| format!("{n} nodes"));
            Err(format!(
                "{what} matched {n} nodes — narrow the locator (`content_contains`/`role`/`label_contains`/`value_contains`), or target a specific `id`. Matches:\n{list}"
            ))
        }
    }
}

/// Depth-first collection of every node satisfying `pred`.
fn find_all<'a>(node: &Node<'a>, pred: &impl Fn(&Node<'_>) -> bool, out: &mut Vec<Node<'a>>) {
    if pred(node) {
        out.push(*node);
    }
    for child in node.children() {
        find_all(&child, pred, out);
    }
}

#[cfg(test)]
mod tests {
    use accesskit::{Node as AkNode, NodeId, Role, Tree as AkTree, TreeId, TreeUpdate};

    use super::*;

    /// `root(Window) → [scaffold(Unknown) → [button(Button "OK")], text(Unknown "hi"),
    /// valued(Unknown, value "42"), noise(Unknown)]`
    fn test_tree() -> Tree {
        let mut root = AkNode::new(Role::Window);
        root.set_children(vec![NodeId(0x2), NodeId(0xff), NodeId(0x5), NodeId(0x4)]);
        let mut scaffold = AkNode::new(Role::Unknown);
        scaffold.set_children(vec![NodeId(0x3)]);
        let mut button = AkNode::new(Role::Button);
        button.set_label("OK");
        let mut text = AkNode::new(Role::Unknown);
        text.set_label("hi");
        let mut valued = AkNode::new(Role::Unknown);
        valued.set_value("42");
        let noise = AkNode::new(Role::Unknown);

        Tree::new(
            TreeUpdate {
                nodes: vec![
                    (NodeId(0x1), root),
                    (NodeId(0x2), scaffold),
                    (NodeId(0x3), button),
                    (NodeId(0xff), text),
                    (NodeId(0x5), valued),
                    (NodeId(0x4), noise),
                ],
                tree: Some(AkTree::new(NodeId(0x1))),
                tree_id: TreeId::ROOT,
                focus: NodeId(0x1),
            },
            false,
        )
    }

    fn query_all(filter: &QueryFilter) -> Vec<TreeNode> {
        query(&test_tree(), filter, 1.0)
    }

    #[test]
    fn unfiltered_query_keeps_the_hierarchy_and_drops_scaffolding() {
        let nodes = query_all(&QueryFilter::default());
        assert_eq!(nodes.len(), 1, "one root");
        let root = &nodes[0];
        assert_eq!(root.id, "1");
        assert_eq!(root.role.as_deref(), Some("Window"));
        // `noise` is childless, label-less and role-less, so it's gone; `scaffold` survives
        // despite being all three, because it still has a child.
        let ids: Vec<&str> = root.children.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["2", "ff", "5"], "`valued` is kept for its text alone");
        assert_eq!(root.children[0].role, None, "`Unknown` is omitted");
        assert_eq!(root.children[0].children[0].id, "3");
        assert_eq!(count(&nodes), 5);
    }

    #[test]
    fn a_filter_lifts_matches_past_their_unmatched_ancestors() {
        let nodes = query_all(&QueryFilter {
            query: Query {
                role: Some("button".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "3", "the button, not its scaffold or the root");
        assert!(nodes[0].children.is_empty());
    }

    #[test]
    fn limit_counts_every_node_and_takes_subtrees_with_it() {
        let nodes = query_all(&QueryFilter {
            limit: 2,
            ..Default::default()
        });
        // root + `scaffold`, so the button is cut off below the limit.
        assert_eq!(count(&nodes), 2);
        assert!(nodes[0].children[0].children.is_empty());

        assert!(
            query_all(&QueryFilter {
                limit: 0,
                ..Default::default()
            })
            .is_empty()
        );
    }

    /// The JSON an agent actually receives.
    ///
    /// Each test above pins one rule; this pins the shape they add up to, so a change to the
    /// output reads as a diff instead of having to be reconstructed from the assertions.
    #[test]
    fn the_query_tree_json_is_what_an_agent_reads() {
        fn pretty(nodes: &[TreeNode]) -> String {
            serde_json::to_string_pretty(nodes).expect("serialize")
        }

        insta::assert_snapshot!(
            "query_tree_unfiltered",
            pretty(&query_all(&QueryFilter::default()))
        );

        // A filter lifts its matches out of the hierarchy, which is the case worth seeing whole.
        insta::assert_snapshot!(
            "query_tree_filtered",
            pretty(&query_all(&QueryFilter {
                query: Query {
                    role: Some("button".to_owned()),
                    ..Default::default()
                },
                ..Default::default()
            }))
        );
    }

    #[test]
    fn a_tree_node_serializes_without_its_empty_fields() {
        fn keys(node: &TreeNode) -> Vec<String> {
            let json = serde_json::to_value(node).expect("serialize");
            json.as_object().expect("object").keys().cloned().collect()
        }

        let nodes = query_all(&QueryFilter::default());
        let scaffold = &nodes[0].children[0];
        assert_eq!(
            keys(scaffold),
            ["children", "id"],
            "no label, no value, no role, and every flag false"
        );
        assert_eq!(
            keys(&scaffold.children[0]),
            ["id", "label", "role"],
            "a leaf carries no `children`"
        );
    }
}
