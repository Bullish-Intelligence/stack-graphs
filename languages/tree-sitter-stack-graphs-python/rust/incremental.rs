//! Bounded, pack-owned Python lowering for incremental semantic prototypes.
//!
//! The lowering uses the pinned Tree-sitter Python grammar. It records imports,
//! classes, functions, parameters, simple assignments, returns, names, calls,
//! attributes, and conditional expressions. It supports nested calls and multiple
//! returns, including recursive call structure. It does not execute Python or
//! infer arbitrary runtime values. Unsupported statement forms are retained as
//! incomplete diagnostics. Syntax errors also produce an incomplete document.
//!
//! Semantic item identities use module name, lexical scope, item kind, name,
//! and a per-scope ordinal. They do not contain source ranges. Current ranges
//! live only in [`SourceMap`], so range changes do not change semantic equality.
//! Document paths distinguish same-named declarations in separate files.
//!
//! ```
//! use tree_sitter_stack_graphs_python::incremental::lower_incremental_document;
//! let source = "def same():\n    return 1\n";
//! let left = lower_incremental_document("pkg.mod", "src/left.py", source);
//! let right = lower_incremental_document("pkg.mod", "src/right.py", source);
//! assert_ne!(left.fragment.items[0].id, right.fragment.items[0].id);
//! ```

use tree_sitter::{Node, Parser};

/// The owned semantic facts produced for one document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnedFragment {
    pub items: Vec<SemanticItem>,
}

/// One semantic fact. `id` and `parent_id` are stable semantic identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticItem {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: SemanticKind,
    pub name: Option<String>,
    pub expression: Option<Expression>,
    pub params: Vec<String>,
}

/// Semantic role of a lowered item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticKind {
    Import {
        module: String,
        symbol: Option<String>,
        alias: Option<String>,
    },
    Class,
    Function,
    Parameter {
        index: usize,
    },
    Assignment,
    Return,
    Name,
    Call,
    Attribute,
    Conditional,
}

/// Bounded expression tree used by the dedicated resolver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expression {
    Name(String),
    Literal(String),
    Call {
        callee: Box<Expression>,
        args: Vec<Expression>,
    },
    Attribute {
        receiver: Box<Expression>,
        name: String,
    },
    Tuple(Vec<Expression>),
    Conditional {
        branches: Vec<Expression>,
    },
    Unknown,
}

/// Source locations for semantic IDs. Coordinates use Tree-sitter's zero-based units.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceMap {
    pub entries: Vec<SourceMapEntry>,
}

/// Current source range for one semantic item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMapEntry {
    pub item_id: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize,
    pub start_column: usize,
    pub end_row: usize,
    pub end_column: usize,
}

/// Whether the lowerer understood all syntax that can affect its bounded facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completeness {
    Complete,
    Incomplete,
}

/// Why a document has incomplete semantic facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    SyntaxError,
    UnsupportedSyntax,
}

/// A syntax or subset diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub item_id: Option<String>,
    pub message: String,
    pub kind: DiagnosticKind,
}

/// Lowered output, with semantic equality and current source positions separated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredDocument {
    pub fragment: OwnedFragment,
    pub source_map: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
    pub completeness: Completeness,
}

/// Parse and lower a Python document with the crate's pinned real parser.
pub fn lower_incremental_document(module: &str, path: &str, source: &str) -> LoweredDocument {
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_err()
    {
        return LoweredDocument {
            fragment: OwnedFragment::default(),
            source_map: SourceMap::default(),
            diagnostics: vec![Diagnostic {
                item_id: None,
                message: "failed to configure Python parser".into(),
                kind: DiagnosticKind::SyntaxError,
            }],
            completeness: Completeness::Incomplete,
        };
    }
    let Some(tree) = parser.parse(source, None) else {
        return LoweredDocument {
            fragment: OwnedFragment::default(),
            source_map: SourceMap::default(),
            diagnostics: vec![Diagnostic {
                item_id: None,
                message: format!("parser returned no tree for {path}"),
                kind: DiagnosticKind::SyntaxError,
            }],
            completeness: Completeness::Incomplete,
        };
    };
    let root = tree.root_node();
    let tree_has_syntax_errors = root.has_error();
    let mut lowerer = Lowerer {
        source,
        fragment: OwnedFragment::default(),
        source_map: SourceMap::default(),
        diagnostics: Vec::new(),
        ordinals: Default::default(),
    };
    let document_scope = format!(
        "module:{}:{}|path:{}:{}",
        module.len(),
        module,
        path.len(),
        path
    );
    lowerer.visit(root, None, &document_scope);
    if tree_has_syntax_errors
        && !lowerer
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::SyntaxError)
    {
        lowerer.diagnostics.push(Diagnostic {
            item_id: None,
            message: "Tree-sitter reported syntax errors in this document".into(),
            kind: DiagnosticKind::SyntaxError,
        });
    }
    let completeness = if lowerer.diagnostics.is_empty() {
        Completeness::Complete
    } else {
        Completeness::Incomplete
    };
    LoweredDocument {
        fragment: lowerer.fragment,
        source_map: lowerer.source_map,
        diagnostics: lowerer.diagnostics,
        completeness,
    }
}

struct Lowerer<'a> {
    source: &'a str,
    fragment: OwnedFragment,
    source_map: SourceMap,
    diagnostics: Vec<Diagnostic>,
    ordinals: std::collections::HashMap<String, usize>,
}

impl Lowerer<'_> {
    fn id(&mut self, parent: &str, kind: &str, name: &str) -> String {
        let key = format!("{parent}/{kind}:{name}");
        let ordinal = self.ordinals.entry(key.clone()).or_insert(0);
        let id = format!("{key}#{ordinal}");
        *ordinal += 1;
        id
    }

    fn add(
        &mut self,
        node: Node<'_>,
        parent: Option<&str>,
        scope: &str,
        kind: SemanticKind,
        tag: &str,
        name: Option<String>,
        expression: Option<Expression>,
        params: Vec<String>,
    ) -> String {
        let id = self.id(scope, tag, name.as_deref().unwrap_or(""));
        self.fragment.items.push(SemanticItem {
            id: id.clone(),
            parent_id: parent.map(str::to_owned),
            kind,
            name,
            expression,
            params,
        });
        let start = node.start_position();
        let end = node.end_position();
        self.source_map.entries.push(SourceMapEntry {
            item_id: id.clone(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_row: start.row,
            start_column: start.column,
            end_row: end.row,
            end_column: end.column,
        });
        id
    }

    fn text(&self, node: Node<'_>) -> String {
        self.source.get(node.byte_range()).unwrap_or("").to_owned()
    }

    fn visit(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        if node.is_error() || node.is_missing() {
            self.diagnostics.push(Diagnostic {
                item_id: parent.map(str::to_owned),
                message: if node.is_missing() {
                    "missing syntax".into()
                } else {
                    "invalid syntax".into()
                },
                kind: DiagnosticKind::SyntaxError,
            });
            // Tree-sitter can recover valid named descendants under an ERROR node.
            // Keep those facts while marking this document incomplete.
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    self.visit(child, parent, scope);
                }
            }
            return;
        }
        match node.kind() {
            "module" | "block" | "expression_statement" => {
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        self.visit(child, parent, scope);
                    }
                }
            }
            "function_definition" => self.function(node, parent, scope),
            "class_definition" => self.class(node, parent, scope),
            "import_statement" | "import_from_statement" => self.import(node, parent, scope),
            "assignment" => self.assignment(node, parent, scope),
            "return_statement" => self.return_stmt(node, parent, scope),
            "if_statement" => self.if_statement(node, parent, scope),
            "conditional_expression" => {
                let expr = self.expr(node);
                let id = self.add(
                    node,
                    parent,
                    scope,
                    SemanticKind::Conditional,
                    "conditional",
                    None,
                    Some(expr),
                    vec![],
                );
                if let Some(condition) = node.child_by_field_name("condition") {
                    self.visit_condition(condition, Some(&id), scope);
                }
                for field in ["consequence", "alternative"] {
                    if let Some(value) = node.child_by_field_name(field) {
                        let value_expr = self.expr(value);
                        self.expr_facts(value, Some(&id), scope, &value_expr);
                    }
                }
            }
            "if_clause" => self.if_clause(node, parent, scope),
            "elif_clause" => self.if_clause(node, parent, scope),
            "else_clause" => self.visit_children(node, parent, scope),
            "identifier" => {
                let s = self.text(node);
                self.add(
                    node,
                    parent,
                    scope,
                    SemanticKind::Name,
                    "name",
                    Some(s.clone()),
                    Some(Expression::Name(s)),
                    vec![],
                );
            }
            "call"
            | "attribute"
            | "integer"
            | "float"
            | "string"
            | "true"
            | "false"
            | "none"
            | "tuple"
            | "parenthesized_expression" => {
                let expr = self.expr(node);
                self.expr_facts(node, parent, scope, &expr);
            }
            "comment" | "decorator" => (),
            _ => {
                if node.is_named() && node.child_count() == 0 {
                    return;
                }
                self.diagnostics.push(Diagnostic {
                    item_id: parent.map(str::to_owned),
                    message: format!("unsupported Python syntax: {}", node.kind()),
                    kind: DiagnosticKind::UnsupportedSyntax,
                });
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        self.visit(child, parent, scope);
                    }
                }
            }
        }
    }

    fn function(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n))
            .unwrap_or_default();
        let id = self.add(
            node,
            parent,
            scope,
            SemanticKind::Function,
            "function",
            Some(name.clone()),
            None,
            vec![],
        );
        let child_scope = id.clone();
        if let Some(parameters) = node.child_by_field_name("parameters") {
            let mut params = Vec::new();
            for i in 0..parameters.named_child_count() {
                if let Some(p) = parameters.named_child(i) {
                    if p.kind() == "identifier" {
                        params.push(self.text(p));
                    } else if let Some(n) = p.child_by_field_name("name") {
                        params.push(self.text(n));
                    }
                }
            }
            if let Some(item) = self.fragment.items.iter_mut().find(|x| x.id == id) {
                item.params = params.clone();
            }
            for (index, p) in params.iter().enumerate() {
                let pnode = find_identifier(parameters, p, self.source);
                if let Some(pnode) = pnode {
                    self.add(
                        pnode,
                        Some(&id),
                        &child_scope,
                        SemanticKind::Parameter { index },
                        "parameter",
                        Some(p.clone()),
                        None,
                        vec![],
                    );
                }
            }
        }
        if let Some(body) = node.child_by_field_name("body") {
            self.visit(body, Some(&id), &child_scope);
        }
    }

    fn class(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n))
            .unwrap_or_default();
        let id = self.add(
            node,
            parent,
            scope,
            SemanticKind::Class,
            "class",
            Some(name.clone()),
            None,
            vec![],
        );
        if let Some(body) = node.child_by_field_name("body") {
            self.visit(body, Some(&id), &id);
        }
    }

    fn import(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        let text = self.text(node);
        let (module, symbols) = if let Some(rest) = text.strip_prefix("from ") {
            let mut parts = rest.splitn(2, " import ");
            (
                parts.next().unwrap_or("").trim().to_owned(),
                parts
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>(),
            )
        } else {
            (
                String::new(),
                text.strip_prefix("import ")
                    .unwrap_or(&text)
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>(),
            )
        };
        for sym in symbols {
            let mut parts = sym.splitn(2, " as ");
            let symbol = parts.next().unwrap_or("").trim();
            let alias = parts.next().map(str::trim);
            self.add(
                node,
                parent,
                scope,
                SemanticKind::Import {
                    module: if module.is_empty() {
                        symbol.to_owned()
                    } else {
                        module.clone()
                    },
                    symbol: if module.is_empty() {
                        None
                    } else {
                        Some(symbol.to_owned())
                    },
                    alias: alias.map(str::to_owned),
                },
                "import",
                Some(alias.unwrap_or(symbol).to_owned()),
                None,
                vec![],
            );
        }
    }

    fn assignment(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        let left = node.child_by_field_name("left");
        let right = node.child_by_field_name("right");
        let name = left.map(|n| self.text(n));
        let expr = right.map(|n| self.expr(n)).unwrap_or(Expression::Unknown);
        let id = self.add(
            node,
            parent,
            scope,
            SemanticKind::Assignment,
            "assignment",
            name,
            Some(expr.clone()),
            vec![],
        );
        if let Some(rhs) = right {
            self.expr_facts(rhs, Some(&id), scope, &expr);
        }
    }

    fn return_stmt(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        let value = node.named_child(0);
        let expr = value.map(|n| self.expr(n));
        let id = self.add(
            node,
            parent,
            scope,
            SemanticKind::Return,
            "return",
            None,
            expr.clone(),
            vec![],
        );
        if let (Some(value), Some(expr)) = (value, expr) {
            self.expr_facts(value, Some(&id), scope, &expr);
        }
    }

    fn if_statement(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        let id = self.add(
            node,
            parent,
            scope,
            SemanticKind::Conditional,
            "conditional",
            None,
            None,
            vec![],
        );
        for i in 0..node.named_child_count() {
            let Some(clause) = node.named_child(i) else {
                continue;
            };
            if matches!(clause.kind(), "if_clause" | "elif_clause" | "else_clause") {
                let arm_id = self.add(
                    clause,
                    Some(&id),
                    scope,
                    SemanticKind::Conditional,
                    "conditional_arm",
                    Some(clause.kind().to_owned()),
                    None,
                    vec![],
                );
                self.visit(clause, Some(&arm_id), scope);
            } else {
                // The condition is a dependency, never a possible return value.
                self.visit(clause, Some(&id), scope);
            }
        }
    }

    fn if_clause(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        let condition = node.child_by_field_name("condition");
        let body = node.child_by_field_name("body");
        if let Some(condition) = condition {
            self.visit_condition(condition, parent, scope);
        }
        if let Some(body) = body {
            self.visit(body, parent, scope);
        }
        if condition.is_none() && body.is_none() {
            self.visit_children(node, parent, scope);
        }
    }

    fn visit_condition(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        match node.kind() {
            "identifier" => {
                let name = self.text(node);
                self.add(
                    node,
                    parent,
                    scope,
                    SemanticKind::Name,
                    "name",
                    Some(name.clone()),
                    Some(Expression::Name(name)),
                    vec![],
                );
            }
            "call" | "attribute" => {
                let expr = self.expr(node);
                self.expr_facts(node, parent, scope, &expr);
            }
            _ => {
                for i in 0..node.named_child_count() {
                    if let Some(child) = node.named_child(i) {
                        self.visit_condition(child, parent, scope);
                    }
                }
            }
        }
    }

    fn visit_children(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.visit(child, parent, scope);
            }
        }
    }

    fn expr(&self, node: Node<'_>) -> Expression {
        match node.kind() {
            "identifier" => Expression::Name(self.text(node)),
            "call" => {
                let callee = node
                    .child_by_field_name("function")
                    .map(|n| self.expr(n))
                    .unwrap_or(Expression::Unknown);
                let args = node
                    .child_by_field_name("arguments")
                    .map(|a| {
                        (0..a.named_child_count())
                            .filter_map(|i| a.named_child(i))
                            .map(|n| self.expr(n))
                            .collect()
                    })
                    .unwrap_or_default();
                Expression::Call {
                    callee: Box::new(callee),
                    args,
                }
            }
            "attribute" => Expression::Attribute {
                receiver: Box::new(
                    node.child_by_field_name("object")
                        .map(|n| self.expr(n))
                        .unwrap_or(Expression::Unknown),
                ),
                name: node
                    .child_by_field_name("attribute")
                    .map(|n| self.text(n))
                    .unwrap_or_default(),
            },
            "tuple" | "expression_list" => Expression::Tuple(
                (0..node.named_child_count())
                    .filter_map(|i| node.named_child(i))
                    .map(|n| self.expr(n))
                    .collect(),
            ),
            "conditional_expression" => {
                let consequence = node
                    .child_by_field_name("consequence")
                    .or_else(|| node.named_child(0));
                let alternative = node
                    .child_by_field_name("alternative")
                    .or_else(|| node.named_child(node.named_child_count().saturating_sub(1)));
                Expression::Conditional {
                    branches: [consequence, alternative]
                        .iter()
                        .flatten()
                        .map(|n| self.expr(*n))
                        .collect(),
                }
            }
            "integer" | "float" | "string" | "true" | "false" | "none" => {
                Expression::Literal(self.text(node))
            }
            "parenthesized_expression" => node
                .named_child(0)
                .map(|n| self.expr(n))
                .unwrap_or(Expression::Unknown),
            _ => Expression::Unknown,
        }
    }

    fn expr_facts(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str, expr: &Expression) {
        let (kind, tag, name) = match expr {
            Expression::Call { .. } => (Some(SemanticKind::Call), "call", None),
            Expression::Attribute { name, .. } => (
                Some(SemanticKind::Attribute),
                "attribute",
                Some(name.clone()),
            ),
            Expression::Name(name) => (Some(SemanticKind::Name), "name", Some(name.clone())),
            Expression::Conditional { .. } => {
                (Some(SemanticKind::Conditional), "conditional", None)
            }
            _ => (None, "", None),
        };
        let item_id = kind.map(|kind| {
            self.add(
                node,
                parent,
                scope,
                kind,
                tag,
                name,
                Some(expr.clone()),
                vec![],
            )
        });
        if matches!(expr, Expression::Unknown) && is_expression_syntax(node.kind()) {
            self.diagnostics.push(Diagnostic {
                item_id: parent.map(str::to_owned),
                message: format!("unsupported expression syntax: {}", node.kind()),
                kind: DiagnosticKind::UnsupportedSyntax,
            });
        }
        if matches!(expr, Expression::Literal(_)) {
            return;
        }
        self.visit_expression_children(node, item_id.as_deref().or(parent), scope);
    }

    fn visit_expression_children(&mut self, node: Node<'_>, parent: Option<&str>, scope: &str) {
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(i) {
                let expression = self.expr(child);
                self.expr_facts(child, parent, scope, &expression);
            }
        }
    }
}

fn is_expression_syntax(kind: &str) -> bool {
    matches!(
        kind,
        "binary_operator"
            | "unary_operator"
            | "comparison_operator"
            | "boolean_operator"
            | "not_operator"
            | "subscript"
            | "dictionary"
            | "dictionary_comprehension"
            | "set"
            | "set_comprehension"
            | "list"
            | "list_comprehension"
            | "await"
            | "yield"
            | "lambda"
            | "named_expression"
            | "generator_expression"
            | "list_splat"
            | "keyword_argument"
            | "pair"
            | "splat_pattern"
    )
}

fn find_identifier<'tree>(node: Node<'tree>, text: &str, source: &str) -> Option<Node<'tree>> {
    if node.kind() == "identifier" && source.get(node.byte_range()) == Some(text) {
        return Some(node);
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if let Some(found) = find_identifier(child, text, source) {
                return Some(found);
            }
        }
    }
    None
}
