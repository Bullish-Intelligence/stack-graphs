// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2023, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

use std::borrow::Cow;
use stack_graphs::graph::StackGraph;
use tree_sitter_stack_graphs::loader::LanguageConfiguration;
use tree_sitter_stack_graphs::loader::LoadError;
use tree_sitter_stack_graphs::BuildError;
use tree_sitter_stack_graphs::CancellationFlag;
use tree_sitter_stack_graphs::StackGraphLanguage;
use tree_sitter_stack_graphs::Variables;
use tree_sitter_stack_graphs::FILE_PATH_VAR;

/// The stack graphs tsg source for this language.
pub const STACK_GRAPHS_TSG_PATH: &str = "src/stack-graphs.tsg";
/// The stack graphs tsg source for this language.
pub const STACK_GRAPHS_TSG_SOURCE: &str = include_str!("../src/stack-graphs.tsg");

/// The stack graphs builtins configuration for this language.
pub const STACK_GRAPHS_BUILTINS_CONFIG: &str = include_str!("../src/builtins.cfg");
/// The stack graphs builtins path for this language
pub const STACK_GRAPHS_BUILTINS_PATH: &str = "src/builtins.py";
/// The stack graphs builtins source for this language.
pub const STACK_GRAPHS_BUILTINS_SOURCE: &str = include_str!("../src/builtins.py");

/// The root path of the stub modules. Pass it as the `ROOT_PATH` global when you build a stub
/// module. The stack graphs rules remove it from the file name of the stub to get the module name.
pub const STUB_ROOT_PATH: &str = "<stubs>/";

/// A stub module that the language ships, for example the `typing` module of the standard library.
///
/// A stub module is a Python source file that defines the names of a module. `import typing` and
/// `from typing import List` resolve to its definitions, as they resolve to a project file. The
/// language configuration holds the stub modules in its builtins graph. A tool that does not use
/// the builtins graph can build the stub modules itself with [`build_stub_module_into`].
#[derive(Clone, Copy, Debug)]
pub struct StubModule {
    /// The name of the module, as `import` writes it.
    pub module: &'static str,
    /// The path of the source file in this crate. It appears in messages only.
    pub path: &'static str,
    /// The source of the stub module.
    pub source: &'static str,
}

impl StubModule {
    /// The name of the file of the stub module in a stack graph: `STUB_ROOT_PATH`, the module
    /// name, and `.py`. It cannot collide with a file of a project.
    pub fn file_name(&self) -> String {
        format!("{}{}.py", STUB_ROOT_PATH, self.module)
    }
}

/// The stub modules of this language.
pub const STUB_MODULES: &[StubModule] = &[StubModule {
    module: "typing",
    path: "src/stubs/typing.py",
    source: include_str!("../src/stubs/typing.py"),
}];

/// Build one stub module into `graph`. Return the file of the stub module.
pub fn build_stub_module_into(
    sgl: &StackGraphLanguage,
    graph: &mut StackGraph,
    stub: &StubModule,
    cancellation_flag: &dyn CancellationFlag,
) -> Result<stack_graphs::arena::Handle<stack_graphs::graph::File>, BuildError> {
    let name = stub.file_name();
    let file = graph.get_or_create_file(&name);
    let mut globals = Variables::new();
    globals
        .add(FILE_PATH_VAR.into(), name.as_str().into())
        .expect("set FILE_PATH");
    globals
        .add("ROOT_PATH".into(), STUB_ROOT_PATH.into())
        .expect("set ROOT_PATH");
    sgl.build_stack_graph_into(graph, file, stub.source, &globals, cancellation_flag)?;
    Ok(file)
}

pub fn language_configuration(cancellation_flag: &dyn CancellationFlag) -> LanguageConfiguration {
    try_language_configuration(cancellation_flag).unwrap_or_else(|err| panic!("{}", err))
}

pub fn try_language_configuration(
    cancellation_flag: &dyn CancellationFlag,
) -> Result<LanguageConfiguration, LoadError> {
    let mut lc = LanguageConfiguration::from_sources(
        tree_sitter_python::LANGUAGE.into(),
        Some(String::from("source.py")),
        None,
        vec![String::from("py")],
        STACK_GRAPHS_TSG_PATH.into(),
        STACK_GRAPHS_TSG_SOURCE,
        Some((
            STACK_GRAPHS_BUILTINS_PATH.into(),
            STACK_GRAPHS_BUILTINS_SOURCE,
        )),
        Some(STACK_GRAPHS_BUILTINS_CONFIG),
        cancellation_flag,
    )?;
    for stub in STUB_MODULES {
        build_stub_module_into(&lc.sgl, &mut lc.builtins, stub, cancellation_flag).map_err(
            |inner| LoadError::Builtins {
                inner,
                source_path: stub.path.into(),
                source: Cow::from(stub.source),
                tsg_path: lc.sgl.tsg_path().to_path_buf(),
                tsg: Cow::from(STACK_GRAPHS_TSG_SOURCE),
            },
        )?;
    }
    Ok(lc)
}
