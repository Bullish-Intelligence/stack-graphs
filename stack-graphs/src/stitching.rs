// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2021, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Partial paths can be "stitched together" to produce name-binding paths.
//!
//! The "path stitching" algorithm defined in this module is how we take a collection of [partial
//! paths][] and use them to build up name-binding paths.  Our conjecture is that by building paths
//! this way, we can precompute a useful amount of work at _index time_ (when we construct the
//! partial paths), to reduce the amount of work that needs to be done at _query time_ (when those
//! partial paths are stitched together into paths).
//!
//! Complicating this story is that for large codebases (especially those with many upstream and
//! downstream dependencies), there is a _very_ large set of partial paths available to us.  We
//! want to be able to load those in _lazily_, during the execution of the path-stitching
//! algorithm.
//!
//! The [`Database`][] and [`PathStitcher`][] types provide this functionality.  `Database`
//! manages a collection of partial paths that have been loaded into this process from some
//! external data store.  `PathStitcher` implements the path-stitching algorithm in _phases_.
//! During each phase, we will process a set of (possibly incomplete) paths, looking in the
//! `Database` for the set of partial paths that are compatible with those paths.  It is your
//! responsibility to make sure that the database contains all of possible extensions of the paths
//! that we're going to process in that phase.  For the first phase, you already know which
//! paths you're starting the search from, and must make sure that the database starts out
//! containing the possible extensions of those "seed" paths.  For subsequent phases, you get to
//! see which paths will be processed in the _next_ phase as part of handling the _current_ phase.
//! This gives you the opporunity to load additional partial paths into the `Database` before
//! allowing the next phase to proceed.
//!
//! [partial paths]: ../partial/index.html
//! [`Database`]: struct.Database.html
//! [`PathStitcher`]: struct.PathStitcher.html

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::VecDeque;
#[cfg(feature = "copious-debugging")]
use std::fmt::Display;

use itertools::izip;
use itertools::Itertools;

use crate::arena::Arena;
use crate::arena::Handle;
use crate::arena::HandleSet;
use crate::arena::List;
use crate::arena::ListArena;
use crate::arena::ListCell;
use crate::arena::SupplementalArena;
use crate::cycles::Appendables;
use crate::cycles::AppendingCycleDetector;
use crate::cycles::SimilarPathDetector;
use crate::cycles::SimilarPathStats;
use crate::graph::Degree;
use crate::graph::Edge;
use crate::graph::File;
use crate::graph::Node;
use crate::graph::StackGraph;
use crate::graph::Symbol;
use crate::partial::Cyclicity;
use crate::partial::PartialPath;
use crate::partial::PartialPaths;
use crate::partial::PartialSymbolStack;
use crate::paths::Extend;
use crate::paths::PathResolutionError;
use crate::stats::FrequencyDistribution;
use crate::CancellationError;
use crate::CancellationFlag;

//-------------------------------------------------------------------------------------------------
// Appendable

/// Something that can be appended to a partial path.
pub trait Appendable {
    /// Append this appendable to the given path. Resolving jump nodes and renaming unused_variables
    /// is part of the responsibility of this method.
    fn append_to(
        &self,
        graph: &StackGraph,
        partials: &mut PartialPaths,
        path: &mut PartialPath,
    ) -> Result<(), PathResolutionError>;

    /// Return the start node.
    fn start_node(&self) -> Handle<Node>;

    /// Return the end node.
    fn end_node(&self) -> Handle<Node>;

    /// Return a Display implementation.
    fn display<'a>(
        &'a self,
        graph: &'a StackGraph,
        partials: &'a mut PartialPaths,
    ) -> Box<dyn std::fmt::Display + 'a>;
}

impl Appendable for Edge {
    fn append_to(
        &self,
        graph: &StackGraph,
        partials: &mut PartialPaths,
        path: &mut PartialPath,
    ) -> Result<(), PathResolutionError> {
        path.resolve_to_node(graph, partials, self.source)?;
        path.append(graph, partials, *self)
    }

    fn start_node(&self) -> Handle<Node> {
        self.source
    }

    fn end_node(&self) -> Handle<Node> {
        self.sink
    }

    fn display<'a>(
        &'a self,
        graph: &'a StackGraph,
        _partials: &'a mut PartialPaths,
    ) -> Box<dyn std::fmt::Display + 'a> {
        Box::new(format!(
            "{} -> {}",
            self.source.display(graph),
            self.sink.display(graph)
        ))
    }
}

impl Appendable for PartialPath {
    fn append_to(
        &self,
        graph: &StackGraph,
        partials: &mut PartialPaths,
        path: &mut PartialPath,
    ) -> Result<(), PathResolutionError> {
        path.resolve_to_node(graph, partials, self.start_node)?;
        path.ensure_no_overlapping_variables(partials, self);
        path.concatenate(graph, partials, self)?;
        Ok(())
    }

    fn start_node(&self) -> Handle<Node> {
        self.start_node
    }

    fn end_node(&self) -> Handle<Node> {
        self.end_node
    }

    fn display<'a>(
        &'a self,
        graph: &'a StackGraph,
        partials: &'a mut PartialPaths,
    ) -> Box<dyn std::fmt::Display + 'a> {
        Box::new(self.display(graph, partials))
    }
}

//-------------------------------------------------------------------------------------------------
// ToAppendable

/// A trait to be implemented on types such as [`Database`][] that allow converting handles
/// to appendables.
///
/// It is very similar to the [`std::ops::Index`] trait, but returns a reference instead
/// of a value, such that an efficient identifity implementation is possible, that doesn't
/// require cloning values.
pub trait ToAppendable<H, A>
where
    A: Appendable,
{
    fn get_appendable<'a>(&'a self, handle: &'a H) -> &'a A;
}

//-------------------------------------------------------------------------------------------------
// Candidates

/// A trait to support finding candidates for partial path extension. The candidates are represented
/// by handles `H`, which are mapped to appendables `A` using the database `Db`. Loading errors are
/// reported as values of the `Err` type.
pub trait ForwardCandidates<H, A, Db, Err>
where
    A: Appendable,
    Db: ToAppendable<H, A>,
{
    /// Load possible forward candidates for the given partial path into this candidates instance.
    /// Must be called before [`get_forward_candidates`] to allow lazy-loading implementations.
    fn load_forward_candidates(
        &mut self,
        _path: &PartialPath,
        _cancellation_flag: &dyn CancellationFlag,
    ) -> Result<(), Err> {
        Ok(())
    }

    /// Get forward candidates for extending the given partial path and add them to the provided
    /// result instance. If this instance loads data lazily, this only considers previously loaded
    /// data.
    fn get_forward_candidates<R>(&mut self, path: &PartialPath, result: &mut R)
    where
        R: std::iter::Extend<H>;

    /// Get the number of available candidates that share the given path's end node.
    fn get_joining_candidate_degree(&self, path: &PartialPath) -> Degree;

    /// Get the graph, partial path arena, and database backing this candidates instance.
    fn get_graph_partials_and_db(&mut self) -> (&StackGraph, &mut PartialPaths, &Db);
}

//-------------------------------------------------------------------------------------------------
// FileEdges

/// Acts as a database of the edges in the graph.
pub struct GraphEdgeCandidates<'a> {
    graph: &'a StackGraph,
    partials: &'a mut PartialPaths,
    file: Option<Handle<File>>,
    edges: GraphEdges,
}

impl<'a> GraphEdgeCandidates<'a> {
    pub fn new(
        graph: &'a StackGraph,
        partials: &'a mut PartialPaths,
        file: Option<Handle<File>>,
    ) -> Self {
        Self {
            graph,
            partials,
            file,
            edges: GraphEdges,
        }
    }
}

impl ForwardCandidates<Edge, Edge, GraphEdges, CancellationError> for GraphEdgeCandidates<'_> {
    fn get_forward_candidates<R>(&mut self, path: &PartialPath, result: &mut R)
    where
        R: std::iter::Extend<Edge>,
    {
        result.extend(self.graph.outgoing_edges(path.end_node).filter(|e| {
            self.file
                .map_or(true, |file| self.graph[e.sink].is_in_file(file))
        }));
    }

    fn get_joining_candidate_degree(&self, path: &PartialPath) -> Degree {
        self.graph.incoming_edge_degree(path.end_node)
    }

    fn get_graph_partials_and_db(&mut self) -> (&StackGraph, &mut PartialPaths, &GraphEdges) {
        (self.graph, self.partials, &self.edges)
    }
}

/// A dummy type to act as the "database" for graph edges. Its [`ToAppendable`] implementation
/// is the identity on edges.
pub struct GraphEdges;

impl ToAppendable<Edge, Edge> for GraphEdges {
    fn get_appendable<'a>(&'a self, edge: &'a Edge) -> &'a Edge {
        edge
    }
}

//-------------------------------------------------------------------------------------------------
// Databases

/// Contains a "database" of partial paths.
///
/// This type is meant to be a lazily loaded "view" into a proper storage layer.  During the
/// path-stitching algorithm, we repeatedly try to extend a currently incomplete path with any
/// partial paths that are compatible with it.  For large codebases, or projects with a large
/// number of dependencies, it can be prohibitive to load in _all_ of the partial paths up-front.
/// We've written the path-stitching algorithm so that you have a chance to only load in the
/// partial paths that are actually needed, placing them into a `Database` instance as they're
/// needed.
pub struct Database {
    pub(crate) partial_paths: Arena<PartialPath>,
    pub(crate) local_nodes: HandleSet<Node>,
    symbol_stack_keys: ListArena<Handle<Symbol>>,
    symbol_stack_key_cache: HashMap<SymbolStackCacheKey, SymbolStackKeyHandle>,
    paths_by_start_node: SupplementalArena<Node, Vec<Handle<PartialPath>>>,
    root_paths_by_precondition_prefix:
        SupplementalArena<SymbolStackKeyCell, Vec<Handle<PartialPath>>>,
    root_paths_by_precondition_with_variable:
        SupplementalArena<SymbolStackKeyCell, Vec<Handle<PartialPath>>>,
    root_paths_by_precondition_without_variable:
        SupplementalArena<SymbolStackKeyCell, Vec<Handle<PartialPath>>>,
    incoming_paths: SupplementalArena<Node, Degree>,
}

impl Database {
    /// Creates a new, empty database.
    pub fn new() -> Database {
        Database {
            partial_paths: Arena::new(),
            local_nodes: HandleSet::new(),
            symbol_stack_keys: List::new_arena(),
            symbol_stack_key_cache: HashMap::new(),
            paths_by_start_node: SupplementalArena::new(),
            root_paths_by_precondition_prefix: SupplementalArena::new(),
            root_paths_by_precondition_with_variable: SupplementalArena::new(),
            root_paths_by_precondition_without_variable: SupplementalArena::new(),
            incoming_paths: SupplementalArena::new(),
        }
    }

    /// Clear the database.  After this, all previous handles into the database are
    /// invalid.
    #[cfg_attr(not(feature = "storage"), allow(dead_code))]
    pub(crate) fn clear(&mut self) {
        self.partial_paths.clear();
        self.local_nodes.clear();
        self.symbol_stack_keys.clear();
        self.symbol_stack_key_cache.clear();
        self.paths_by_start_node.clear();
        self.root_paths_by_precondition_prefix.clear();
        self.root_paths_by_precondition_with_variable.clear();
        self.root_paths_by_precondition_without_variable.clear();
        self.incoming_paths.clear();
    }

    /// Adds a partial path to this database.  We do not deduplicate partial paths in any way; it's
    /// your responsibility to only add each partial path once.
    pub fn add_partial_path(
        &mut self,
        graph: &StackGraph,
        partials: &mut PartialPaths,
        path: PartialPath,
    ) -> Handle<PartialPath> {
        let start_node = path.start_node;
        let end_node = path.end_node;
        copious_debugging!(
            "    Add {} path to database {}",
            if graph[start_node].is_root() {
                "root"
            } else {
                "node"
            },
            path.display(graph, partials)
        );
        let symbol_stack_precondition = path.symbol_stack_precondition;
        let handle = self.partial_paths.add(path);

        // If the partial path starts at the root node, index it by its symbol stack precondition.
        if graph[start_node].is_root() {
            // The join node is root, so there's no need to use half-open symbol stacks here, as we
            // do for [`PartialPath::concatenate`][].
            let mut key = SymbolStackKey::from_partial_symbol_stack(
                partials,
                self,
                symbol_stack_precondition,
            );
            if !key.is_empty() {
                match symbol_stack_precondition.has_variable() {
                    true => self.root_paths_by_precondition_with_variable[key.back_handle()]
                        .push(handle),
                    false => self.root_paths_by_precondition_without_variable[key.back_handle()]
                        .push(handle),
                }
            }
            while key.pop_back(self).is_some() && !key.is_empty() {
                self.root_paths_by_precondition_prefix[key.back_handle()].push(handle);
            }
        } else {
            // Otherwise index it by its source node.
            self.paths_by_start_node[start_node].push(handle);
        }

        self.incoming_paths[end_node] += Degree::One;
        handle
    }

    /// Find all partial paths in this database that start at the given path's end node.
    /// If the end node is the root node, returns paths with a symbol stack precondition
    /// that are compatible with the path's symbol stack post condition.
    pub fn find_candidate_partial_paths<R>(
        &self,
        graph: &StackGraph,
        partials: &mut PartialPaths,
        path: &PartialPath,
        result: &mut R,
    ) where
        R: std::iter::Extend<Handle<PartialPath>>,
    {
        if graph[path.end_node].is_root() {
            // The join node is root, so there's no need to use half-open symbol stacks here, as we
            // do for [`PartialPath::concatenate`][].
            self.find_candidate_partial_paths_from_root(
                graph,
                partials,
                Some(path.symbol_stack_postcondition),
                result,
            );
        } else {
            self.find_candidate_partial_paths_from_node(graph, partials, path.end_node, result);
        }
    }

    /// Find all partial paths in this database that start at the root node, and have a symbol
    /// stack precondition that is compatible with a given symbol stack.
    #[cfg_attr(not(feature = "copious-debugging"), allow(unused_variables))]
    pub fn find_candidate_partial_paths_from_root<R>(
        &self,
        graph: &StackGraph,
        partials: &mut PartialPaths,
        symbol_stack: Option<PartialSymbolStack>,
        result: &mut R,
    ) where
        R: std::iter::Extend<Handle<PartialPath>>,
    {
        // If the path currently ends at the root node, then we need to look up partial paths whose
        // symbol stack precondition is compatible with the path.
        match symbol_stack {
            Some(symbol_stack) => {
                // The lookup does not add keys to the database.  A stack that has no key cannot
                // equal the precondition of any root path, and cannot extend it.
                let (mut key, key_exists) =
                    SymbolStackKey::lookup_partial_symbol_stack(partials, self, symbol_stack);
                copious_debugging!(
                    "      Search for symbol stack <{}>",
                    key.display(graph, self)
                );
                // paths that have exactly this symbol stack
                if let Some(paths) = key_exists
                    .then(|| {
                        self.root_paths_by_precondition_without_variable
                            .get(key.back_handle())
                    })
                    .flatten()
                {
                    #[cfg(feature = "copious-debugging")]
                    {
                        for path in paths {
                            copious_debugging!(
                                "        Found path with exact stack {}",
                                self[*path].display(graph, partials)
                            );
                        }
                    }
                    result.extend(paths.iter().copied());
                }
                // paths that have an extension of this symbol stack
                if symbol_stack.has_variable() {
                    if let Some(paths) = key_exists
                        .then(|| {
                            self.root_paths_by_precondition_prefix
                                .get(key.back_handle())
                        })
                        .flatten()
                    {
                        #[cfg(feature = "copious-debugging")]
                        {
                            for path in paths {
                                copious_debugging!(
                                    "        Found path with smaller stack {}",
                                    self[*path].display(graph, partials)
                                );
                            }
                        }
                        result.extend(paths.iter().copied());
                    }
                }
                loop {
                    // paths that have a prefix of this symbol stack
                    if let Some(paths) = self
                        .root_paths_by_precondition_with_variable
                        .get(key.back_handle())
                    {
                        #[cfg(feature = "copious-debugging")]
                        {
                            for path in paths {
                                copious_debugging!(
                                    "        Found path with smaller stack {}",
                                    self[*path].display(graph, partials)
                                );
                            }
                        }
                        result.extend(paths.iter().copied());
                    }
                    if key.pop_back(self).is_none() {
                        break;
                    }
                }
            }
            None => {
                copious_debugging!("      Search for all root paths");
                for (_, paths) in self
                    .root_paths_by_precondition_with_variable
                    .iter()
                    .chain(self.root_paths_by_precondition_without_variable.iter())
                {
                    #[cfg(feature = "copious-debugging")]
                    {
                        for path in paths {
                            copious_debugging!(
                                "        Found path {}",
                                self[*path].display(graph, partials)
                            );
                        }
                    }
                    result.extend(paths.iter().copied());
                }
            }
        }
    }

    /// Find all partial paths in the database that start at the given node.  We don't filter the
    /// results any further than that, since we have to check each partial path for compatibility
    /// as we try to append it to the current incomplete path anyway, and non-root nodes will
    /// typically have a small number of outgoing edges.
    #[cfg_attr(not(feature = "copious-debugging"), allow(unused_variables))]
    pub fn find_candidate_partial_paths_from_node<R>(
        &self,
        graph: &StackGraph,
        partials: &mut PartialPaths,
        start_node: Handle<Node>,
        result: &mut R,
    ) where
        R: std::iter::Extend<Handle<PartialPath>>,
    {
        copious_debugging!("      Search for start node {}", start_node.display(graph));
        // Return all of the partial paths that start at the requested node.
        if let Some(paths) = self.paths_by_start_node.get(start_node) {
            #[cfg(feature = "copious-debugging")]
            {
                for path in paths {
                    copious_debugging!(
                        "        Found path {}",
                        self[*path].display(graph, partials)
                    );
                }
            }
            result.extend(paths.iter().copied());
        }
    }

    /// Returns the number of paths in this database that share the given end node.  Nodes that
    /// no partial path in this database ends at have degree [`Degree::Zero`][].
    pub fn get_incoming_path_degree(&self, end_node: Handle<Node>) -> Degree {
        // Only `add_partial_path` grows `incoming_paths`.  It has the size of the largest end node
        // of any partial path in the database, not the size of the graph.  Callers can ask for any
        // graph node, so the index can be out of bounds.
        self.incoming_paths.get(end_node).copied().unwrap_or_default()
    }

    /// Determines which nodes in the stack graph are “local”, taking into account the partial
    /// paths in this database.
    ///
    /// A local node has no partial path that connects it to the root node in either direction.
    /// That means that it cannot participate in any paths that leave the file.
    ///
    /// This method is meant to be used at index time, to calculate the set of nodes that are local
    /// after having just calculated the set of partial paths for the file.
    pub fn find_local_nodes(&mut self) {
        // Assume that any node that is the start or end of a partial path is local to this file
        // until we see a path connecting the root node to it (in either direction).
        self.local_nodes.clear();
        for handle in self.iter_partial_paths() {
            self.local_nodes.add(self[handle].start_node);
            self.local_nodes.add(self[handle].end_node);
        }

        // The root node and jump-to-scope node are the most obvious non-local nodes.
        let mut nonlocal_start_nodes = HandleSet::new();
        let mut nonlocal_end_nodes = HandleSet::new();
        self.local_nodes.remove(StackGraph::root_node());
        nonlocal_start_nodes.add(StackGraph::root_node());
        nonlocal_end_nodes.add(StackGraph::root_node());
        self.local_nodes.remove(StackGraph::jump_to_node());
        nonlocal_start_nodes.add(StackGraph::jump_to_node());
        nonlocal_end_nodes.add(StackGraph::jump_to_node());

        // Other nodes are non-local if we see any partial path that connects it to another
        // non-local node.  Repeat until we reach a fixed point.
        let mut keep_checking = true;
        while keep_checking {
            keep_checking = false;
            for handle in self.iter_partial_paths() {
                let start_node = self[handle].start_node;
                let end_node = self[handle].end_node;

                // First check forwards paths, where non-localness propagates from the start node
                // of each path.
                let start_node_is_nonlocal = nonlocal_start_nodes.contains(start_node);
                let end_node_is_nonlocal = nonlocal_start_nodes.contains(end_node);
                if start_node_is_nonlocal && !end_node_is_nonlocal {
                    keep_checking = true;
                    nonlocal_start_nodes.add(end_node);
                    self.local_nodes.remove(end_node);
                }

                // Then check reverse paths, where non-localness propagates from the end node of
                // each path.
                let start_node_is_nonlocal = nonlocal_end_nodes.contains(start_node);
                let end_node_is_nonlocal = nonlocal_end_nodes.contains(end_node);
                if !start_node_is_nonlocal && end_node_is_nonlocal {
                    keep_checking = true;
                    nonlocal_end_nodes.add(start_node);
                    self.local_nodes.remove(start_node);
                }
            }
        }
    }

    /// Marks that a stack graph node is local.
    ///
    /// This method is meant to be used at query time.  You will have precalculated the set of
    /// local nodes for a file at index time; at query time, you will load this information from
    /// your storage layer and use this method to update our internal view of which nodes are
    /// local.
    pub fn mark_local_node(&mut self, node: Handle<Node>) {
        self.local_nodes.add(node);
    }

    /// Returns whether a node is local according to the partial paths in this database.  You must
    /// have already called [`find_local_nodes`][] or [`mark_local_node`][], depending on whether
    /// it is index time or query time.
    pub fn node_is_local(&self, node: Handle<Node>) -> bool {
        self.local_nodes.contains(node)
    }

    /// Returns an iterator over all of the handles of all of the partial paths in this database.
    /// (Note that because we're only returning _handles_, this iterator does not retain a
    /// reference to the `Database`.)
    pub fn iter_partial_paths(&self) -> impl Iterator<Item = Handle<PartialPath>> {
        self.partial_paths.iter_handles()
    }

    pub fn ensure_both_directions(&mut self, partials: &mut PartialPaths) {
        for path in self.partial_paths.iter_handles() {
            self.partial_paths
                .get_mut(path)
                .ensure_both_directions(partials);
        }
    }

    pub fn ensure_forwards(&mut self, partials: &mut PartialPaths) {
        for path in self.partial_paths.iter_handles() {
            self.partial_paths.get_mut(path).ensure_forwards(partials);
        }
    }
}

impl std::ops::Index<Handle<PartialPath>> for Database {
    type Output = PartialPath;
    #[inline(always)]
    fn index(&self, handle: Handle<PartialPath>) -> &PartialPath {
        self.partial_paths.get(handle)
    }
}

impl ToAppendable<Handle<PartialPath>, PartialPath> for Database {
    fn get_appendable<'a>(&'a self, handle: &'a Handle<PartialPath>) -> &'a PartialPath {
        &self[*handle]
    }
}

/// Finds candidate partial paths in a [`Database`][].  Queries do not change the database.  The
/// `partials` argument is the only state that grows.
pub struct DatabaseCandidates<'a> {
    graph: &'a StackGraph,
    partials: &'a mut PartialPaths,
    database: &'a Database,
}

impl<'a> DatabaseCandidates<'a> {
    pub fn new(
        graph: &'a StackGraph,
        partials: &'a mut PartialPaths,
        database: &'a Database,
    ) -> Self {
        Self {
            graph,
            partials,
            database,
        }
    }
}

impl ForwardCandidates<Handle<PartialPath>, PartialPath, Database, CancellationError>
    for DatabaseCandidates<'_>
{
    fn get_forward_candidates<R>(&mut self, path: &PartialPath, result: &mut R)
    where
        R: std::iter::Extend<Handle<PartialPath>>,
    {
        self.database
            .find_candidate_partial_paths(self.graph, self.partials, path, result);
    }

    fn get_joining_candidate_degree(&self, path: &PartialPath) -> Degree {
        self.database.get_incoming_path_degree(path.end_node)
    }

    fn get_graph_partials_and_db(&mut self) -> (&StackGraph, &mut PartialPaths, &Database) {
        (self.graph, self.partials, self.database)
    }
}

/// The key type that we use to find partial paths that start from the root node and have a
/// particular symbol stack as their precondition.
#[derive(Clone, Copy)]
pub struct SymbolStackKey {
    // Note: the symbols are stored in reverse order, with the "front" of the List being the "back"
    // of the symbol stack.  That lets us easily get a handle to the back of the symbol stack, and
    // also lets us easily pops items off the back of key, which we need to do to search for all
    // prefixes of a particular symbol stack down in `find_candidate_partial_paths_from_root`.
    symbols: List<Handle<Symbol>>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct SymbolStackCacheKey {
    head: Handle<Symbol>,
    tail: SymbolStackKeyHandle,
}

type SymbolStackKeyCell = ListCell<Handle<Symbol>>;
type SymbolStackKeyHandle = Handle<SymbolStackKeyCell>;

impl SymbolStackKey {
    /// Returns an empty symbol stack key.
    fn empty() -> SymbolStackKey {
        SymbolStackKey {
            symbols: List::empty(),
        }
    }

    fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Pushes a new symbol onto the back of this symbol stack key.
    fn push_back(&mut self, db: &mut Database, symbol: Handle<Symbol>) {
        let cache_key = SymbolStackCacheKey {
            head: symbol,
            tail: self.back_handle(),
        };
        if let Some(handle) = db.symbol_stack_key_cache.get(&cache_key) {
            self.symbols = List::from_handle(*handle);
            return;
        }
        // push_front because we store the key's symbols in reverse order.
        self.symbols.push_front(&mut db.symbol_stack_keys, symbol);
        let handle = self.back_handle();
        db.symbol_stack_key_cache.insert(cache_key, handle);
    }

    /// Pushes a symbol onto the back of this key, only if the database already has the result.
    /// Returns whether it has.  The key does not change when it has not.
    fn push_back_existing(&mut self, db: &Database, symbol: Handle<Symbol>) -> bool {
        let cache_key = SymbolStackCacheKey {
            head: symbol,
            tail: self.back_handle(),
        };
        match db.symbol_stack_key_cache.get(&cache_key) {
            Some(handle) => {
                self.symbols = List::from_handle(*handle);
                true
            }
            None => false,
        }
    }

    /// Pops a symbol from the back of this symbol stack key.
    fn pop_back(&mut self, db: &Database) -> Option<Handle<Symbol>> {
        // pop_front because we store the key's symbols in reverse order.
        self.symbols.pop_front(&db.symbol_stack_keys).copied()
    }

    /// Extracts a new symbol stack key from a partial symbol stack.
    pub fn from_partial_symbol_stack(
        partials: &mut PartialPaths,
        db: &mut Database,
        mut stack: PartialSymbolStack,
    ) -> SymbolStackKey {
        let mut result = SymbolStackKey::empty();
        while let Some(symbol) = stack.pop_front(partials) {
            result.push_back(db, symbol.symbol);
        }
        result
    }

    /// Extracts a symbol stack key from a partial symbol stack, without changing the database.
    /// If the database lacks a key for the whole stack, this returns the longest prefix that has
    /// a key, and `false`.  No partial path in the database has the whole stack as a precondition
    /// prefix in that case.
    fn lookup_partial_symbol_stack(
        partials: &mut PartialPaths,
        db: &Database,
        mut stack: PartialSymbolStack,
    ) -> (SymbolStackKey, bool) {
        let mut result = SymbolStackKey::empty();
        while let Some(symbol) = stack.pop_front(partials) {
            if !result.push_back_existing(db, symbol.symbol) {
                return (result, false);
            }
        }
        (result, true)
    }

    /// Returns a handle to the back of the symbol stack key.
    fn back_handle(self) -> SymbolStackKeyHandle {
        // Because the symbols are stored in reverse order, the handle to the "front" of the list
        // is a handle to the "back" of the key.
        self.symbols.handle()
    }

    #[cfg(feature = "copious-debugging")]
    fn display<'a>(self, graph: &'a StackGraph, db: &'a Database) -> impl Display + 'a {
        DisplaySymbolStackKey(self, graph, db)
    }
}

#[cfg(feature = "copious-debugging")]
struct DisplaySymbolStackKey<'a>(SymbolStackKey, &'a StackGraph, &'a Database);

#[cfg(feature = "copious-debugging")]
impl<'a> Display for DisplaySymbolStackKey<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // Use a recursive function to print the contents of the key out in reverse order.
        fn display_one(
            mut key: SymbolStackKey,
            graph: &StackGraph,
            db: &Database,
            f: &mut std::fmt::Formatter,
        ) -> std::fmt::Result {
            let last = match key.pop_back(db) {
                Some(last) => last,
                None => return Ok(()),
            };
            display_one(key, graph, db, f)?;
            last.display(graph).fmt(f)
        }
        display_one(self.0, self.1, self.2, f)
    }
}

//-------------------------------------------------------------------------------------------------
// Stitching partial paths together

/// Implements a phased forward partial path stitching algorithm.
///
/// Our overall goal is to start with a set of _seed_ partial paths, and to repeatedly extend each
/// partial path by concatenating another, compatible partial path onto the end of it.  (If there
/// are multiple compatible partial paths, we concatenate each of them separately, resulting in
/// more than one extension for the current path.)
///
/// We perform this processing in _phases_.  At the start of each phase, we have a _current set_ of
/// partial paths that need to be processed.  As we extend those partial paths, we add the
/// extensions to the set of partial paths to process in the _next_ phase.  Phases are processed
/// one at a time, each time you invoke the [`process_next_phase`][] method.
///
/// [`process_next_phase`]: #method.process_next_phase
///
/// After each phase has completed, you can use the [`previous_phase_partial_paths`][] method to
/// retrieve all of the partial paths that were discovered during that phase.  That gives you a
/// chance to add to the `Database` all of the other partial paths that we might need to extend
/// those partial paths with before invoking the next phase.
///
/// [`previous_phase_partial_paths`]: #method.previous_phase_partial_paths
///
/// If you don't care about this phasing nonsense, you can instead preload your `Database` with all
/// possible partial paths, and run the forward partial path stitching algorithm all the way to
/// completion, using the [`find_all_complete_partial_paths`][] method.
///
/// [`find_all_complete_partial_paths`]: #method.find_all_complete_partial_paths
pub struct ForwardPartialPathStitcher<H> {
    candidates: Vec<H>,
    extensions: Vec<(PartialPath, AppendingCycleDetector<H>)>,
    queue: VecDeque<(PartialPath, AppendingCycleDetector<H>, bool)>,
    // tracks the number of initial paths in the queue because we do not want call
    // extend_until on those
    initial_paths_in_queue: usize,
    // next_iteration is a tuple of queues instead of an queue of tuples so that the path queue
    // can be cheaply exposed through the C API as a continuous memory block
    next_iteration: (
        VecDeque<PartialPath>,
        VecDeque<AppendingCycleDetector<H>>,
        VecDeque<bool>,
    ),
    appended_paths: Appendables<H>,
    similar_path_detector: Option<SimilarPathDetector<PartialPath>>,
    shadow_aware_similar_paths: bool,
    check_only_join_nodes: bool,
    max_work_per_phase: usize,
    limits: StitcherLimits,
    /// The `PartialPaths` cell count when the stitcher started.
    arena_baseline: usize,
    /// The work done so far.  See [`StitcherLimits::max_work`].
    work: usize,
    peak_queue_len: usize,
    arena_cells_added: usize,
    /// Set when a limit stopped the run, or cut paths.
    bounded: Option<StitchBound>,
    /// Set when the run stopped for good.
    aborted: bool,
    initial_paths: usize,
    stats: Option<Stats>,
    #[cfg(feature = "copious-debugging")]
    phase_number: usize,
}

impl<H> ForwardPartialPathStitcher<H> {
    /// Creates a new forward partial path stitcher that is "seeded" with a set of initial partial
    /// paths. If the sticher is used to find complete paths, it is the responsibility of the caller
    /// to ensure precondition variables are eliminated by calling [`PartialPath::eliminate_precondition_stack_variables`][].
    pub fn from_partial_paths<I>(
        _graph: &StackGraph,
        partials: &mut PartialPaths,
        initial_partial_paths: I,
    ) -> Self
    where
        I: IntoIterator<Item = PartialPath>,
    {
        let mut appended_paths = Appendables::new();
        let next_iteration: (VecDeque<_>, VecDeque<_>, VecDeque<_>) = initial_partial_paths
            .into_iter()
            .map(|p| {
                let c = AppendingCycleDetector::from(&mut appended_paths, p.clone().into());
                (p, c, false)
            })
            .multiunzip();
        let initial_paths = next_iteration.0.len();
        Self {
            candidates: Vec::new(),
            extensions: Vec::new(),
            queue: VecDeque::new(),
            initial_paths_in_queue: initial_paths,
            next_iteration,
            appended_paths,
            // By default, all paths are checked for similarity
            similar_path_detector: Some(SimilarPathDetector::new()),
            // By default, the detector keeps the first of two similar paths
            shadow_aware_similar_paths: false,
            // By default, all nodes are checked for cycles and (if enabled) similarity
            check_only_join_nodes: false,
            // By default, there's no artificial bound on the amount of work done per phase
            max_work_per_phase: usize::MAX,
            // By default, there are no limits
            limits: StitcherLimits::default(),
            arena_baseline: partials.allocated_cells(),
            work: 0,
            peak_queue_len: 0,
            arena_cells_added: 0,
            bounded: None,
            aborted: false,
            initial_paths,
            stats: None,
            #[cfg(feature = "copious-debugging")]
            phase_number: 1,
        }
    }

    /// Sets whether similar path detection should be enabled during path stitching. Paths are similar
    /// if start and end node, and pre- and postconditions are the same. The presence of similar paths
    /// can lead to exponential blow up during path stitching. Similar path detection is enabled by
    /// default.
    pub fn set_similar_path_detection(&mut self, detect_similar_paths: bool) {
        if !detect_similar_paths {
            self.similar_path_detector = None;
        } else if self.similar_path_detector.is_none() {
            let mut similar_path_detector = SimilarPathDetector::new();
            similar_path_detector.set_collect_stats(self.stats.is_some());
            self.similar_path_detector = Some(similar_path_detector);
        }
    }

    /// Sets whether similar path detection takes shadowing into account.  Of two similar paths, the
    /// detector keeps the first.  The first path can be one that another path shadows, while the
    /// second path is not.  Then the detector hides a definition that the second path reaches.
    /// With this option, the detector keeps the first path that nothing can shadow, too.  At most
    /// two similar paths per start node, end node, and pair of stack lengths stay.  See
    /// [`PartialPath::can_be_shadowed`][].  The option has no effect if similar path detection is
    /// off.
    pub fn set_shadow_aware_similar_paths(&mut self, shadow_aware_similar_paths: bool) {
        self.shadow_aware_similar_paths = shadow_aware_similar_paths;
    }

    /// Sets whether all nodes are checked for cycles and (if enabled) similar paths, or only nodes with multiple
    /// incoming candidates. Checking only join nodes is **unsafe** unless the database of candidates is stable
    /// between all stitching phases. If paths are added to the database from one phase to another, for example if
    /// paths are dynamically loaded from storage, setting this to true is incorrect and might lead to non-termination!
    pub fn set_check_only_join_nodes(&mut self, check_only_join_nodes: bool) {
        self.check_only_join_nodes = check_only_join_nodes;
    }

    /// Sets the maximum amount of work that can be performed during each phase of the algorithm.
    /// By bounding our work this way, you can ensure that it's not possible for our CPU-bound
    /// algorithm to starve any worker threads or processes that you might be using.  If you don't
    /// call this method, then we allow ourselves to process all of the extensions of all of the
    /// paths found in the previous phase, with no additional bound.
    pub fn set_max_work_per_phase(&mut self, max_work_per_phase: usize) {
        self.max_work_per_phase = max_work_per_phase;
    }

    /// Sets the limits of this run.  See [`StitcherLimits`][].  By default, a stitcher has no
    /// limits.
    pub fn set_limits(&mut self, limits: StitcherLimits) {
        self.limits = limits;
    }

    /// Returns why a limit stopped the run or cut paths, or `None` if no limit had an effect.
    pub fn bounded(&self) -> Option<StitchBound> {
        self.bounded
    }

    /// Sets whether to collect statistics during stitching.
    pub fn set_collect_stats(&mut self, collect_stats: bool) {
        if !collect_stats {
            self.stats = None;
        } else if self.stats.is_none() {
            let mut stats = Stats::default();
            stats.initial_paths.record(self.initial_paths);
            self.stats = Some(stats);
        }
        if let Some(similar_path_detector) = &mut self.similar_path_detector {
            similar_path_detector.set_collect_stats(collect_stats);
        }
    }

    pub fn into_stats(mut self) -> Stats {
        if let (Some(stats), Some(similar_path_detector)) =
            (&mut self.stats, self.similar_path_detector)
        {
            stats.similar_paths_stats = similar_path_detector.stats();
        }
        // The limit counters are always available, even when the caller does not collect stats.
        let mut stats = self.stats.unwrap_or_default();
        stats.bounded = self.bounded;
        stats.work = self.work;
        stats.peak_queue_len = self.peak_queue_len;
        stats.arena_cells_added = self.arena_cells_added;
        stats
    }
}

impl<H: Clone> ForwardPartialPathStitcher<H> {
    /// Returns an iterator of all of the (possibly incomplete) partial paths that were encountered
    /// during the most recent phase of the algorithm.
    pub fn previous_phase_partial_paths(&self) -> impl Iterator<Item = &PartialPath> + '_ {
        self.next_iteration.0.iter()
    }

    /// Returns a slice of all of the (possibly incomplete) partial paths that were encountered
    /// during the most recent phase of the algorithm.
    pub fn previous_phase_partial_paths_slice(&mut self) -> &[PartialPath] {
        self.next_iteration.0.make_contiguous();
        self.next_iteration.0.as_slices().0
    }

    /// Returns a mutable slice of all of the (possibly incomplete) partial paths that were
    /// encountered during the most recent phase of the algorithm.
    pub fn previous_phase_partial_paths_slice_mut(&mut self) -> &mut [PartialPath] {
        self.next_iteration.0.make_contiguous();
        self.next_iteration.0.as_mut_slices().0
    }

    /// Attempts to extend one partial path as part of the algorithm.  When calling this function,
    /// you are responsible for ensuring that `db` already contains all of the possible appendables
    /// that we might want to extend `partial_path` with.
    fn extend<A, Db, C, Err>(
        &mut self,
        candidates: &mut C,
        partial_path: &PartialPath,
        cycle_detector: AppendingCycleDetector<H>,
        has_split: bool,
    ) -> usize
    where
        A: Appendable,
        Db: ToAppendable<H, A>,
        C: ForwardCandidates<H, A, Db, Err>,
    {
        let check_cycle = !self.check_only_join_nodes
            || partial_path.start_node == partial_path.end_node
            || candidates.get_joining_candidate_degree(partial_path) == Degree::Multiple;

        let (graph, partials, db) = candidates.get_graph_partials_and_db();
        copious_debugging!("    Extend {}", partial_path.display(graph, partials));

        if check_cycle {
            // Check is path is cyclic, in which case we do not extend it. We only do this if the start and end nodes are the same,
            // or the current end node has multiple incoming edges. If neither of these hold, the path cannot end in a cycle.
            let has_precondition_variables = partial_path.symbol_stack_precondition.has_variable()
                || partial_path.scope_stack_precondition.has_variable();
            let cycles = match cycle_detector.is_cyclic(graph, partials, db, &mut self.appended_paths)
            {
                Ok(cycles) => cycles,
                Err(_error) => {
                    // The cycle test rebuilds the suspected cycle on a new path at the end node.
                    // It appends the fragments to initial stack variables, not to the concrete
                    // stacks of the real path.  This can fail to unify.  For example, a "drop
                    // scopes" end node gives a scope stack without variables, which cannot
                    // satisfy a fragment that needs concrete scopes.  A failure means we cannot
                    // decide if the path is cyclic.  Discontinue the path, as for a proven cycle.
                    copious_debugging!("      is discontinued: cycle test failed: {:?}", _error);
                    return 0;
                }
            };
            let cyclic = match has_precondition_variables {
                // If the precondition has no variables, we allow cycles that strengthen the
                // precondition, because we know they cannot strengthen the precondition of
                // the overall path.
                false => !cycles
                    .into_iter()
                    .all(|c| c == Cyclicity::StrengthensPrecondition),
                // If the precondition has variables, do not allow any cycles, not even those
                // that strengthen the precondition. This is more strict than necessary. Better
                // might be to disallow precondition strengthening cycles only if they would
                // strengthen the overall path precondition.
                true => !cycles.is_empty(),
            };
            if cyclic {
                copious_debugging!("      is discontinued: cyclic");
                return 0;
            }
        }

        // find candidates to append
        self.candidates.clear();
        candidates.get_forward_candidates(partial_path, &mut self.candidates);
        let (graph, partials, db) = candidates.get_graph_partials_and_db();

        // try to extend path with candidates
        let candidate_count = self.candidates.len();
        self.extensions.clear();
        self.extensions.reserve(candidate_count);
        for candidate in &self.candidates {
            let appendable = db.get_appendable(candidate);
            copious_debugging!("      with {}", appendable.display(graph, partials));

            let mut new_partial_path = partial_path.clone();
            let mut new_cycle_detector = cycle_detector.clone();
            // If there are errors concatenating these partial paths, or resolving the resulting
            // partial path, just skip the extension — it's not a fatal error.
            #[cfg_attr(not(feature = "copious-debugging"), allow(unused_variables))]
            {
                if let Err(err) = appendable.append_to(graph, partials, &mut new_partial_path) {
                    copious_debugging!("        is invalid: {:?}", err);
                    continue;
                }
            }
            new_cycle_detector.append(&mut self.appended_paths, candidate.clone());
            copious_debugging!("        is {}", new_partial_path.display(graph, partials));
            self.extensions.push((new_partial_path, new_cycle_detector));
        }

        let extension_count = self.extensions.len();
        let new_has_split = has_split || self.extensions.len() > 1;
        self.next_iteration.0.reserve(extension_count);
        self.next_iteration.1.reserve(extension_count);
        self.next_iteration.2.reserve(extension_count);
        for (new_partial_path, new_cycle_detector) in self.extensions.drain(..) {
            if let Some(max_path_length) = self.limits.max_path_length {
                if new_partial_path.edges.len() > max_path_length {
                    // Drop the path, and keep going.  The result can lack paths.
                    self.bounded.get_or_insert(StitchBound::PathLength);
                    continue;
                }
            }
            let check_similar_path = new_has_split
                && (!self.check_only_join_nodes
                    || candidates.get_joining_candidate_degree(&new_partial_path)
                        == Degree::Multiple);
            let (graph, partials, _) = candidates.get_graph_partials_and_db();
            if check_similar_path {
                if let Some(similar_path_detector) = &mut self.similar_path_detector {
                    let cmp = |ps: &mut PartialPaths, left: &PartialPath, right: &PartialPath| {
                        if !left.equals(ps, right) {
                            None
                        } else {
                            if left.shadows(ps, right) {
                                Some(Ordering::Less)
                            } else if right.shadows(ps, left) {
                                Some(Ordering::Greater)
                            } else {
                                Some(Ordering::Equal)
                            }
                        }
                    };
                    let rejected = if self.shadow_aware_similar_paths {
                        similar_path_detector.add_path_shadow_aware(
                            graph,
                            partials,
                            &new_partial_path,
                            |ps, path| path.can_be_shadowed(graph, ps),
                            cmp,
                        )
                    } else {
                        similar_path_detector.add_path(graph, partials, &new_partial_path, cmp)
                    };
                    if rejected {
                        copious_debugging!(
                            " extension {}",
                            new_partial_path.display(graph, partials)
                        );
                        copious_debugging!("        is rejected: too many similar");
                        continue;
                    }
                }
            }

            self.next_iteration.0.push(new_partial_path);
            self.next_iteration.1.push(new_cycle_detector);
            self.next_iteration.2.push(new_has_split);
        }

        if let Some(stats) = &mut self.stats {
            let (graph, _, _) = candidates.get_graph_partials_and_db();
            let end_node = &graph[partial_path.end_node];
            if end_node.is_root() {
                stats.candidates_per_root_path.record(candidate_count);
                stats.extensions_per_root_path.record(extension_count);
                stats.root_visits += 1;
            } else {
                stats.candidates_per_node_path.record(candidate_count);
                stats.extensions_per_node_path.record(extension_count);
                stats.node_visits.record(end_node.id());
            }
            if extension_count == 0 {
                stats.terminal_path_lengh.record(partial_path.edges.len());
            }
        }
        candidate_count
    }

    /// Returns whether the algorithm has completed.
    pub fn is_complete(&self) -> bool {
        self.aborted || (self.queue.is_empty() && self.next_iteration.0.is_empty())
    }

    /// Checks the limits that can stop the run.  Returns whether the run must stop.
    fn check_limits(&mut self, partials: &PartialPaths) -> bool {
        let queue_len = self.queue.len() + self.next_iteration.0.len();
        self.peak_queue_len = self.peak_queue_len.max(queue_len);
        self.arena_cells_added = partials
            .allocated_cells()
            .saturating_sub(self.arena_baseline);
        let reason = if self.limits.max_work.map_or(false, |max| self.work > max) {
            Some(StitchBound::Work)
        } else if self.limits.max_queue_len.map_or(false, |max| queue_len > max) {
            Some(StitchBound::QueueLength)
        } else if self
            .limits
            .max_arena_cells
            .map_or(false, |max| self.arena_cells_added > max)
        {
            Some(StitchBound::ArenaCells)
        } else {
            None
        };
        if let Some(reason) = reason {
            // Paths that earlier phases found stay available.
            self.bounded = Some(reason);
            self.aborted = true;
            // Free the memory of the unprocessed paths now.
            self.queue = VecDeque::new();
            return true;
        }
        false
    }

    /// Runs the next phase of the algorithm.  We will have built up a set of incomplete partial
    /// paths during the _previous_ phase.  Before calling this function, you must ensure that `db`
    /// contains all of the possible appendables that we might want to extend any of those
    /// candidate partial paths with.
    ///
    /// After this method returns, you can use [`previous_phase_partial_paths`][] to retrieve a
    /// list of the (possibly incomplete) partial paths that were encountered during this phase.
    ///
    /// The `extend_while` closure is used to control whether the extended paths are further extended
    /// or not. It is not called on the initial paths.
    ///
    /// [`previous_phase_partial_paths`]: #method.previous_phase_partial_paths
    pub fn process_next_phase<A, Db, C, E, Err>(&mut self, candidates: &mut C, extend_while: E)
    where
        A: Appendable,
        Db: ToAppendable<H, A>,
        C: ForwardCandidates<H, A, Db, Err>,
        E: Fn(&StackGraph, &mut PartialPaths, &PartialPath) -> bool,
    {
        copious_debugging!("==> Start phase {}", self.phase_number);
        if self.aborted {
            return;
        }
        self.queue.extend(izip!(
            self.next_iteration.0.drain(..),
            self.next_iteration.1.drain(..),
            self.next_iteration.2.drain(..),
        ));
        if let Some(stats) = &mut self.stats {
            stats.queued_paths_per_phase.record(self.queue.len());
        }
        let mut work_performed = 0;
        while let Some((partial_path, cycle_detector, has_split)) = self.queue.pop_front() {
            let (graph, partials, _) = candidates.get_graph_partials_and_db();
            copious_debugging!(
                "--> Candidate partial path {}",
                partial_path.display(graph, partials)
            );
            if self.initial_paths_in_queue > 0 {
                self.initial_paths_in_queue -= 1;
            } else if !extend_while(graph, partials, &partial_path) {
                copious_debugging!(
                    "    Do not extend {}",
                    partial_path.display(graph, partials)
                );
                continue;
            }
            let extended = self.extend(candidates, &partial_path, cycle_detector, has_split);
            work_performed += extended;
            self.work += 1 + extended;
            let (_, partials, _) = candidates.get_graph_partials_and_db();
            if self.check_limits(partials) {
                break;
            }
            if work_performed >= self.max_work_per_phase {
                break;
            }
        }
        if let Some(stats) = &mut self.stats {
            stats.processed_paths_per_phase.record(work_performed);
        }

        #[cfg(feature = "copious-debugging")]
        {
            if let Some(similar_path_detector) = &self.similar_path_detector {
                copious_debugging!(
                    "    Max similar path bucket size: {}",
                    similar_path_detector.max_bucket_size()
                );
            }
            copious_debugging!("==> End phase {}", self.phase_number);
            self.phase_number += 1;
        }
    }
}

impl ForwardPartialPathStitcher<Edge> {
    /// Finds a minimal set of partial paths in a file, calling the `visit` closure for each one.
    ///
    /// This function ensures that the set of visited partial paths
    ///  (a) is minimal, no path can be constructed by stitching other paths in the set, and
    ///  (b) covers all complete paths, from references to definitions, when used for path stitching
    ///
    /// This function will not return until all reachable partial paths have been processed, so
    /// your database must already contain all partial paths that might be needed.  If you have a
    /// very large stack graph stored in some other storage system, and want more control over
    /// lazily loading only the necessary pieces, then you should code up your own loop that calls
    /// [`process_next_phase`][] manually.
    ///
    /// Caveat: Edges between nodes of different files are not used. Hence the returned set of partial
    /// paths will not cover paths going through those edges.
    ///
    /// [`process_next_phase`]: #method.process_next_phase
    pub fn find_minimal_partial_path_set_in_file<F>(
        graph: &StackGraph,
        partials: &mut PartialPaths,
        file: Handle<File>,
        config: StitcherConfig,
        cancellation_flag: &dyn CancellationFlag,
        mut visit: F,
    ) -> Result<Stats, CancellationError>
    where
        F: FnMut(&StackGraph, &mut PartialPaths, &PartialPath),
    {
        fn as_complete_as_necessary(graph: &StackGraph, path: &PartialPath) -> bool {
            path.starts_at_endpoint(graph)
                && (path.ends_at_endpoint(graph) || path.ends_in_jump(graph))
        }

        let initial_paths = graph
            .nodes_for_file(file)
            .chain(std::iter::once(StackGraph::root_node()))
            .filter(|node| graph[*node].is_endpoint())
            .map(|node| PartialPath::from_node(graph, partials, node))
            .collect::<Vec<_>>();
        let mut stitcher =
            ForwardPartialPathStitcher::from_partial_paths(graph, partials, initial_paths);
        config.apply(&mut stitcher);
        stitcher.set_check_only_join_nodes(true);

        let mut accepted_path_length = FrequencyDistribution::default();
        while !stitcher.is_complete() {
            cancellation_flag.check("finding complete partial paths")?;
            stitcher.process_next_phase(
                &mut GraphEdgeCandidates::new(graph, partials, Some(file)),
                |g, _ps, p| !as_complete_as_necessary(g, p),
            );
            for path in stitcher.previous_phase_partial_paths() {
                if as_complete_as_necessary(graph, path) {
                    accepted_path_length.record(path.edges.len());
                    visit(graph, partials, path);
                }
            }
        }

        Ok(Stats {
            accepted_path_length,
            ..stitcher.into_stats()
        })
    }
}

impl<H: Clone> ForwardPartialPathStitcher<H> {
    /// Finds all complete partial paths that are reachable from a set of starting nodes,
    /// building them up by stitching together partial paths from this database, and calling
    /// the `visit` closure on each one.
    ///
    /// This function will not return until all reachable partial paths have been processed, so
    /// your database must already contain all partial paths that might be needed.  If you have a
    /// very large stack graph stored in some other storage system, and want more control over
    /// lazily loading only the necessary pieces, then you should code up your own loop that calls
    /// [`process_next_phase`][] manually.
    ///
    /// [`process_next_phase`]: #method.process_next_phase
    pub fn find_all_complete_partial_paths<I, F, A, Db, C, Err>(
        candidates: &mut C,
        starting_nodes: I,
        config: StitcherConfig,
        cancellation_flag: &dyn CancellationFlag,
        mut visit: F,
    ) -> Result<Stats, Err>
    where
        I: IntoIterator<Item = Handle<Node>>,
        A: Appendable,
        Db: ToAppendable<H, A>,
        C: ForwardCandidates<H, A, Db, Err>,
        F: FnMut(&StackGraph, &mut PartialPaths, &PartialPath),
        Err: std::convert::From<CancellationError>,
    {
        let (graph, partials, _) = candidates.get_graph_partials_and_db();
        let initial_paths = starting_nodes
            .into_iter()
            .filter(|n| graph[*n].is_reference())
            .map(|n| {
                let mut p = PartialPath::from_node(graph, partials, n);
                p.eliminate_precondition_stack_variables(partials);
                p
            })
            .collect::<Vec<_>>();
        let mut stitcher =
            ForwardPartialPathStitcher::from_partial_paths(graph, partials, initial_paths);
        config.apply(&mut stitcher);
        stitcher.set_check_only_join_nodes(true);

        let mut accepted_path_length = FrequencyDistribution::default();
        while !stitcher.is_complete() {
            cancellation_flag.check("finding complete partial paths")?;
            for path in stitcher.previous_phase_partial_paths() {
                candidates.load_forward_candidates(path, cancellation_flag)?;
            }
            stitcher.process_next_phase(candidates, |_, _, _| true);
            let (graph, partials, _) = candidates.get_graph_partials_and_db();
            for path in stitcher.previous_phase_partial_paths() {
                if path.is_complete(graph) {
                    accepted_path_length.record(path.edges.len());
                    visit(graph, partials, path);
                }
            }
        }

        Ok(Stats {
            accepted_path_length,
            ..stitcher.into_stats()
        })
    }
}

impl ForwardPartialPathStitcher<Handle<PartialPath>> {
    /// Finds all complete partial paths that start at one reference, and releases the memory of
    /// the search when it ends.
    ///
    /// This method runs [`find_all_complete_partial_paths`][] inside a
    /// [`PartialPaths::with_scope`][].  A search adds cells to `partials`, and plain `partials`
    /// never gives them back.  Here, every cell that the search added drops when the method
    /// returns, even when `config` bounds the run or the cancellation flag stops it.  The
    /// memory of one reference does not add to the memory of the next one.
    ///
    /// The database is read-only.  The `visit` closure sees each complete path while the search
    /// is alive.  The path holds handles that the method drops at the end.  Copy the plain data
    /// that you need, for example the end node, and do not keep the path or its stacks.
    ///
    /// Check [`Stats::bounded`][] in the result.  A bounded search can lack paths.
    ///
    /// [`find_all_complete_partial_paths`]: #method.find_all_complete_partial_paths
    pub fn find_complete_partial_paths_in_scope<F>(
        graph: &StackGraph,
        partials: &mut PartialPaths,
        database: &Database,
        reference: Handle<Node>,
        config: StitcherConfig,
        cancellation_flag: &dyn CancellationFlag,
        visit: F,
    ) -> Result<Stats, CancellationError>
    where
        F: FnMut(&StackGraph, &mut PartialPaths, &PartialPath),
    {
        partials.with_scope(|partials| {
            let mut candidates = DatabaseCandidates::new(graph, partials, database);
            Self::find_all_complete_partial_paths(
                &mut candidates,
                std::iter::once(reference),
                config,
                cancellation_flag,
                visit,
            )
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// The distribution of the number of initial paths
    pub initial_paths: FrequencyDistribution<usize>,
    /// The distribution of the number of queued paths per stitching phase
    pub queued_paths_per_phase: FrequencyDistribution<usize>,
    /// The distribution of the number of processed paths per stitching phase
    pub processed_paths_per_phase: FrequencyDistribution<usize>,
    /// The distribution of the length of accepted paths
    pub accepted_path_length: FrequencyDistribution<usize>,
    /// The distribution of the maximal length of paths (when they cannot be extended more)
    pub terminal_path_lengh: FrequencyDistribution<usize>,
    /// The distribution of the number of candidates for paths ending in a regular node
    pub candidates_per_node_path: FrequencyDistribution<usize>,
    /// The distribution of the number of candidates for paths ending in the root node
    pub candidates_per_root_path: FrequencyDistribution<usize>,
    /// The distribution of the number of extensions (accepted candidates) for paths ending in a regular node
    pub extensions_per_node_path: FrequencyDistribution<usize>,
    /// The distribution of the number of extensions (accepted candidates) for paths ending in the root node
    pub extensions_per_root_path: FrequencyDistribution<usize>,
    /// The number of times the root node is visited
    pub root_visits: usize,
    /// The distribution of the number of times a regular node is visited
    pub node_visits: FrequencyDistribution<crate::graph::NodeID>,
    /// The distribution of the number of similar paths between node pairs.
    pub similar_paths_stats: SimilarPathStats,
    /// The limit that stopped the run or cut paths, if any.  A bounded run can lack results.
    /// Unlike the other fields, the stitcher records this field even when it does not collect
    /// statistics.
    pub bounded: Option<StitchBound>,
    /// The work done.  See [`StitcherLimits::max_work`].  Always recorded.
    pub work: usize,
    /// The largest number of paths that waited for extension.  Always recorded.
    pub peak_queue_len: usize,
    /// The number of cells that the run added to the `PartialPaths` arenas.  Recorded at each
    /// extension, so it can lag by one path.  Always recorded.
    pub arena_cells_added: usize,
}

impl std::ops::AddAssign<Self> for Stats {
    fn add_assign(&mut self, rhs: Self) {
        self.initial_paths += rhs.initial_paths;
        self.queued_paths_per_phase += rhs.queued_paths_per_phase;
        self.processed_paths_per_phase += rhs.processed_paths_per_phase;
        self.accepted_path_length += rhs.accepted_path_length;
        self.terminal_path_lengh += rhs.terminal_path_lengh;
        self.candidates_per_node_path += rhs.candidates_per_node_path;
        self.candidates_per_root_path += rhs.candidates_per_root_path;
        self.extensions_per_node_path += rhs.extensions_per_node_path;
        self.extensions_per_root_path += rhs.extensions_per_root_path;
        self.root_visits += rhs.root_visits;
        self.node_visits += rhs.node_visits;
        self.similar_paths_stats += rhs.similar_paths_stats;
        self.bounded = self.bounded.or(rhs.bounded);
        self.work += rhs.work;
        self.peak_queue_len = self.peak_queue_len.max(rhs.peak_queue_len);
        self.arena_cells_added += rhs.arena_cells_added;
    }
}

impl std::ops::AddAssign<&Self> for Stats {
    fn add_assign(&mut self, rhs: &Self) {
        self.initial_paths += &rhs.initial_paths;
        self.processed_paths_per_phase += &rhs.processed_paths_per_phase;
        self.accepted_path_length += &rhs.accepted_path_length;
        self.terminal_path_lengh += &rhs.terminal_path_lengh;
        self.candidates_per_node_path += &rhs.candidates_per_node_path;
        self.candidates_per_root_path += &rhs.candidates_per_root_path;
        self.extensions_per_node_path += &rhs.extensions_per_node_path;
        self.extensions_per_root_path += &rhs.extensions_per_root_path;
        self.root_visits += rhs.root_visits;
        self.node_visits += &rhs.node_visits;
        self.similar_paths_stats += &rhs.similar_paths_stats;
        self.bounded = self.bounded.or(rhs.bounded);
        self.work += rhs.work;
        self.peak_queue_len = self.peak_queue_len.max(rhs.peak_queue_len);
        self.arena_cells_added += rhs.arena_cells_added;
    }
}

/// The reason why a limit of a stitcher run had an effect.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StitchBound {
    /// The run exceeded [`StitcherLimits::max_work`].  The run stopped.
    Work,
    /// The run exceeded [`StitcherLimits::max_queue_len`].  The run stopped.
    QueueLength,
    /// The run exceeded [`StitcherLimits::max_arena_cells`].  The run stopped.
    ArenaCells,
    /// The run met a path longer than [`StitcherLimits::max_path_length`].  The run dropped that
    /// path and went on.
    PathLength,
}

impl std::fmt::Display for StitchBound {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            StitchBound::Work => "work",
            StitchBound::QueueLength => "queue-length",
            StitchBound::ArenaCells => "arena-cells",
            StitchBound::PathLength => "path-length",
        })
    }
}

/// Limits on the cost of one stitcher run.  A run is one call of
/// [`find_all_complete_partial_paths`][ForwardPartialPathStitcher::find_all_complete_partial_paths]
/// or one stitcher that you drive yourself.  Every limit is off by default, and a run without
/// limits behaves as before.
///
/// When a limit has an effect, the run reports it in [`Stats::bounded`].  A bounded run is not
/// the same as a run that found nothing.  The paths that the run found before it stopped stay
/// valid, but the set can lack paths.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StitcherLimits {
    /// The maximum work of the run.  The run does one unit of work for each path that it
    /// extends, and one more for each candidate that it appends to that path.  When the work
    /// exceeds the limit, the run stops.
    pub max_work: Option<usize>,
    /// The maximum number of paths that wait for extension.  When the number exceeds the limit,
    /// the run stops.
    pub max_queue_len: Option<usize>,
    /// The maximum number of cells that the run adds to the `PartialPaths` arenas.  This limit
    /// bounds the memory of the run.  When the number exceeds the limit, the run stops.
    pub max_arena_cells: Option<usize>,
    /// The maximum number of edges in a path.  The run drops longer paths and goes on.
    pub max_path_length: Option<usize>,
}

/// Configuration for partial path stitchers.
#[derive(Clone, Copy, Debug)]
pub struct StitcherConfig {
    /// Enables similar path detection during path stitching.
    detect_similar_paths: bool,
    /// Makes similar path detection keep a path that nothing can shadow.
    shadow_aware_similar_paths: bool,
    /// Collect statistics about path stitching.
    collect_stats: bool,
    /// Limits on the cost of one run.
    limits: StitcherLimits,
}

impl StitcherConfig {
    pub fn detect_similar_paths(&self) -> bool {
        self.detect_similar_paths
    }

    pub fn with_detect_similar_paths(mut self, detect_similar_paths: bool) -> Self {
        self.detect_similar_paths = detect_similar_paths;
        self
    }

    pub fn shadow_aware_similar_paths(&self) -> bool {
        self.shadow_aware_similar_paths
    }

    /// Sets whether similar path detection takes shadowing into account.  See
    /// [`ForwardPartialPathStitcher::set_shadow_aware_similar_paths`][].  The default is `false`.
    pub fn with_shadow_aware_similar_paths(mut self, shadow_aware_similar_paths: bool) -> Self {
        self.shadow_aware_similar_paths = shadow_aware_similar_paths;
        self
    }

    pub fn collect_stats(&self) -> bool {
        self.collect_stats
    }

    pub fn with_collect_stats(mut self, collect_stats: bool) -> Self {
        self.collect_stats = collect_stats;
        self
    }

    pub fn limits(&self) -> StitcherLimits {
        self.limits
    }

    /// Sets all limits of a run.  See [`StitcherLimits`][].
    pub fn with_limits(mut self, limits: StitcherLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Sets the work limit.  See [`StitcherLimits::max_work`].
    pub fn with_max_work(mut self, max_work: Option<usize>) -> Self {
        self.limits.max_work = max_work;
        self
    }

    /// Sets the queue limit.  See [`StitcherLimits::max_queue_len`].
    pub fn with_max_queue_len(mut self, max_queue_len: Option<usize>) -> Self {
        self.limits.max_queue_len = max_queue_len;
        self
    }

    /// Sets the arena limit.  See [`StitcherLimits::max_arena_cells`].
    pub fn with_max_arena_cells(mut self, max_arena_cells: Option<usize>) -> Self {
        self.limits.max_arena_cells = max_arena_cells;
        self
    }

    /// Sets the path length limit.  See [`StitcherLimits::max_path_length`].
    pub fn with_max_path_length(mut self, max_path_length: Option<usize>) -> Self {
        self.limits.max_path_length = max_path_length;
        self
    }
}

impl StitcherConfig {
    fn apply<H>(&self, stitcher: &mut ForwardPartialPathStitcher<H>) {
        stitcher.set_similar_path_detection(self.detect_similar_paths);
        stitcher.set_shadow_aware_similar_paths(self.shadow_aware_similar_paths);
        stitcher.set_collect_stats(self.collect_stats);
        stitcher.set_limits(self.limits);
    }
}

impl Default for StitcherConfig {
    fn default() -> Self {
        Self {
            detect_similar_paths: true,
            shadow_aware_similar_paths: false,
            collect_stats: false,
            limits: StitcherLimits::default(),
        }
    }
}
