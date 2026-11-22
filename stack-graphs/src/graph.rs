// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2021, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Defines the structure of a stack graph.
//!
//! This module contains all of the types that you need to define the structure of a particular
//! stack graph.
//!
//! The stack graph as a whole lives in an instance of [`StackGraph`][].  This type contains
//! several [`Arena`s][`Arena`], which are used to manage the life cycle of the data instances that
//! comprise the stack graph.  You cannot delete anything from the stack graph; all of its contents
//! are dropped in a single operation when the graph itself is dropped.
//!
//! [`Arena`]: ../arena/struct.Arena.html
//! [`StackGraph`]: struct.StackGraph.html
//!
//! There are several different kinds of node that can appear in a stack graph.  As we search for a
//! path representing a name binding, each kind of node has different rules for how it interacts
//! with the symbol and scope stacks:
//!
//!   - the singleton [_root node_][`RootNode`], which allows name binding paths to cross between
//!     files
//!   - [_scope_][`ScopeNode`] nodes, which define the name binding structure within a single file
//!   - [_push symbol_][`PushSymbolNode`] and [_push scoped symbol_][`PushScopedSymbolNode`] nodes,
//!     which push onto the symbol stack new things for us to look for
//!   - [_pop symbol_][`PopSymbolNode`] and [_pop scoped symbol_][`PopScopedSymbolNode`] nodes,
//!     which pop things off the symbol stack once they've been found
//!   - [_drop scopes_][`DropScopesNode`] and [_jump to scope_][`JumpToNode`] nodes, which
//!     manipulate the scope stack
//!
//! [`DropScopesNode`]: struct.DropScopesNode.html
//! [`JumpToNode`]: struct.JumpToNode.html
//! [`PushScopedSymbolNode`]: struct.PushScopedSymbolNode.html
//! [`PushSymbolNode`]: struct.PushSymbolNode.html
//! [`PopScopedSymbolNode`]: struct.PopScopedSymbolNode.html
//! [`PopSymbolNode`]: struct.PopSymbolNode.html
//! [`RootNode`]: struct.RootNode.html
//! [`ScopeNode`]: struct.ScopeNode.html
//!
//! All nodes except for the singleton _root node_ and _jump to scope_ node belong to
//! [files][`File`].
//!
//! Nodes are connected via [edges][`Edge`].
//!
//! [`Edge`]: struct.Edge.html
//! [`File`]: struct.File.html

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Display;
use std::num::NonZeroU32;
use std::ops::Index;
use std::ops::IndexMut;

use controlled_option::ControlledOption;
use either::Either;
use fxhash::FxHashMap;
use smallvec::SmallVec;

use crate::arena::Arena;
use crate::arena::Handle;
use crate::arena::SupplementalArena;

//-------------------------------------------------------------------------------------------------
// String content

#[repr(C)]
struct InternedStringContent {
    // See InternedStringArena below for how we fill in these fields safely.
    start: *const u8,
    len: usize,
}

const INITIAL_STRING_CAPACITY: usize = 512;

/// The content of each interned string is stored in one of the buffers inside of a
/// `InternedStringArena` instance, following the trick [described by Aleksey Kladov][interner].
///
/// The buffers stored in this type are preallocated, and are never allowed to grow.  That ensures
/// that pointers into the buffer are stable, as long as the buffer has not been destroyed.
/// (`InternedStringContent` instances are also stored in an arena, ensuring that the strings that
/// we hand out don't outlive the buffers.)
///
/// [interner]: https://matklad.github.io/2020/03/22/fast-simple-rust-interner.html
struct InternedStringArena {
    current_buffer: Vec<u8>,
    full_buffers: Vec<Vec<u8>>,
}

impl InternedStringArena {
    fn new() -> InternedStringArena {
        InternedStringArena {
            current_buffer: Vec::with_capacity(INITIAL_STRING_CAPACITY),
            full_buffers: Vec::new(),
        }
    }

    // Adds a new string.  This does not check whether we've already stored a string with the same
    // content; that is handled down below in `StackGraph::add_symbol` and `add_file`.
    fn add(&mut self, value: &str) -> InternedStringContent {
        // Is there enough room in current_buffer to hold this string?
        let value = value.as_bytes();
        let len = value.len();
        let capacity = self.current_buffer.capacity();
        let remaining_capacity = capacity - self.current_buffer.len();
        if len > remaining_capacity {
            // If not, move current_buffer over into full_buffers (so that we hang onto it until
            // we're dropped) and allocate a new current_buffer that's at least big enough to hold
            // this string.
            let new_capacity = (capacity.max(len) + 1).next_power_of_two();
            let new_buffer = Vec::with_capacity(new_capacity);
            let old_buffer = std::mem::replace(&mut self.current_buffer, new_buffer);
            self.full_buffers.push(old_buffer);
        }

        // Copy the string's content into current_buffer and return a pointer to it.  That pointer
        // is stable since we never allow the current_buffer to be resized — once we run out of
        // room, we allocate a _completely new buffer_ to replace it.
        let start_index = self.current_buffer.len();
        self.current_buffer.extend_from_slice(value);
        let start = unsafe { self.current_buffer.as_ptr().add(start_index) };
        InternedStringContent { start, len }
    }
}

impl InternedStringContent {
    /// Returns the content of this string as a `str`.  This is safe as long as the lifetime of the
    /// InternedStringContent is outlived by the lifetime of the InternedStringArena that holds its
    /// data.  That is guaranteed because we store the InternedStrings in an Arena alongside the
    /// InternedStringArena, and only hand out references to them.
    fn as_str(&self) -> &str {
        unsafe {
            let bytes = std::slice::from_raw_parts(self.start, self.len);
            std::str::from_utf8_unchecked(bytes)
        }
    }

    // Returns a supposedly 'static reference to the string's data.  The string data isn't really
    // static, but we are careful only to use this as a key in the HashMap that StackGraph uses to
    // track whether we've stored a particular symbol already.  That HashMap lives alongside the
    // InternedStringArena that holds the data, so we can get away with a technically incorrect
    // 'static lifetime here.  As an extra precaution, this method is is marked as unsafe so that
    // we don't inadvertently call it from anywhere else in the crate.
    unsafe fn as_hash_key(&self) -> &'static str {
        let bytes = std::slice::from_raw_parts(self.start, self.len);
        std::str::from_utf8_unchecked(bytes)
    }
}

unsafe impl Send for InternedStringContent {}
unsafe impl Sync for InternedStringContent {}

//-------------------------------------------------------------------------------------------------
// Symbols

/// A name that we are trying to resolve using stack graphs.
///
/// This typically represents a portion of an identifier as it appears in the source language.  It
/// can also represent some other "operation" that can occur in source code, and which needs to be
/// modeled in a stack graph — for instance, many languages will use a "fake" symbol named `.` to
/// represent member access.
///
/// We deduplicate `Symbol` instances in a `StackGraph` — that is, we ensure that there are never
/// multiple `Symbol` instances with the same content.  That means that you can compare _handles_
/// to symbols using simple equality, without having to dereference into the `StackGraph` arena.
#[repr(C)]
pub struct Symbol {
    content: InternedStringContent,
}

impl Symbol {
    pub fn as_str(&self) -> &str {
        self.content.as_str()
    }
}

impl PartialEq<&str> for Symbol {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl StackGraph {
    /// Adds a symbol to the stack graph, ensuring that there's only ever one copy of a particular
    /// symbol stored in the graph.
    pub fn add_symbol<S: AsRef<str> + ?Sized>(&mut self, symbol: &S) -> Handle<Symbol> {
        let symbol = symbol.as_ref();
        if let Some(handle) = self.symbol_handles.get(symbol) {
            return *handle;
        }

        let interned = self.interned_strings.add(symbol);
        let hash_key = unsafe { interned.as_hash_key() };
        let handle = self.symbols.add(Symbol { content: interned });
        self.symbol_handles.insert(hash_key, handle);
        handle
    }

    /// Returns an iterator over all of the handles of all of the symbols in this stack graph.
    /// (Note that because we're only returning _handles_, this iterator does not retain a
    /// reference to the `StackGraph`.)
    pub fn iter_symbols(&self) -> impl Iterator<Item = Handle<Symbol>> {
        self.symbols.iter_handles()
    }
}

impl Index<Handle<Symbol>> for StackGraph {
    type Output = str;
    #[inline(always)]
    fn index(&self, handle: Handle<Symbol>) -> &str {
        self.symbols.get(handle).as_str()
    }
}

#[doc(hidden)]
pub struct DisplaySymbol<'a> {
    wrapped: Handle<Symbol>,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplaySymbol<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", &self.graph[self.wrapped])
    }
}

impl Handle<Symbol> {
    pub fn display(self, graph: &StackGraph) -> impl Display + '_ {
        DisplaySymbol {
            wrapped: self,
            graph,
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Interned strings

/// Arbitrary string content associated with some part of a stack graph.
#[repr(C)]
pub struct InternedString {
    content: InternedStringContent,
}

impl InternedString {
    fn as_str(&self) -> &str {
        self.content.as_str()
    }
}

impl PartialEq<&str> for InternedString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl StackGraph {
    /// Adds an interned string to the stack graph, ensuring that there's only ever one copy of a
    /// particular string stored in the graph.
    pub fn add_string<S: AsRef<str> + ?Sized>(&mut self, string: &S) -> Handle<InternedString> {
        let string = string.as_ref();
        if let Some(handle) = self.string_handles.get(string) {
            return *handle;
        }

        let interned = self.interned_strings.add(string);
        let hash_key = unsafe { interned.as_hash_key() };
        let handle = self.strings.add(InternedString { content: interned });
        self.string_handles.insert(hash_key, handle);
        handle
    }

    /// Returns an iterator over all of the handles of all of the interned strings in this stack
    /// graph. (Note that because we're only returning _handles_, this iterator does not retain a
    /// reference to the `StackGraph`.)
    pub fn iter_strings(&self) -> impl Iterator<Item = Handle<InternedString>> {
        self.strings.iter_handles()
    }
}

impl Index<Handle<InternedString>> for StackGraph {
    type Output = str;
    #[inline(always)]
    fn index(&self, handle: Handle<InternedString>) -> &str {
        self.strings.get(handle).as_str()
    }
}

#[doc(hidden)]
pub struct DisplayInternedString<'a> {
    wrapped: Handle<InternedString>,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayInternedString<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", &self.graph[self.wrapped])
    }
}

impl Handle<InternedString> {
    pub fn display(self, graph: &StackGraph) -> impl Display + '_ {
        DisplayInternedString {
            wrapped: self,
            graph,
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Files

/// A source file that we have extracted stack graph data from.
///
/// It's up to you to choose what names to use for your files, but they must be unique within a
/// stack graph.  If you are analyzing files from the local filesystem, the file's path is a good
/// choice.  If your files belong to packages or repositories, they should include the package or
/// repository IDs to make sure that files in different packages or repositories don't clash with
/// each other.
pub struct File {
    /// The name of this source file.
    name: InternedStringContent,
}

impl File {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl StackGraph {
    /// Adds a file to the stack graph.  There can only ever be one file with a particular name in
    /// the graph.  If a file with the requested name already exists, we return `Err`; if it
    /// doesn't already exist, we return `Ok`.  In both cases, the value of the result is the
    /// file's handle.
    pub fn add_file<S: AsRef<str> + ?Sized>(
        &mut self,
        name: &S,
    ) -> Result<Handle<File>, Handle<File>> {
        let name = name.as_ref();
        if let Some(handle) = self.file_handles.get(name) {
            return Err(*handle);
        }

        let interned = self.interned_strings.add(name);
        let hash_key = unsafe { interned.as_hash_key() };
        let handle = self.files.add(File { name: interned });
        self.file_handles.insert(hash_key, handle);
        Ok(handle)
    }

    /// Adds a file to the stack graph, returning its handle.  There can only ever be one file with
    /// a particular name in the graph, so if you call this multiple times with the same name,
    /// you'll get the same handle each time.
    #[inline(always)]
    pub fn get_or_create_file<S: AsRef<str> + ?Sized>(&mut self, name: &S) -> Handle<File> {
        self.add_file(name).unwrap_or_else(|handle| handle)
    }

    /// Returns the file with a particular name, if it exists.
    pub fn get_file<S: AsRef<str> + ?Sized>(&self, name: &S) -> Option<Handle<File>> {
        let name = name.as_ref();
        self.file_handles.get(name).copied()
    }
}

impl StackGraph {
    /// Returns an iterator of all of the nodes that belong to a particular file.  Note that this
    /// does **_not_** include the singleton _root_ or _jump to scope_ nodes.
    pub fn nodes_for_file(&self, file: Handle<File>) -> impl Iterator<Item = Handle<Node>> + '_ {
        self.node_id_handles.nodes_for_file(file)
    }

    /// Returns an iterator over all of the handles of all of the files in this stack graph.  (Note
    /// that because we're only returning _handles_, this iterator does not retain a reference to
    /// the `StackGraph`.)
    pub fn iter_files(&self) -> impl Iterator<Item = Handle<File>> + '_ {
        self.files.iter_handles()
    }
}

impl Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Index<Handle<File>> for StackGraph {
    type Output = File;
    #[inline(always)]
    fn index(&self, handle: Handle<File>) -> &File {
        &self.files.get(handle)
    }
}

#[doc(hidden)]
pub struct DisplayFile<'a> {
    wrapped: Handle<File>,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayFile<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.graph[self.wrapped])
    }
}

impl Handle<File> {
    pub fn display(self, graph: &StackGraph) -> impl Display + '_ {
        DisplayFile {
            wrapped: self,
            graph,
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Nodes

/// Uniquely identifies a node in a stack graph.
///
/// Each node (except for the _root node_ and _jump to scope_ node) lives in a file, and has a
/// _local ID_ that must be unique within its file.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeID {
    file: ControlledOption<Handle<File>>,
    local_id: u32,
}

pub(crate) const ROOT_NODE_ID: u32 = 1;
pub(crate) const JUMP_TO_NODE_ID: u32 = 2;

impl NodeID {
    /// Returns the ID of the singleton _root node_.
    #[inline(always)]
    pub fn root() -> NodeID {
        NodeID {
            file: ControlledOption::none(),
            local_id: ROOT_NODE_ID,
        }
    }

    /// Returns the ID of the singleton _jump to scope_ node.
    #[inline(always)]
    pub fn jump_to() -> NodeID {
        NodeID {
            file: ControlledOption::none(),
            local_id: JUMP_TO_NODE_ID,
        }
    }

    /// Creates a new file-local node ID.
    #[inline(always)]
    pub fn new_in_file(file: Handle<File>, local_id: u32) -> NodeID {
        NodeID {
            file: ControlledOption::some(file),
            local_id,
        }
    }

    /// Returns whether this ID refers to the singleton _root node_.
    #[inline(always)]
    pub fn is_root(self) -> bool {
        self.file.is_none() && self.local_id == ROOT_NODE_ID
    }

    /// Returns whether this ID refers to the singleton _jump to scope_ node.
    #[inline(always)]
    pub fn is_jump_to(self) -> bool {
        self.file.is_none() && self.local_id == JUMP_TO_NODE_ID
    }

    /// Returns the file that this node belongs to.  Returns `None` for the singleton _root_ and
    /// _jump to scope_ nodes, which belong to all files.
    #[inline(always)]
    pub fn file(self) -> Option<Handle<File>> {
        self.file.into()
    }

    /// Returns the local ID of this node within its file.  Panics if this node ID refers to the
    /// singleton _root_ or _jump to scope_ nodes.
    #[inline(always)]
    pub fn local_id(self) -> u32 {
        self.local_id
    }

    /// Returns whether this node belongs to a particular file.  Always returns `true` for the
    /// singleton _root_ and _jump to scope_ nodes, which belong to all files.
    #[inline(always)]
    pub fn is_in_file(self, file: Handle<File>) -> bool {
        match self.file.into_option() {
            Some(this_file) => this_file == file,
            _ => true,
        }
    }
}

#[doc(hidden)]
pub struct DisplayNodeID<'a> {
    wrapped: NodeID,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayNodeID<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.wrapped.file.into_option() {
            Some(file) => write!(f, "{}({})", file.display(self.graph), self.wrapped.local_id),
            None => {
                if self.wrapped.is_root() {
                    write!(f, "[root]")
                } else if self.wrapped.is_jump_to() {
                    write!(f, "[jump]")
                } else {
                    unreachable!();
                }
            }
        }
    }
}

impl NodeID {
    pub fn display(self, graph: &StackGraph) -> impl Display + '_ {
        DisplayNodeID {
            wrapped: self,
            graph,
        }
    }
}

/// A node in a stack graph.
#[repr(C)]
pub enum Node {
    DropScopes(DropScopesNode),
    JumpTo(JumpToNode),
    PopScopedSymbol(PopScopedSymbolNode),
    PopSymbol(PopSymbolNode),
    PushScopedSymbol(PushScopedSymbolNode),
    PushSymbol(PushSymbolNode),
    Root(RootNode),
    Scope(ScopeNode),
}

impl Node {
    #[inline(always)]
    pub fn is_exported_scope(&self) -> bool {
        match self {
            Node::Scope(node) => node.is_exported,
            _ => false,
        }
    }

    #[inline(always)]
    pub fn is_definition(&self) -> bool {
        match self {
            Node::PopScopedSymbol(node) => node.is_definition,
            Node::PopSymbol(node) => node.is_definition,
            _ => false,
        }
    }

    #[inline(always)]
    pub fn is_reference(&self) -> bool {
        match self {
            Node::PushScopedSymbol(node) => node.is_reference,
            Node::PushSymbol(node) => node.is_reference,
            _ => false,
        }
    }

    #[inline(always)]
    pub fn is_jump_to(&self) -> bool {
        matches!(self, Node::JumpTo(_))
    }

    #[inline(always)]
    pub fn is_root(&self) -> bool {
        matches!(self, Node::Root(_))
    }

    #[inline(always)]
    pub fn is_endpoint(&self) -> bool {
        self.is_definition() || self.is_exported_scope() || self.is_reference() || self.is_root()
    }

    /// Returns this node's symbol, if it has one.  (_Pop symbol_, _pop scoped symbol_, _push
    /// symbol_, and _push scoped symbol_ nodes have symbols.)
    pub fn symbol(&self) -> Option<Handle<Symbol>> {
        match self {
            Node::PushScopedSymbol(node) => Some(node.symbol),
            Node::PushSymbol(node) => Some(node.symbol),
            Node::PopScopedSymbol(node) => Some(node.symbol),
            Node::PopSymbol(node) => Some(node.symbol),
            _ => None,
        }
    }

    /// Returns this node's attached scope, if it has one.  (_Push scoped symbol_ nodes have
    /// attached scopes.)
    pub fn scope(&self) -> Option<NodeID> {
        match self {
            Node::PushScopedSymbol(node) => Some(node.scope),
            _ => None,
        }
    }

    /// Returns the ID of this node.
    pub fn id(&self) -> NodeID {
        match self {
            Node::DropScopes(node) => node.id,
            Node::JumpTo(node) => node.id,
            Node::PushScopedSymbol(node) => node.id,
            Node::PushSymbol(node) => node.id,
            Node::PopScopedSymbol(node) => node.id,
            Node::PopSymbol(node) => node.id,
            Node::Root(node) => node.id,
            Node::Scope(node) => node.id,
        }
    }

    /// Returns the file that this node belongs to.  Returns `None` for the singleton _root_ and
    /// _jump to scope_ nodes, which belong to all files.
    #[inline(always)]
    pub fn file(&self) -> Option<Handle<File>> {
        self.id().file()
    }

    /// Returns whether this node belongs to a particular file.  Always returns `true` for the
    /// singleton _root_ and _jump to scope_ nodes, which belong to all files.
    #[inline(always)]
    pub fn is_in_file(&self, file: Handle<File>) -> bool {
        self.id().is_in_file(file)
    }

    pub fn display<'a>(&'a self, graph: &'a StackGraph) -> impl Display + 'a {
        DisplayNode {
            wrapped: self,
            graph,
        }
    }
}

impl StackGraph {
    /// Returns a handle to the stack graph's singleton _jump to scope_ node.
    #[inline(always)]
    pub fn jump_to_node() -> Handle<Node> {
        Handle::new(unsafe { NonZeroU32::new_unchecked(2) })
    }

    /// Returns a handle to the stack graph's singleton _root node_.
    #[inline(always)]
    pub fn root_node() -> Handle<Node> {
        Handle::new(unsafe { NonZeroU32::new_unchecked(1) })
    }

    /// Returns an unused [`NodeID`][] for the given file.
    ///
    /// [`NodeID`]: struct.NodeID.html
    pub fn new_node_id(&mut self, file: Handle<File>) -> NodeID {
        self.node_id_handles.unused_id(file)
    }

    /// Returns an iterator of all of the nodes in the graph.  (Note that because we're only
    /// returning _handles_, this iterator does not retain a reference to the `StackGraph`.)
    pub fn iter_nodes(&self) -> impl Iterator<Item = Handle<Node>> {
        self.nodes.iter_handles()
    }

    /// Returns the handle to the node with a particular ID, if it exists.
    pub fn node_for_id(&self, id: NodeID) -> Option<Handle<Node>> {
        if id.file().is_some() {
            self.node_id_handles.try_handle_for_id(id)
        } else if id.is_root() {
            Some(StackGraph::root_node())
        } else if id.is_jump_to() {
            Some(StackGraph::jump_to_node())
        } else {
            None
        }
    }

    pub(crate) fn add_node(&mut self, id: NodeID, node: Node) -> Option<Handle<Node>> {
        if let Some(_) = self.node_id_handles.handle_for_id(id) {
            return None;
        }
        let handle = self.nodes.add(node);
        self.node_id_handles.set_handle_for_id(id, handle);
        Some(handle)
    }

    pub(crate) fn get_or_create_node(&mut self, id: NodeID, node: Node) -> Handle<Node> {
        if let Some(handle) = self.node_id_handles.handle_for_id(id) {
            return handle;
        }
        let handle = self.nodes.add(node);
        self.node_id_handles.set_handle_for_id(id, handle);
        handle
    }
}

#[doc(hidden)]
pub struct DisplayNode<'a> {
    wrapped: &'a Node,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.wrapped {
            Node::DropScopes(node) => node.display(self.graph).fmt(f),
            Node::JumpTo(node) => node.fmt(f),
            Node::PushScopedSymbol(node) => node.display(self.graph).fmt(f),
            Node::PushSymbol(node) => node.display(self.graph).fmt(f),
            Node::PopScopedSymbol(node) => node.display(self.graph).fmt(f),
            Node::PopSymbol(node) => node.display(self.graph).fmt(f),
            Node::Root(node) => node.fmt(f),
            Node::Scope(node) => node.display(self.graph).fmt(f),
        }
    }
}

impl Handle<Node> {
    pub fn display(self, graph: &StackGraph) -> impl Display + '_ {
        DisplayNode {
            wrapped: &graph[self],
            graph,
        }
    }
}

impl Index<Handle<Node>> for StackGraph {
    type Output = Node;
    #[inline(always)]
    fn index(&self, handle: Handle<Node>) -> &Node {
        self.nodes.get(handle)
    }
}

impl IndexMut<Handle<Node>> for StackGraph {
    #[inline(always)]
    fn index_mut(&mut self, handle: Handle<Node>) -> &mut Node {
        self.nodes.get_mut(handle)
    }
}

/// Removes everything from the current scope stack.
#[repr(C)]
pub struct DropScopesNode {
    /// The unique identifier for this node.
    pub id: NodeID,
    _symbol: ControlledOption<Handle<Symbol>>,
    _scope: NodeID,
    _is_endpoint: bool,
}

impl From<DropScopesNode> for Node {
    fn from(node: DropScopesNode) -> Node {
        Node::DropScopes(node)
    }
}

impl StackGraph {
    /// Adds a _drop scopes_ node to the stack graph.
    pub fn add_drop_scopes_node(&mut self, id: NodeID) -> Option<Handle<Node>> {
        let node = DropScopesNode {
            id,
            _symbol: ControlledOption::none(),
            _scope: NodeID::default(),
            _is_endpoint: false,
        };
        self.add_node(id, node.into())
    }
}

impl DropScopesNode {
    pub fn display<'a>(&'a self, graph: &'a StackGraph) -> impl Display + 'a {
        DisplayDropScopesNode {
            wrapped: self,
            graph,
        }
    }
}

#[doc(hidden)]
pub struct DisplayDropScopesNode<'a> {
    wrapped: &'a DropScopesNode,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayDropScopesNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "[{}]", self.wrapped.id.display(self.graph))
        } else {
            write!(f, "[{} drop scopes]", self.wrapped.id.display(self.graph))
        }
    }
}

/// The singleton "jump to" node, which allows a name binding path to jump back to another part of
/// the graph.
#[repr(C)]
pub struct JumpToNode {
    id: NodeID,
    _symbol: ControlledOption<Handle<Symbol>>,
    _scope: NodeID,
    _is_endpoint: bool,
}

impl From<JumpToNode> for Node {
    fn from(node: JumpToNode) -> Node {
        Node::JumpTo(node)
    }
}

impl JumpToNode {
    fn new() -> JumpToNode {
        JumpToNode {
            id: NodeID::jump_to(),
            _symbol: ControlledOption::none(),
            _scope: NodeID::default(),
            _is_endpoint: false,
        }
    }
}

impl Display for JumpToNode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[jump to scope]")
    }
}

/// Pops a scoped symbol from the symbol stack.  If the top of the symbol stack doesn't match the
/// requested symbol, or if the top of the symbol stack doesn't have an attached scope list, then
/// the path is not allowed to enter this node.
#[repr(C)]
pub struct PopScopedSymbolNode {
    /// The unique identifier for this node.
    pub id: NodeID,
    /// The symbol to pop off the symbol stack.
    pub symbol: Handle<Symbol>,
    _scope: NodeID,
    /// Whether this node represents a reference in the source language.
    pub is_definition: bool,
}

impl From<PopScopedSymbolNode> for Node {
    fn from(node: PopScopedSymbolNode) -> Node {
        Node::PopScopedSymbol(node)
    }
}

impl StackGraph {
    /// Adds a _pop scoped symbol_ node to the stack graph.
    pub fn add_pop_scoped_symbol_node(
        &mut self,
        id: NodeID,
        symbol: Handle<Symbol>,
        is_definition: bool,
    ) -> Option<Handle<Node>> {
        let node = PopScopedSymbolNode {
            id,
            symbol,
            _scope: NodeID::default(),
            is_definition,
        };
        self.add_node(id, node.into())
    }
}

impl PopScopedSymbolNode {
    pub fn display<'a>(&'a self, graph: &'a StackGraph) -> impl Display + 'a {
        DisplayPopScopedSymbolNode {
            wrapped: self,
            graph,
        }
    }
}

#[doc(hidden)]
pub struct DisplayPopScopedSymbolNode<'a> {
    wrapped: &'a PopScopedSymbolNode,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayPopScopedSymbolNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "[{}]", self.wrapped.id.display(self.graph))
        } else {
            write!(
                f,
                "[{} {} {}]",
                self.wrapped.id.display(self.graph),
                if self.wrapped.is_definition {
                    "scoped definition"
                } else {
                    "pop scoped"
                },
                self.wrapped.symbol.display(self.graph),
            )
        }
    }
}

/// Pops a symbol from the symbol stack.  If the top of the symbol stack doesn't match the
/// requested symbol, then the path is not allowed to enter this node.
#[repr(C)]
pub struct PopSymbolNode {
    /// The unique identifier for this node.
    pub id: NodeID,
    /// The symbol to pop off the symbol stack.
    pub symbol: Handle<Symbol>,
    _scope: NodeID,
    /// Whether this node represents a reference in the source language.
    pub is_definition: bool,
}

impl From<PopSymbolNode> for Node {
    fn from(node: PopSymbolNode) -> Node {
        Node::PopSymbol(node)
    }
}

impl StackGraph {
    /// Adds a _pop symbol_ node to the stack graph.
    pub fn add_pop_symbol_node(
        &mut self,
        id: NodeID,
        symbol: Handle<Symbol>,
        is_definition: bool,
    ) -> Option<Handle<Node>> {
        let node = PopSymbolNode {
            id,
            symbol,
            _scope: NodeID::default(),
            is_definition,
        };
        self.add_node(id, node.into())
    }
}

impl PopSymbolNode {
    pub fn display<'a>(&'a self, graph: &'a StackGraph) -> impl Display + 'a {
        DisplayPopSymbolNode {
            wrapped: self,
            graph,
        }
    }
}

#[doc(hidden)]
pub struct DisplayPopSymbolNode<'a> {
    wrapped: &'a PopSymbolNode,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayPopSymbolNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "[{}]", self.wrapped.id.display(self.graph))
        } else {
            write!(
                f,
                "[{} {} {}]",
                self.wrapped.id.display(self.graph),
                if self.wrapped.is_definition {
                    "definition"
                } else {
                    "pop"
                },
                self.wrapped.symbol.display(self.graph),
            )
        }
    }
}

/// Pushes a scoped symbol onto the symbol stack.
#[repr(C)]
pub struct PushScopedSymbolNode {
    /// The unique identifier for this node.
    pub id: NodeID,
    /// The symbol to push onto the symbol stack.
    pub symbol: Handle<Symbol>,
    /// The exported scope node that should be attached to the scoped symbol.  The node ID must
    /// refer to an exported scope node.
    pub scope: NodeID,
    /// Whether this node represents a reference in the source language.
    pub is_reference: bool,
    _phantom: (),
}

impl From<PushScopedSymbolNode> for Node {
    fn from(node: PushScopedSymbolNode) -> Node {
        Node::PushScopedSymbol(node)
    }
}

impl StackGraph {
    /// Adds a _push scoped symbol_ node to the stack graph.
    pub fn add_push_scoped_symbol_node(
        &mut self,
        id: NodeID,
        symbol: Handle<Symbol>,
        scope: NodeID,
        is_reference: bool,
    ) -> Option<Handle<Node>> {
        let node = PushScopedSymbolNode {
            id,
            symbol,
            scope,
            is_reference,
            _phantom: (),
        };
        self.add_node(id, node.into())
    }
}

impl PushScopedSymbolNode {
    pub fn display<'a>(&'a self, graph: &'a StackGraph) -> impl Display + 'a {
        DisplayPushScopedSymbolNode {
            wrapped: self,
            graph,
        }
    }
}

#[doc(hidden)]
pub struct DisplayPushScopedSymbolNode<'a> {
    wrapped: &'a PushScopedSymbolNode,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayPushScopedSymbolNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "[{}]", self.wrapped.id.display(self.graph))
        } else {
            write!(
                f,
                "[{} {} {} {}]",
                self.wrapped.id.display(self.graph),
                if self.wrapped.is_reference {
                    "scoped reference"
                } else {
                    "push scoped"
                },
                self.wrapped.symbol.display(self.graph),
                self.wrapped.scope.display(self.graph),
            )
        }
    }
}

/// Pushes a symbol onto the symbol stack.
#[repr(C)]
pub struct PushSymbolNode {
    /// The unique identifier for this node.
    pub id: NodeID,
    /// The symbol to push onto the symbol stack.
    pub symbol: Handle<Symbol>,
    _scope: NodeID,
    /// Whether this node represents a reference in the source language.
    pub is_reference: bool,
}

impl From<PushSymbolNode> for Node {
    fn from(node: PushSymbolNode) -> Node {
        Node::PushSymbol(node)
    }
}

impl StackGraph {
    /// Adds a _push symbol_ node to the stack graph.
    pub fn add_push_symbol_node(
        &mut self,
        id: NodeID,
        symbol: Handle<Symbol>,
        is_reference: bool,
    ) -> Option<Handle<Node>> {
        let node = PushSymbolNode {
            id,
            symbol,
            _scope: NodeID::default(),
            is_reference,
        };
        self.add_node(id, node.into())
    }
}

impl PushSymbolNode {
    pub fn display<'a>(&'a self, graph: &'a StackGraph) -> impl Display + 'a {
        DisplayPushSymbolNode {
            wrapped: self,
            graph,
        }
    }
}

#[doc(hidden)]
pub struct DisplayPushSymbolNode<'a> {
    wrapped: &'a PushSymbolNode,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayPushSymbolNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "[{}]", self.wrapped.id.display(self.graph))
        } else {
            write!(
                f,
                "[{} {} {}]",
                self.wrapped.id.display(self.graph),
                if self.wrapped.is_reference {
                    "reference"
                } else {
                    "push"
                },
                self.wrapped.symbol.display(self.graph),
            )
        }
    }
}

/// The singleton root node, which allows a name binding path to cross between files.
#[repr(C)]
pub struct RootNode {
    id: NodeID,
    _symbol: ControlledOption<Handle<Symbol>>,
    _scope: NodeID,
    _is_endpoint: bool,
}

impl From<RootNode> for Node {
    fn from(node: RootNode) -> Node {
        Node::Root(node)
    }
}

impl RootNode {
    fn new() -> RootNode {
        RootNode {
            id: NodeID::root(),
            _symbol: ControlledOption::none(),
            _scope: NodeID::default(),
            _is_endpoint: false,
        }
    }
}

impl Display for RootNode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[root]")
    }
}

struct NodeIDHandles {
    files: SupplementalArena<File, Vec<Option<Handle<Node>>>>,
}

impl NodeIDHandles {
    fn new() -> NodeIDHandles {
        NodeIDHandles {
            files: SupplementalArena::new(),
        }
    }

    fn try_handle_for_id(&self, node_id: NodeID) -> Option<Handle<Node>> {
        let file_entry = self.files.get(node_id.file().unwrap())?;
        let node_index = node_id.local_id as usize;
        if node_index >= file_entry.len() {
            return None;
        }
        file_entry[node_index]
    }

    fn handle_for_id(&mut self, node_id: NodeID) -> Option<Handle<Node>> {
        let file_entry = &mut self.files[node_id.file().unwrap()];
        let node_index = node_id.local_id as usize;
        if node_index >= file_entry.len() {
            file_entry.resize(node_index + 1, None);
        }
        file_entry[node_index]
    }

    fn set_handle_for_id(&mut self, node_id: NodeID, handle: Handle<Node>) {
        let file_entry = &mut self.files[node_id.file().unwrap()];
        let node_index = node_id.local_id as usize;
        file_entry[node_index] = Some(handle);
    }

    fn unused_id(&mut self, file: Handle<File>) -> NodeID {
        let local_id = self
            .files
            .get(file)
            .map(|file_entry| file_entry.len() as u32)
            .unwrap_or(0);
        NodeID::new_in_file(file, local_id)
    }

    fn nodes_for_file(&self, file: Handle<File>) -> impl Iterator<Item = Handle<Node>> + '_ {
        let file_entry = match self.files.get(file) {
            Some(file_entry) => file_entry,
            None => return Either::Left(std::iter::empty()),
        };
        Either::Right(file_entry.iter().filter_map(|entry| *entry))
    }
}

/// A node that adds structure to the graph. If the node is exported, it can be
/// referred to on the scope stack, which allows "jump to" nodes in any other
/// part of the graph can jump back here.
#[repr(C)]
pub struct ScopeNode {
    /// The unique identifier for this node.
    pub id: NodeID,
    _symbol: ControlledOption<Handle<Symbol>>,
    _scope: NodeID,
    pub is_exported: bool,
}

impl From<ScopeNode> for Node {
    fn from(node: ScopeNode) -> Node {
        Node::Scope(node)
    }
}

impl StackGraph {
    /// Adds a _scope_ node to the stack graph.
    pub fn add_scope_node(&mut self, id: NodeID, is_exported: bool) -> Option<Handle<Node>> {
        let node = ScopeNode {
            id,
            _symbol: ControlledOption::none(),
            _scope: NodeID::default(),
            is_exported,
        };
        self.add_node(id, node.into())
    }
}

impl ScopeNode {
    pub fn display<'a>(&'a self, graph: &'a StackGraph) -> impl Display + 'a {
        DisplayScopeNode {
            wrapped: self,
            graph,
        }
    }
}

#[doc(hidden)]
pub struct DisplayScopeNode<'a> {
    wrapped: &'a ScopeNode,
    graph: &'a StackGraph,
}

impl<'a> Display for DisplayScopeNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "[{}]", self.wrapped.id.display(self.graph))
        } else {
            write!(
                f,
                "[{}{} scope]",
                self.wrapped.id.display(self.graph),
                if self.wrapped.is_exported {
                    " exported"
                } else {
                    ""
                },
            )
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Edges

/// Connects two nodes in a stack graph.
///
/// These edges provide the basic graph connectivity that allow us to search for name binding paths
/// in a stack graph.  (Though not all sequence of edges is a well-formed name binding: the nodes
/// that you encounter along the path must also satisfy all of the rules for maintaining correct
/// symbol and scope stacks.)
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Edge {
    pub source: Handle<Node>,
    pub sink: Handle<Node>,
    pub precedence: i32,
}

pub(crate) struct OutgoingEdge {
    sink: Handle<Node>,
    precedence: i32,
}

impl StackGraph {
    /// Adds a new edge to the stack graph.
    pub fn add_edge(&mut self, source: Handle<Node>, sink: Handle<Node>, precedence: i32) {
        let edges = &mut self.outgoing_edges[source];
        if let Err(index) = edges.binary_search_by_key(&sink, |o| o.sink) {
            edges.insert(index, OutgoingEdge { sink, precedence });
            self.incoming_edges[sink] += Degree::One;
        }
    }

    /// Sets edge precedence of the given edge.
    pub fn set_edge_precedence(
        &mut self,
        source: Handle<Node>,
        sink: Handle<Node>,
        precedence: i32,
    ) {
        let edges = &mut self.outgoing_edges[source];
        if let Ok(index) = edges.binary_search_by_key(&sink, |o| o.sink) {
            edges[index].precedence = precedence;
        }
    }

    /// Returns an iterator of all of the edges that begin at a particular source node.
    pub fn outgoing_edges(&self, source: Handle<Node>) -> impl Iterator<Item = Edge> + '_ {
        match self.outgoing_edges.get(source) {
            Some(edges) => Either::Right(edges.iter().map(move |o| Edge {
                source,
                sink: o.sink,
                precedence: o.precedence,
            })),
            None => Either::Left(std::iter::empty()),
        }
    }

    /// Returns the number of edges that end at a particular sink node.
    pub fn incoming_edge_degree(&self, sink: Handle<Node>) -> Degree {
        self.incoming_edges
            .get(sink)
            .cloned()
            .unwrap_or(Degree::Zero)
    }
}

//-------------------------------------------------------------------------------------------------
// Source code

/// Contains information about a range of code in a source code file.
#[repr(C)]
#[derive(Clone, Default)]
pub struct SourceInfo {
    /// The location in its containing file of the source code that this node represents.
    pub span: lsp_positions::Span,
    /// The kind of syntax entity this node represents (e.g. `function`, `class`, `method`, etc.).
    pub syntax_type: ControlledOption<Handle<InternedString>>,
    /// The full content of the line containing this node in its source file.
    pub containing_line: ControlledOption<Handle<InternedString>>,
    /// The location in its containing file of the source code that this node's definiens represents.
    /// This is used for things like the bodies of functions, rather than the RHSes of equations.
    /// If you need one of these to make the type checker happy, but you don't have one, just use
    /// lsp_positions::Span::default(), as this will correspond to the all-0s spans which mean "no definiens".
    pub definiens_span: lsp_positions::Span,
    /// The fully qualified name is a representation of the symbol that captures its name and its
    /// embedded context (e.g. `foo.bar` for the symbol `bar` defined in the module `foo`).
    pub fully_qualified_name: ControlledOption<Handle<InternedString>>,
}

impl StackGraph {
    /// Returns information about the source code that a stack graph node represents.
    ///
    /// The graph stores source information in a compact form. This function builds the value.
    /// To read the syntax type only, use [`syntax_type`][StackGraph::syntax_type].
    pub fn source_info(&self, node: Handle<Node>) -> Option<SourceInfo> {
        self.source_info.get(node)
    }

    /// Returns the syntax type of a node, if the node has source information and a syntax type.
    #[inline]
    pub fn syntax_type(&self, node: Handle<Node>) -> Option<Handle<InternedString>> {
        self.source_info.syntax_type(node)
    }

    /// Returns a mutable reference to the information about the source code that a stack graph
    /// node represents. The graph stores the changes when the reference goes out of scope.
    pub fn source_info_mut(&mut self, node: Handle<Node>) -> SourceInfoMut<'_> {
        let info = self.source_info.get_for_update(node);
        SourceInfoMut {
            table: &mut self.source_info,
            node,
            info,
        }
    }
}

/// A mutable reference to the source information of a node. It dereferences to a
/// [`SourceInfo`][]. The graph stores the new value when this reference goes out of scope.
pub struct SourceInfoMut<'a> {
    table: &'a mut SourceInfoTable,
    node: Handle<Node>,
    info: SourceInfo,
}

impl std::ops::Deref for SourceInfoMut<'_> {
    type Target = SourceInfo;
    fn deref(&self) -> &SourceInfo {
        &self.info
    }
}

impl std::ops::DerefMut for SourceInfoMut<'_> {
    fn deref_mut(&mut self) -> &mut SourceInfo {
        &mut self.info
    }
}

impl Drop for SourceInfoMut<'_> {
    fn drop(&mut self) {
        self.table.set(self.node, std::mem::take(&mut self.info));
    }
}

/// One line of a source file: its number and its byte ranges, with and without whitespace.
/// Many positions share a line, so the table stores each line once.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
struct PackedLine {
    line: u32,
    containing_line: [u32; 2],
    trimmed_line: [u32; 2],
}

/// A position: the index of its line, and its three column offsets.
#[derive(Clone, Copy, Default)]
struct PackedPosition {
    line: u32,
    column: [u32; 3],
}

#[derive(Clone, Copy, Default)]
struct PackedSpan {
    start: PackedPosition,
    end: PackedPosition,
}

/// The source information of one node, in 48 bytes.
#[derive(Clone, Copy, Default)]
struct PackedInfo {
    span: PackedSpan,
    syntax_type: ControlledOption<Handle<InternedString>>,
    containing_line: ControlledOption<Handle<InternedString>>,
    fully_qualified_name: ControlledOption<Handle<InternedString>>,
    /// 0 when the definiens span is the default span, else the index in `definiens` plus 1.
    definiens: u32,
}

/// The bit of a slot that marks an entry of `wide`.
const WIDE_SLOT: u32 = 1 << 31;

/// Stores the source information of the nodes of a graph.
///
/// A `SourceInfo` takes 272 bytes, and most nodes have one. This table stores each one in
/// 48 bytes: the values fit in `u32`, the lines are shared, and the definiens spans are rare.
/// A value that does not fit goes in `wide`, as a `SourceInfo`.
///
/// A node with a handle below `slots.len()` and without an entry reads as a default
/// `SourceInfo`. This keeps the behavior of the former dense arena: `get` returns `Some` for
/// every handle up to the highest handle that has had a mutable access.
pub(crate) struct SourceInfoTable {
    /// For each node handle: 0 when the node has no entry, else the index in `entries` plus 1,
    /// or the index in `wide` plus 1 with `WIDE_SLOT` set.
    slots: Vec<u32>,
    entries: Vec<PackedInfo>,
    wide: Vec<SourceInfo>,
    definiens: Vec<PackedSpan>,
    /// The lines of the positions. Index 0 is the default line.
    lines: Vec<PackedLine>,
    line_indexes: FxHashMap<PackedLine, u32>,
}

impl SourceInfoTable {
    fn new() -> SourceInfoTable {
        let mut line_indexes = FxHashMap::default();
        line_indexes.insert(PackedLine::default(), 0);
        SourceInfoTable {
            // Slot 0 stands for the missing node and is never used.
            slots: vec![0],
            entries: Vec::new(),
            wide: Vec::new(),
            definiens: Vec::new(),
            lines: vec![PackedLine::default()],
            line_indexes,
        }
    }

    fn unpack_position(&self, position: &PackedPosition) -> lsp_positions::Position {
        let line = &self.lines[position.line as usize];
        lsp_positions::Position {
            line: line.line as usize,
            column: lsp_positions::Offset {
                utf8_offset: position.column[0] as usize,
                utf16_offset: position.column[1] as usize,
                grapheme_offset: position.column[2] as usize,
            },
            containing_line: line.containing_line[0] as usize..line.containing_line[1] as usize,
            trimmed_line: line.trimmed_line[0] as usize..line.trimmed_line[1] as usize,
        }
    }

    fn unpack_span(&self, span: &PackedSpan) -> lsp_positions::Span {
        lsp_positions::Span {
            start: self.unpack_position(&span.start),
            end: self.unpack_position(&span.end),
        }
    }

    fn unpack(&self, packed: &PackedInfo) -> SourceInfo {
        SourceInfo {
            span: self.unpack_span(&packed.span),
            syntax_type: packed.syntax_type,
            containing_line: packed.containing_line,
            definiens_span: match packed.definiens {
                0 => lsp_positions::Span::default(),
                index => self.unpack_span(&self.definiens[index as usize - 1]),
            },
            fully_qualified_name: packed.fully_qualified_name,
        }
    }

    /// Returns the index of a line. Returns `None` when a value does not fit in `u32`.
    fn pack_line(&mut self, position: &lsp_positions::Position) -> Option<u32> {
        let line = PackedLine {
            line: u32::try_from(position.line).ok()?,
            containing_line: [
                u32::try_from(position.containing_line.start).ok()?,
                u32::try_from(position.containing_line.end).ok()?,
            ],
            trimmed_line: [
                u32::try_from(position.trimmed_line.start).ok()?,
                u32::try_from(position.trimmed_line.end).ok()?,
            ],
        };
        if let Some(index) = self.line_indexes.get(&line) {
            return Some(*index);
        }
        let index = u32::try_from(self.lines.len()).ok()?;
        self.lines.push(line);
        self.line_indexes.insert(line, index);
        Some(index)
    }

    fn pack_position(&mut self, position: &lsp_positions::Position) -> Option<PackedPosition> {
        Some(PackedPosition {
            line: self.pack_line(position)?,
            column: [
                u32::try_from(position.column.utf8_offset).ok()?,
                u32::try_from(position.column.utf16_offset).ok()?,
                u32::try_from(position.column.grapheme_offset).ok()?,
            ],
        })
    }

    fn pack_span(&mut self, span: &lsp_positions::Span) -> Option<PackedSpan> {
        Some(PackedSpan {
            start: self.pack_position(&span.start)?,
            end: self.pack_position(&span.end)?,
        })
    }

    /// Packs a source information. Returns `None` when a value does not fit in `u32`. The
    /// `definiens` argument is the index of the definiens span to reuse, or 0.
    fn pack(&mut self, info: &SourceInfo, definiens: u32) -> Option<PackedInfo> {
        let span = self.pack_span(&info.span)?;
        let definiens_span = if info.definiens_span == lsp_positions::Span::default() {
            None
        } else {
            Some(self.pack_span(&info.definiens_span)?)
        };
        let definiens = match definiens_span {
            None => 0,
            Some(span) if definiens != 0 => {
                self.definiens[definiens as usize - 1] = span;
                definiens
            }
            Some(span) => {
                self.definiens.push(span);
                u32::try_from(self.definiens.len()).ok()?
            }
        };
        Some(PackedInfo {
            span,
            syntax_type: info.syntax_type,
            containing_line: info.containing_line,
            fully_qualified_name: info.fully_qualified_name,
            definiens,
        })
    }

    fn from_slot(&self, slot: u32) -> SourceInfo {
        if slot == 0 {
            SourceInfo::default()
        } else if slot & WIDE_SLOT != 0 {
            self.wide[(slot & !WIDE_SLOT) as usize - 1].clone()
        } else {
            self.unpack(&self.entries[slot as usize - 1])
        }
    }

    #[inline]
    fn get(&self, handle: Handle<Node>) -> Option<SourceInfo> {
        let slot = *self.slots.get(handle.as_usize())?;
        Some(self.from_slot(slot))
    }

    #[inline]
    fn syntax_type(&self, handle: Handle<Node>) -> Option<Handle<InternedString>> {
        let slot = *self.slots.get(handle.as_usize())?;
        if slot == 0 {
            None
        } else if slot & WIDE_SLOT != 0 {
            self.wide[(slot & !WIDE_SLOT) as usize - 1]
                .syntax_type
                .into_option()
        } else {
            self.entries[slot as usize - 1].syntax_type.into_option()
        }
    }

    /// Returns the value of a node to change, and makes the node have a slot.
    fn get_for_update(&mut self, handle: Handle<Node>) -> SourceInfo {
        let index = handle.as_usize();
        if self.slots.len() <= index {
            self.slots.resize(index + 1, 0);
        }
        self.get(handle).unwrap_or_default()
    }

    fn set(&mut self, handle: Handle<Node>, info: SourceInfo) {
        let index = handle.as_usize();
        let slot = self.slots[index];
        // The definiens span of the entry that the node has now, to reuse its place.
        let reuse = if slot == 0 || slot & WIDE_SLOT != 0 {
            0
        } else {
            self.entries[slot as usize - 1].definiens
        };
        match self.pack(&info, reuse) {
            Some(packed) => {
                if slot == 0 || slot & WIDE_SLOT != 0 {
                    self.entries.push(packed);
                    self.slots[index] = self.entries.len() as u32;
                } else {
                    self.entries[slot as usize - 1] = packed;
                }
            }
            None => {
                if slot & WIDE_SLOT != 0 {
                    self.wide[(slot & !WIDE_SLOT) as usize - 1] = info;
                } else {
                    self.wide.push(info);
                    self.slots[index] = self.wide.len() as u32 | WIDE_SLOT;
                }
            }
        }
    }

    /// Returns the value of `get` for each handle from 0, as the former dense arena stored them.
    pub(crate) fn to_dense(&self) -> Vec<SourceInfo> {
        self.slots.iter().map(|&slot| self.from_slot(slot)).collect()
    }
}

//-------------------------------------------------------------------------------------------------
// Debug info

/// Contains debug info about a stack graph node as key-value pairs of strings.
#[derive(Default)]
pub struct DebugInfo {
    entries: Vec<DebugEntry>,
}

impl DebugInfo {
    pub fn add(&mut self, key: Handle<InternedString>, value: Handle<InternedString>) {
        self.entries.push(DebugEntry { key, value });
    }

    pub fn iter(&self) -> std::slice::Iter<DebugEntry> {
        self.entries.iter()
    }
}

/// A debug entry consisting of a string key-value air of strings.
pub struct DebugEntry {
    pub key: Handle<InternedString>,
    pub value: Handle<InternedString>,
}

impl StackGraph {
    /// Returns debug information about the stack graph node.
    pub fn node_debug_info(&self, node: Handle<Node>) -> Option<&DebugInfo> {
        self.node_debug_info.get(node)
    }

    /// Returns a mutable reference to the debug info about the stack graph node.
    pub fn node_debug_info_mut(&mut self, node: Handle<Node>) -> &mut DebugInfo {
        &mut self.node_debug_info[node]
    }

    /// Returns debug information about the stack graph edge.
    pub fn edge_debug_info(&self, source: Handle<Node>, sink: Handle<Node>) -> Option<&DebugInfo> {
        self.edge_debug_info.get(source).and_then(|es| {
            match es.binary_search_by_key(&sink, |e| e.0) {
                Ok(idx) => Some(&es[idx].1),
                Err(_) => None,
            }
        })
    }

    /// Returns a mutable reference to the debug info about the stack graph edge.
    pub fn edge_debug_info_mut(
        &mut self,
        source: Handle<Node>,
        sink: Handle<Node>,
    ) -> &mut DebugInfo {
        let es = &mut self.edge_debug_info[source];
        let idx = match es.binary_search_by_key(&sink, |e| e.0) {
            Ok(idx) => idx,
            Err(idx) => {
                es.insert(idx, (sink, DebugInfo::default()));
                idx
            }
        };
        &mut es[idx].1
    }
}

//-------------------------------------------------------------------------------------------------
// Stack graphs

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Degree {
    Zero,
    One,
    Multiple,
}

impl Default for Degree {
    fn default() -> Self {
        Self::Zero
    }
}

impl std::ops::Add for Degree {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Zero, result) | (result, Self::Zero) => result,
            _ => Self::Multiple,
        }
    }
}

impl std::ops::AddAssign for Degree {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

/// Contains all of the nodes and edges that make up a stack graph.
pub struct StackGraph {
    interned_strings: InternedStringArena,
    pub(crate) symbols: Arena<Symbol>,
    symbol_handles: FxHashMap<&'static str, Handle<Symbol>>,
    pub(crate) strings: Arena<InternedString>,
    string_handles: FxHashMap<&'static str, Handle<InternedString>>,
    pub(crate) files: Arena<File>,
    file_handles: FxHashMap<&'static str, Handle<File>>,
    pub(crate) nodes: Arena<Node>,
    pub(crate) source_info: SourceInfoTable,
    node_id_handles: NodeIDHandles,
    outgoing_edges: SupplementalArena<Node, SmallVec<[OutgoingEdge; 4]>>,
    incoming_edges: SupplementalArena<Node, Degree>,
    pub(crate) node_debug_info: SupplementalArena<Node, DebugInfo>,
    pub(crate) edge_debug_info: SupplementalArena<Node, SmallVec<[(Handle<Node>, DebugInfo); 4]>>,
}

impl StackGraph {
    /// Creates a new, initially empty stack graph.
    pub fn new() -> StackGraph {
        StackGraph::default()
    }

    /// Copies the given stack graph into this stack graph. Panics if any of the files
    /// in the other stack graph are already defined in the current one.
    pub fn add_from_graph(
        &mut self,
        other: &StackGraph,
    ) -> Result<Vec<Handle<File>>, Handle<File>> {
        let mut files = HashMap::new();
        for other_file in other.iter_files() {
            let file = self.add_file(other[other_file].name())?;
            files.insert(other_file, file);
        }
        let files = files;
        let node_id = |other_node_id: NodeID| {
            if other_node_id.is_root() {
                NodeID::root()
            } else if other_node_id.is_jump_to() {
                NodeID::jump_to()
            } else {
                NodeID::new_in_file(
                    files[&other_node_id.file.into_option().unwrap()],
                    other_node_id.local_id,
                )
            }
        };
        let mut nodes = HashMap::new();
        nodes.insert(Self::root_node(), Self::root_node());
        nodes.insert(Self::jump_to_node(), Self::jump_to_node());
        for other_file in files.keys().cloned() {
            let file = files[&other_file];
            for other_node in other.nodes_for_file(other_file) {
                let value: Node = match other[other_node] {
                    Node::DropScopes(DropScopesNode { id, .. }) => DropScopesNode {
                        id: NodeID::new_in_file(file, id.local_id),
                        _symbol: ControlledOption::default(),
                        _scope: NodeID::default(),
                        _is_endpoint: bool::default(),
                    }
                    .into(),
                    Node::JumpTo(JumpToNode { .. }) => JumpToNode {
                        id: NodeID::jump_to(),
                        _symbol: ControlledOption::default(),
                        _scope: NodeID::default(),
                        _is_endpoint: bool::default(),
                    }
                    .into(),
                    Node::PopScopedSymbol(PopScopedSymbolNode {
                        id,
                        symbol,
                        is_definition,
                        ..
                    }) => PopScopedSymbolNode {
                        id: NodeID::new_in_file(file, id.local_id),
                        symbol: self.add_symbol(&other[symbol]),
                        _scope: NodeID::default(),
                        is_definition: is_definition,
                    }
                    .into(),
                    Node::PopSymbol(PopSymbolNode {
                        id,
                        symbol,
                        is_definition,
                        ..
                    }) => PopSymbolNode {
                        id: NodeID::new_in_file(file, id.local_id),
                        symbol: self.add_symbol(&other[symbol]),
                        _scope: NodeID::default(),
                        is_definition: is_definition,
                    }
                    .into(),
                    Node::PushScopedSymbol(PushScopedSymbolNode {
                        id,
                        symbol,
                        scope,
                        is_reference,
                        ..
                    }) => PushScopedSymbolNode {
                        id: NodeID::new_in_file(file, id.local_id),
                        symbol: self.add_symbol(&other[symbol]),
                        scope: node_id(scope),
                        is_reference: is_reference,
                        _phantom: (),
                    }
                    .into(),
                    Node::PushSymbol(PushSymbolNode {
                        id,
                        symbol,
                        is_reference,
                        ..
                    }) => PushSymbolNode {
                        id: NodeID::new_in_file(file, id.local_id),
                        symbol: self.add_symbol(&other[symbol]),
                        _scope: NodeID::default(),
                        is_reference: is_reference,
                    }
                    .into(),
                    Node::Root(RootNode { .. }) => RootNode {
                        id: NodeID::root(),
                        _symbol: ControlledOption::default(),
                        _scope: NodeID::default(),
                        _is_endpoint: bool::default(),
                    }
                    .into(),
                    Node::Scope(ScopeNode {
                        id, is_exported, ..
                    }) => ScopeNode {
                        id: NodeID::new_in_file(file, id.local_id),
                        _symbol: ControlledOption::default(),
                        _scope: NodeID::default(),
                        is_exported: is_exported,
                    }
                    .into(),
                };
                let node = self.add_node(value.id(), value).unwrap();
                nodes.insert(other_node, node);
                if let Some(source_info) = other.source_info(other_node) {
                    *self.source_info_mut(node) = SourceInfo {
                        span: source_info.span.clone(),
                        syntax_type: source_info
                            .syntax_type
                            .into_option()
                            .map(|st| self.add_string(&other[st]))
                            .into(),
                        containing_line: source_info
                            .containing_line
                            .into_option()
                            .map(|cl| self.add_string(&other[cl]))
                            .into(),
                        definiens_span: source_info.definiens_span.clone(),
                        fully_qualified_name: ControlledOption::default(),
                    };
                }
                if let Some(debug_info) = other.node_debug_info(other_node) {
                    *self.node_debug_info_mut(node) = DebugInfo {
                        entries: debug_info
                            .entries
                            .iter()
                            .map(|e| DebugEntry {
                                key: self.add_string(&other[e.key]),
                                value: self.add_string(&other[e.value]),
                            })
                            .collect::<Vec<_>>(),
                    };
                }
            }
        }
        // Add the edges after the nodes of all files. An edge can join nodes of two files, for
        // example an edge from the root node.
        for other_node in nodes.keys().cloned() {
            for other_edge in other.outgoing_edges(other_node) {
                self.add_edge(
                    nodes[&other_edge.source],
                    nodes[&other_edge.sink],
                    other_edge.precedence,
                );
            }
        }
        Ok(files.into_values().collect())
    }
}

impl Default for StackGraph {
    fn default() -> StackGraph {
        let mut nodes = Arena::new();
        nodes.add(RootNode::new().into());
        nodes.add(JumpToNode::new().into());

        StackGraph {
            interned_strings: InternedStringArena::new(),
            symbols: Arena::new(),
            symbol_handles: FxHashMap::default(),
            strings: Arena::new(),
            string_handles: FxHashMap::default(),
            files: Arena::new(),
            file_handles: FxHashMap::default(),
            nodes,
            source_info: SourceInfoTable::new(),
            node_id_handles: NodeIDHandles::new(),
            outgoing_edges: SupplementalArena::new(),
            incoming_edges: SupplementalArena::new(),
            node_debug_info: SupplementalArena::new(),
            edge_debug_info: SupplementalArena::new(),
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Simplification of the graph of one file: prune dead-end scope nodes, contract plain scope nodes.

/// Counts of one simplification of the graph of a file.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContractionStats {
    /// Dead-end scope nodes that the pruning cut off from the routes of the graph.
    pub pruned_nodes: usize,
    /// Edges that the pruning removed.
    pub pruned_edges: usize,
    /// Plain scope nodes that the contraction removed from the routes of the graph.
    pub removable_nodes: usize,
    /// Edges that the contraction redirected past a removed node.
    pub redirected_edges: usize,
    /// Redirections that the contraction skipped because the source already had an edge to the
    /// target.
    pub skipped_duplicates: usize,
}

impl StackGraph {
    /// Simplifies the graph of one file before the file phase: prunes the dead-end scope nodes,
    /// then contracts the chains of plain scope nodes (mode 1). The tree-sitter-stack-graphs
    /// builder calls this at the end of each build, unless the caller turns it off.
    pub fn simplify_file(&mut self, file: Handle<File>) -> ContractionStats {
        let (pruned_nodes, pruned_edges) = self.prune_dead_end_scopes_in_file(file);
        let mut stats = self.contract_plain_scopes_in_file(file, 1);
        stats.pruned_nodes = pruned_nodes;
        stats.pruned_edges = pruned_edges;
        stats
    }

    /// Prunes the dead-end scope nodes of one file. A dead-end scope node is a scope node that is
    /// not exported and has no outgoing edge, or only edges to dead-end scope nodes. No path can
    /// end in such a node, because it is not an endpoint, and no path can leave it. A path
    /// that enters it is never complete. The pruning removes the edges that enter a dead-end node.
    /// This is safe with the arena: no node is deleted, so no handle changes. The dead-end nodes
    /// stay in the arena, but no edge leads to them or leaves them. The pruning does not change the
    /// incoming degree of a node that stays: an edge that it removes ends in a dead-end node, and
    /// a dead-end node has edges to dead-end nodes only. The incoming degree of a dead-end node
    /// becomes zero, as it is in a graph that is stored and loaded again. The one exception is a
    /// dead-end node that has an edge from another file.
    ///
    /// Returns the number of dead-end nodes and the number of removed edges.
    pub fn prune_dead_end_scopes_in_file(&mut self, file: Handle<File>) -> (usize, usize) {
        let nodes: Vec<Handle<Node>> = self.nodes_for_file(file).collect();
        // The scope nodes that can become dead: not exported, with a count of live outgoing edges.
        let mut live_out: FxHashMap<Handle<Node>, usize> = FxHashMap::default();
        for &n in &nodes {
            match &self[n] {
                Node::Scope(s) if !s.is_exported => {}
                _ => continue,
            }
            let count = self.outgoing_edges.get(n).map_or(0, |v| v.len());
            live_out.insert(n, count);
        }
        let mut dead: Vec<Handle<Node>> = live_out
            .iter()
            .filter(|(_, &count)| count == 0)
            .map(|(&n, _)| n)
            .collect();
        if dead.is_empty() {
            return (0, 0);
        }
        // The sources of the edges of the file, by sink.
        let mut sources: FxHashMap<Handle<Node>, SmallVec<[Handle<Node>; 2]>> =
            FxHashMap::default();
        for &x in &nodes {
            for o in self.outgoing_edges.get(x).into_iter().flatten() {
                sources.entry(o.sink).or_default().push(x);
            }
        }
        let mut is_dead: FxHashMap<Handle<Node>, ()> = dead.iter().map(|&n| (n, ())).collect();
        let mut next = 0;
        while next < dead.len() {
            let d = dead[next];
            next += 1;
            for &p in sources.get(&d).into_iter().flatten() {
                if is_dead.contains_key(&p) {
                    continue;
                }
                if let Some(count) = live_out.get_mut(&p) {
                    *count -= 1;
                    if *count == 0 {
                        is_dead.insert(p, ());
                        dead.push(p);
                    }
                }
            }
        }
        let mut removed = 0usize;
        for &x in &nodes {
            let touches = self
                .outgoing_edges
                .get(x)
                .map_or(false, |v| v.iter().any(|o| is_dead.contains_key(&o.sink)));
            if !touches {
                continue;
            }
            let list = &mut self.outgoing_edges[x];
            let before = list.len();
            list.retain(|o| !is_dead.contains_key(&o.sink));
            removed += before - list.len();
        }
        // A dead-end node has lost all its incoming edges from this file. Its degree must say so,
        // or a graph that is stored and loaded again would differ from this one. A node that
        // keeps an edge from another file stays as it is.
        for &d in &dead {
            let in_file = sources.get(&d).map_or(0, |v| v.len());
            let degree = self.incoming_edge_degree(d);
            let all_in_file = match degree {
                Degree::Zero => false,
                Degree::One => in_file == 1,
                Degree::Multiple => in_file >= 2,
            };
            if all_in_file {
                self.incoming_edges[d] = Degree::Zero;
            }
        }
        (dead.len(), removed)
    }

    /// Contracts the plain scope nodes of one file.  A plain scope node is a scope node that is
    /// not exported.  It neither pushes nor pops, and it cannot be a definition or reference.
    ///
    /// `mode` 1 removes plain scope nodes with one incoming and one outgoing edge.  Mode 2
    /// removes plain scope nodes with one outgoing edge, whatever their incoming edges.
    ///
    /// An edge `X -> S` of precedence `p`, where `S` has the single edge `S -> Z`, becomes
    /// `X -> Z` of precedence `p`.  The precedence of `S -> Z` has no effect: no other edge of
    /// `S` can shadow it.  The method removes the edge `S -> Z` when no edge enters `S` any more.
    /// The node stays in the arena, with no edge and with the incoming degree zero.  So the graph
    /// is equal to the graph that a storage load gives back.  A node with an edge from another file
    /// (mode 2) keeps its edge.  A redirection that would duplicate an existing edge of `X` is
    /// skipped, so that no two routes merge into one.  Then `S` keeps its edge.
    pub fn contract_plain_scopes_in_file(
        &mut self,
        file: Handle<File>,
        mode: u8,
    ) -> ContractionStats {
        let nodes: Vec<Handle<Node>> = self.nodes_for_file(file).collect();
        let mut counted_in: FxHashMap<Handle<Node>, u32> = FxHashMap::default();
        for &x in &nodes {
            for o in self.outgoing_edges.get(x).into_iter().flatten() {
                *counted_in.entry(o.sink).or_insert(0) += 1;
            }
        }
        // For each removable node: its single sink, and whether its in-edges are exact.
        let mut removable: FxHashMap<Handle<Node>, (Handle<Node>, bool)> = FxHashMap::default();
        for &n in &nodes {
            let exported = match &self[n] {
                Node::Scope(s) => s.is_exported,
                _ => continue,
            };
            if exported {
                continue;
            }
            let outs: &[OutgoingEdge] = self.outgoing_edges.get(n).map_or(&[][..], |v| &v[..]);
            if outs.len() != 1 || outs[0].sink == n {
                continue;
            }
            let exact = self.incoming_edge_degree(n) == Degree::One
                && counted_in.get(&n).copied().unwrap_or(0) == 1;
            if mode == 1 && !exact {
                continue;
            }
            removable.insert(n, (outs[0].sink, exact));
        }
        let mut stats = ContractionStats::default();
        let mut inexact_targets: Vec<Handle<Node>> = Vec::new();
        for &x in &nodes {
            if removable.contains_key(&x) {
                continue;
            }
            let mut list: SmallVec<[OutgoingEdge; 4]> = SmallVec::new();
            for o in self.outgoing_edges.get(x).into_iter().flatten() {
                list.push(OutgoingEdge {
                    sink: o.sink,
                    precedence: o.precedence,
                });
            }
            let mut changed = false;
            for i in 0..list.len() {
                let first = list[i].sink;
                if !removable.contains_key(&first) {
                    continue;
                }
                // Follow the chain to the first node that stays.
                let mut z = first;
                let mut steps = 0usize;
                let mut exact = true;
                while let Some(&(next, e)) = removable.get(&z) {
                    exact &= e;
                    z = next;
                    steps += 1;
                    if steps > removable.len() {
                        break;
                    }
                }
                if steps > removable.len() {
                    continue;
                }
                if list.iter().any(|o| o.sink == z) {
                    stats.skipped_duplicates += 1;
                    continue;
                }
                list[i].sink = z;
                changed = true;
                stats.redirected_edges += 1;
                if !exact {
                    inexact_targets.push(z);
                }
            }
            if changed {
                list.sort_by_key(|o| o.sink);
                self.outgoing_edges[x] = list;
            }
        }
        for z in inexact_targets {
            if self.incoming_edge_degree(z) == Degree::One {
                self.incoming_edges[z] = Degree::Multiple;
            }
        }
        // Find the removable nodes that an edge still reaches: the ones that a kept node still
        // names (a skipped redirection), the ones that have edges from other files, and the
        // removable nodes behind them.
        let mut live: FxHashMap<Handle<Node>, ()> = FxHashMap::default();
        let mut work: Vec<Handle<Node>> = Vec::new();
        for &x in &nodes {
            if removable.contains_key(&x) {
                continue;
            }
            for o in self.outgoing_edges.get(x).into_iter().flatten() {
                if removable.contains_key(&o.sink) && live.insert(o.sink, ()).is_none() {
                    work.push(o.sink);
                }
            }
        }
        for (&n, &(_, exact)) in &removable {
            if !exact && live.insert(n, ()).is_none() {
                work.push(n);
            }
        }
        while let Some(n) = work.pop() {
            let next = removable[&n].0;
            if removable.contains_key(&next) && live.insert(next, ()).is_none() {
                work.push(next);
            }
        }
        // The other removable nodes are bypassed. The edge that left such a node now starts at
        // the node that was before it, so the degree of its sink does not change.
        for &n in removable.keys() {
            if !live.contains_key(&n) {
                self.outgoing_edges[n].clear();
                self.incoming_edges[n] = Degree::Zero;
            }
        }
        stats.removable_nodes = removable.len();
        stats
    }
}
