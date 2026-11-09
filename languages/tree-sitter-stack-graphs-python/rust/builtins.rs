// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2024, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Tests that names in Python source resolve to definitions in `src/builtins.py`.
//!
//! The `.py` fixtures in `test/` cannot check this. The loader adds the builtins graph to every
//! fixture graph, but a `defined:` assertion can only name lines of the fixture file.
//! This test resolves the references itself and reads the line of each definition in the builtins.

use stack_graphs::graph::StackGraph;
use stack_graphs::partial::PartialPaths;
use stack_graphs::stitching::Database;
use stack_graphs::stitching::DatabaseCandidates;
use stack_graphs::stitching::ForwardPartialPathStitcher;
use stack_graphs::stitching::StitcherConfig;
use tree_sitter_stack_graphs::NoCancellation;
use tree_sitter_stack_graphs::Variables;
use tree_sitter_stack_graphs_python::STACK_GRAPHS_BUILTINS_SOURCE;
use tree_sitter_stack_graphs_python::STUB_MODULES;

const BUILTINS_FILE: &str = "<builtins>";
const TEST_FILE: &str = "test.py";

/// One resolved definition of a reference.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    /// A definition in the builtins. The string is the source line of the definition.
    Builtin(String),
    /// A definition in a stub module. The first string is the name of the stub file. The second
    /// string is the source line of the definition.
    Stub(String, String),
    /// A definition in the test source. The number is the zero-based line.
    Local(usize),
}

/// Resolve all references in `source` and return the targets of the references named `name`.
fn resolve(source: &str, name: &str) -> Vec<Target> {
    let builtin_lines = STACK_GRAPHS_BUILTINS_SOURCE.lines().collect::<Vec<_>>();
    resolve_lines(source, name)
        .into_iter()
        .map(|(file, line)| {
            if file == BUILTINS_FILE {
                Target::Builtin(builtin_lines[line].trim().to_string())
            } else if file == TEST_FILE {
                Target::Local(line)
            } else {
                let text = stub_lines(&file)[line].trim().to_string();
                Target::Stub(file, text)
            }
        })
        .collect()
}

/// The lines of the stub module with the file name `file`.
fn stub_lines(file: &str) -> Vec<&'static str> {
    let stub = STUB_MODULES
        .iter()
        .find(|s| s.file_name() == file)
        .unwrap_or_else(|| panic!("no stub module {}", file));
    stub.source.lines().collect()
}

/// Return the class that contains the builtins line `line`, and the trimmed text of the line.
fn builtin_member(line: usize) -> (String, String) {
    let builtin_lines = STACK_GRAPHS_BUILTINS_SOURCE.lines().collect::<Vec<_>>();
    let owner = builtin_lines[..line]
        .iter()
        .rev()
        .find_map(|l| l.strip_prefix("class "))
        .map(|l| l.split(|c| c == '(' || c == ':').next().unwrap().to_string())
        .unwrap_or_default();
    (owner, builtin_lines[line].trim().to_string())
}

/// Resolve all references in `source` and return the definitions of the references named `name`.
/// A definition is a pair. The first item is the name of the file of the definition.
/// The second item is the zero-based line of the definition.
fn resolve_lines(source: &str, name: &str) -> Vec<(String, usize)> {
    let lc = tree_sitter_stack_graphs_python::language_configuration(&NoCancellation);
    let mut graph = StackGraph::new();
    graph.add_from_graph(&lc.builtins).expect("add builtins");
    let file = graph.get_or_create_file(TEST_FILE);
    let mut globals = Variables::new();
    globals
        .add("FILE_PATH".into(), TEST_FILE.into())
        .expect("set FILE_PATH");
    lc.sgl
        .build_stack_graph_into(&mut graph, file, source, &globals, &NoCancellation)
        .expect("build test source");

    let mut partials = PartialPaths::new();
    let mut db = Database::new();
    let files = graph.iter_files().collect::<Vec<_>>();
    for f in files {
        let mut paths = Vec::new();
        ForwardPartialPathStitcher::find_minimal_partial_path_set_in_file(
            &graph,
            &mut partials,
            f,
            StitcherConfig::default(),
            &stack_graphs::NoCancellation,
            |_, _, p| paths.push(p.clone()),
        )
        .expect("partial paths");
        for p in paths {
            db.add_partial_path(&graph, &mut partials, p);
        }
    }

    let references = graph
        .iter_nodes()
        .filter(|n| graph[*n].is_reference())
        .filter(|n| graph[*n].file() == Some(file))
        .filter(|n| graph[*n].symbol().map(|s| &graph[s] == name).unwrap_or(false))
        .collect::<Vec<_>>();
    assert!(!references.is_empty(), "no reference named {}", name);

    let mut targets = Vec::new();
    for reference in references {
        let mut paths = Vec::new();
        ForwardPartialPathStitcher::find_all_complete_partial_paths(
            &mut DatabaseCandidates::new(&graph, &mut partials, &db),
            std::iter::once(reference),
            StitcherConfig::default(),
            &stack_graphs::NoCancellation,
            |_, _, p| paths.push(p.clone()),
        )
        .expect("resolve");
        // Drop the paths that another path shadows, as the test runner does.
        let ends = paths
            .iter()
            .filter(|p| paths.iter().all(|o| !o.shadows(&mut partials, p)))
            .map(|p| p.end_node)
            .collect::<Vec<_>>();
        for end in ends {
            let line = graph.source_info(end).expect("source info").span.start.line;
            let end_file = graph[end].file().expect("definition file");
            let target = (graph[end_file].name().to_string(), line);
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    targets
}

fn assert_builtin(source: &str, name: &str, definition: &str) {
    let targets = resolve(source, name);
    assert_eq!(
        targets,
        vec![Target::Builtin(definition.to_string())],
        "reference {} in:\n{}",
        name,
        source
    );
}

/// Check that the reference `name` resolves to exactly one definition, a member of `class`.
fn assert_member(source: &str, name: &str, class: &str, definition: &str) {
    let targets = resolve_lines(source, name);
    assert_eq!(targets.len(), 1, "reference {} in:\n{}\n{:?}", name, source, targets);
    assert_eq!(targets[0].0, BUILTINS_FILE, "reference {} is not a builtin in:\n{}", name, source);
    assert_eq!(
        builtin_member(targets[0].1),
        (class.to_string(), definition.to_string()),
        "reference {} in:\n{}",
        name,
        source
    );
}

#[test]
fn builtin_function_len() {
    assert_builtin("x = len([1, 2])\n", "len", "def len(*args, **kwargs): ...");
}

#[test]
fn builtin_function_isinstance() {
    assert_builtin(
        "def f(x):\n    return isinstance(x, int)\n",
        "isinstance",
        "def isinstance(*args, **kwargs): ...",
    );
}

#[test]
fn builtin_class() {
    assert_builtin("x = int('1')\n", "int", "class int:");
    assert_builtin("x = super()\n", "super", "class super:");
}

#[test]
fn builtin_exception_class() {
    assert_builtin(
        "def f():\n    raise ValueError('bad')\n",
        "ValueError",
        "class ValueError(Exception):",
    );
}

#[test]
fn builtin_exception_in_except_clause() {
    assert_builtin(
        "try:\n    pass\nexcept KeyError as e:\n    pass\n",
        "KeyError",
        "class KeyError(LookupError):",
    );
}

#[test]
fn builtin_exception_base_class() {
    // The base class `Exception` resolves, because it is a builtin too.
    assert_builtin(
        "class MyError(Exception):\n    pass\n",
        "Exception",
        "class Exception(BaseException):",
    );
}

#[test]
fn builtin_class_method_through_class() {
    assert_builtin(
        "x = str.join(',', ['a'])\n",
        "join",
        "def join(self, *args, **kwargs): ...",
    );
}

#[test]
fn builtin_method_inherited_from_base_class() {
    // `with_traceback` is defined in `BaseException`, the base of `Exception`.
    assert_builtin(
        "x = Exception().with_traceback\n",
        "with_traceback",
        "def with_traceback(self, *args, **kwargs): ...",
    );
}

#[test]
fn builtin_member_through_user_subclass_of_exception() {
    // `MyError` derives from `Exception`, which derives from `BaseException`.
    let source = "class MyError(Exception):\n    pass\n\nx = MyError().args\ny = MyError().with_traceback\n";
    assert_builtin(source, "args", "args = None");
    assert_builtin(
        source,
        "with_traceback",
        "def with_traceback(self, *args, **kwargs): ...",
    );
}

#[test]
fn builtin_member_through_three_levels() {
    // `KeyError` derives from `LookupError`, `Exception` and `BaseException`.
    let source = "class MyKeyError(KeyError):\n    pass\n\nx = MyKeyError().args\n";
    assert_builtin(source, "args", "args = None");
    assert_builtin("x = KeyError().args\n", "args", "args = None");
}

#[test]
fn builtin_class_member_through_user_subclass() {
    let source = "class MyError(Exception):\n    pass\n\nx = MyError.with_traceback\n";
    assert_builtin(
        source,
        "with_traceback",
        "def with_traceback(self, *args, **kwargs): ...",
    );
}

#[test]
fn builtin_member_through_self_in_subclass() {
    let source =
        "class MyError(Exception):\n    def show(self):\n        return self.args\n";
    assert_builtin(source, "args", "args = None");
}

const ANY_METHOD: &str = "(self, *args, **kwargs): ...";

fn method(name: &str) -> String {
    format!("def {}{}", name, ANY_METHOD)
}

#[test]
fn string_literal_is_a_str() {
    assert_member("x = 'abc'.join\n", "join", "str", &method("join"));
    assert_member("x = ', '.join(['a'])\n", "join", "str", &method("join"));
    assert_member("x = '{}'.format(1)\n", "format", "str", &method("format"));
    assert_member("x = f'a{1}'.upper\n", "upper", "str", &method("upper"));
    assert_member("x = ('a' 'b').lower\n", "lower", "str", &method("lower"));
}

#[test]
fn bytes_literal_is_a_bytes() {
    assert_member("x = b'a'.decode\n", "decode", "bytes", &method("decode"));
    assert_member("x = rb'a'.hex\n", "hex", "bytes", &method("hex"));
    assert_member("x = (b'a' b'b').decode\n", "decode", "bytes", &method("decode"));
}

#[test]
fn number_literals_are_instances() {
    assert_member("x = (1).bit_length\n", "bit_length", "int", &method("bit_length"));
    assert_member("x = 0x1F.bit_length\n", "bit_length", "int", &method("bit_length"));
    assert_member("x = 1.5.is_integer\n", "is_integer", "float", &method("is_integer"));
    assert_member("x = (1j).real\n", "real", "complex", &method("real"));
}

#[test]
fn boolean_literal_is_a_bool_and_reaches_int() {
    // `bool` derives from `int`, so this also checks that members chain through a base class.
    assert_member("x = True.bit_length\n", "bit_length", "int", &method("bit_length"));
    assert_member("x = False.bit_length\n", "bit_length", "int", &method("bit_length"));
}

#[test]
fn container_literals_are_instances() {
    assert_member("[1].append\n", "append", "list", &method("append"));
    assert_member("[x for x in y].sort\n", "sort", "list", &method("sort"));
    assert_member("{}.get\n", "get", "dict", &method("get"));
    assert_member("{'a': 1}.items\n", "items", "dict", &method("items"));
    assert_member("{k: v for k, v in y}.items\n", "items", "dict", &method("items"));
    assert_member("{1}.add\n", "add", "set", &method("add"));
    assert_member("{x for x in y}.union\n", "union", "set", &method("union"));
    assert_member("(1, 2).count\n", "count", "tuple", &method("count"));
    assert_member("x = 1, 2\nx.count\n", "count", "tuple", &method("count"));
}

#[test]
fn variable_of_a_literal_has_members_of_the_class() {
    assert_member("x = 'a'\ny = x.upper\n", "upper", "str", &method("upper"));
    assert_member("x = []\nx.append(1)\n", "append", "list", &method("append"));
}

#[test]
fn reference_to_variable_of_a_literal_resolves_to_the_variable_only() {
    for value in ["'a'", "b'a'", "1", "1.5", "True", "[]", "{}", "{1}", "(1, 2)", "[x for x in y]"] {
        let source = format!("x = {}\nprint(x)\n", value);
        assert_eq!(resolve(&source, "x"), vec![Target::Local(0)], "{}", value);
    }
}

#[test]
fn reference_to_instance_of_a_subclass_does_not_resolve_to_the_base_class() {
    let source = "class B:\n    pass\n\nclass A(B):\n    pass\n\nx = A()\nprint(x)\n";
    assert_eq!(resolve(source, "x"), vec![Target::Local(6)]);
}

#[test]
fn annotated_parameter_has_members_of_the_builtin_class() {
    assert_member("def f(x: int):\n    x.bit_length\n", "bit_length", "int", &method("bit_length"));
    assert_member("def f(s: 'str'):\n    s.upper\n", "upper", "str", &method("upper"));
    assert_member("def f(s: str | None):\n    s.upper\n", "upper", "str", &method("upper"));
    assert_member("def f(s: str = ''):\n    s.upper\n", "upper", "str", &method("upper"));
}

#[test]
fn annotated_generic_resolves_the_container_only() {
    assert_member("def f(a: list[int]):\n    a.append\n", "append", "list", &method("append"));
    // The element type does not reach the name.
    assert!(resolve_lines("def f(a: list[int]):\n    a.bit_length\n", "bit_length").is_empty());
}

#[test]
fn annotated_variable_has_members_of_the_builtin_class() {
    assert_member("x: dict = {}\nx.get\n", "get", "dict", &method("get"));
    assert_member("x: set\nx.add\n", "add", "set", &method("add"));
}

#[test]
fn annotated_exception_reaches_the_base_class() {
    assert_member(
        "def f(e: KeyError):\n    e.with_traceback\n",
        "with_traceback",
        "BaseException",
        &method("with_traceback"),
    );
}

#[test]
fn return_annotation_gives_members_of_the_builtin_class() {
    assert_member(
        "def f() -> str: ...\n\nf().upper\n",
        "upper",
        "str",
        &method("upper"),
    );
}

#[test]
fn reference_to_annotated_variable_resolves_to_the_variable_only() {
    let source = "def f(x: int):\n    return x\n";
    assert_eq!(resolve(source, "x"), vec![Target::Local(0)]);
    let source = "y: int = 1\nprint(y)\n";
    assert_eq!(resolve(source, "y"), vec![Target::Local(0)]);
}

#[test]
fn local_class_named_like_a_literal_class_does_not_capture_literals() {
    // The literal uses the global scope, so a local class `str` does not change it.
    let source = "class str:\n    def upper(self): ...\n\nx = 'a'.join\n";
    assert_member(source, "join", "str", &method("join"));
}

#[test]
fn builtin_module_constant() {
    assert_builtin(
        "if __name__ == '__main__':\n    pass\n",
        "__name__",
        "__name__ = None",
    );
}

#[test]
fn function_definition_shadows_builtin() {
    let targets = resolve("def len(x):\n    pass\n\ny = len(1)\n", "len");
    assert_eq!(targets, vec![Target::Local(0)]);
}

#[test]
fn class_definition_shadows_builtin() {
    let targets = resolve("class dict:\n    pass\n\ny = dict()\n", "dict");
    assert_eq!(targets, vec![Target::Local(0)]);
}

#[test]
fn nested_definition_shadows_builtin() {
    let source = "def f():\n    def len():\n        pass\n    return len\n";
    assert_eq!(resolve(source, "len"), vec![Target::Local(1)]);
}

#[test]
fn parameter_shadows_builtin() {
    let source = "def f(id, *iter, type=1, **min):\n    return id, iter, type, min\n";
    for name in ["id", "iter", "type", "min"] {
        assert_eq!(resolve(source, name), vec![Target::Local(0)], "{}", name);
    }
}

#[test]
fn method_parameter_shadows_builtin() {
    let source = "class A:\n    def m(self, id):\n        return id\n";
    assert_eq!(resolve(source, "id"), vec![Target::Local(1)]);
}

#[test]
fn lambda_parameter_shadows_builtin() {
    let targets = resolve("f = lambda id: id\n", "id");
    assert_eq!(targets, vec![Target::Local(0)]);
}

#[test]
fn local_variable_shadows_builtin() {
    let source = "def f():\n    id = 1\n    return id\n";
    assert_eq!(resolve(source, "id"), vec![Target::Local(1)]);
}

#[test]
fn conditional_binding_keeps_the_builtin() {
    // The assignment may not run, so both the local variable and the builtin can be the target.
    let source = "def f(a):\n    if a:\n        id = 1\n    return id\n";
    let targets = resolve(source, "id");
    assert_eq!(targets.len(), 2, "{:?}", targets);
    assert!(targets.contains(&Target::Local(2)), "{:?}", targets);
}

/// Check that the reference `name` resolves to the definition in the stub module `module`.
/// A name that is an alias of a builtin class, like `List = list`, also resolves to the class.
/// No other definition is allowed.
fn assert_stub(source: &str, name: &str, module: &str, definition: &str) {
    let file = format!("<stubs>/{}.py", module);
    let targets = resolve(source, name);
    assert!(
        targets.contains(&Target::Stub(file, definition.to_string())),
        "reference {} in:\n{}\n{:?}",
        name,
        source,
        targets
    );
    assert!(
        targets.len() == 1 || (targets.len() == 2 && targets.iter().any(|t| matches!(t, Target::Builtin(_)))),
        "reference {} in:\n{}\n{:?}",
        name,
        source,
        targets
    );
}

#[test]
fn from_import_of_a_typing_name_resolves_to_the_stub() {
    let source = "from typing import List, Optional, Any\n";
    assert_stub(source, "List", "typing", "List = list");
    assert_stub(source, "Optional", "typing", "Optional = None");
    assert_stub(source, "Any", "typing", "class Any:");
}

#[test]
fn imported_typing_module_resolves_to_the_stub() {
    let source = "import typing\n\nx = typing.Dict\n";
    assert_stub(source, "Dict", "typing", "Dict = dict");
    // The module name resolves to the stub file, which has no line of its own.
    assert!(!resolve_lines(source, "typing").is_empty());
}

#[test]
fn aliased_import_of_a_typing_name_resolves_to_the_stub() {
    let source = "from typing import Sequence as Seq\n\nx: Seq\n";
    assert_stub(source, "Sequence", "typing", "class Sequence(Reversible, Collection):");
}

#[test]
fn unknown_typing_name_does_not_resolve() {
    assert!(resolve_lines("from typing import Nonexistent\n", "Nonexistent").is_empty());
}

#[test]
fn typing_aliases_have_members_of_the_builtin_class() {
    for (annotation, name, class) in [
        ("List[int]", "append", "list"),
        ("Dict[str, int]", "items", "dict"),
        ("Set[int]", "add", "set"),
        ("FrozenSet[int]", "union", "frozenset"),
        ("Tuple[int, str]", "count", "tuple"),
        ("Type[object]", "mro", "type"),
    ] {
        let head = annotation.split('[').next().unwrap();
        let source = format!(
            "from typing import {}\n\ndef f(a: {}):\n    a.{}\n",
            head, annotation, name
        );
        assert_member(&source, name, class, &method(name));
    }
}

#[test]
fn typing_alias_in_an_optional_annotation_has_members_of_the_builtin_class() {
    let source = "from typing import List, Optional\n\ndef f(a: Optional[List[int]]):\n    a.append\n";
    // `Optional` follows the first type argument, which is the container `List`.
    assert_member(source, "append", "list", &method("append"));
}

#[test]
fn typing_alias_through_the_module_has_members_of_the_builtin_class() {
    let source = "import typing\n\ndef f(a: typing.List[int]):\n    a.append\n";
    assert_member(source, "append", "list", &method("append"));
}

#[test]
fn typing_alias_for_a_variable_has_members_of_the_builtin_class() {
    let source = "from typing import Dict\n\nx: Dict[str, int] = {}\nx.get\n";
    assert_member(source, "get", "dict", &method("get"));
}

#[test]
fn typing_class_has_its_methods() {
    let source = "from typing import Mapping\n\ndef f(m: Mapping):\n    m.keys\n";
    let targets = resolve_lines(source, "keys");
    assert_eq!(targets.len(), 1, "{:?}", targets);
    assert_eq!(targets[0].0, "<stubs>/typing.py");
}

#[test]
fn local_name_shadows_a_typing_import() {
    let source = "from typing import List\n\nclass List:\n    pass\n\nx = List\n";
    // The last reference `List` is in `x = List`.
    let targets = resolve(source, "List");
    assert!(targets.contains(&Target::Local(2)), "{:?}", targets);
}

#[test]
fn definition_after_a_function_shadows_the_builtin() {
    let source = "def f():\n    return len(1)\n\ndef len(x):\n    pass\n";
    assert_eq!(resolve(source, "len"), vec![Target::Local(3)]);
}

#[test]
fn variable_after_a_function_shadows_the_builtin() {
    let source = "def f():\n    return id\n\nid = 5\n";
    assert_eq!(resolve(source, "id"), vec![Target::Local(3)]);
}

#[test]
fn parameter_shadows_a_definition_after_the_function() {
    let source = "def f(len):\n    return len\n\ndef len(x):\n    pass\n";
    assert_eq!(resolve(source, "len"), vec![Target::Local(0)]);
}

#[test]
fn builtin_stays_visible_when_the_module_does_not_define_the_name() {
    let source = "def f():\n    return len(1)\n";
    assert_eq!(
        resolve(source, "len"),
        vec![Target::Builtin("def len(*args, **kwargs): ...".to_string())]
    );
}

#[test]
fn nested_function_keeps_the_binding_of_the_enclosing_function() {
    let source = "y = 1\ndef outer():\n    y = 2\n    def inner():\n        return y\n";
    let targets = resolve(source, "y");
    assert!(targets.contains(&Target::Local(2)), "{:?}", targets);
}

#[test]
fn annotated_star_args_is_a_tuple() {
    let source = "def f(*args: int):\n    args.count\n";
    assert_member(source, "count", "tuple", &method("count"));
    // The annotation does not apply to the tuple.
    assert!(resolve_lines("def f(*args: int):\n    args.bit_length\n", "bit_length").is_empty());
}

#[test]
fn annotated_star_star_kwargs_is_a_dict() {
    let source = "def f(**kwargs: str):\n    kwargs.items\n";
    assert_member(source, "items", "dict", &method("items"));
    assert!(resolve_lines("def f(**kwargs: str):\n    kwargs.upper\n", "upper").is_empty());
}

#[test]
fn annotated_star_args_resolves_to_the_parameter_only() {
    let source = "def f(*args: int, **kwargs: int):\n    return args, kwargs\n";
    assert_eq!(resolve(source, "args"), vec![Target::Local(0)]);
    assert_eq!(resolve(source, "kwargs"), vec![Target::Local(0)]);
}

#[test]
fn property_method_gets_the_class_scope() {
    let source = "class A:\n    @property\n    def p(self):\n        return self.args\n";
    // `A` has no base class, so `args` does not resolve.
    assert!(resolve_lines(source, "args").is_empty());
    let source = "class A(Exception):\n    @property\n    def p(self):\n        return self.args\n";
    assert_builtin(source, "args", "args = None");
}

#[test]
fn classmethod_parameter_behaves_like_the_class() {
    let source = "class A(Exception):\n    @classmethod\n    def make(cls):\n        return cls().args\n";
    assert_builtin(source, "args", "args = None");
    let source = "class A(Exception):\n    @classmethod\n    def make(cls):\n        return cls.with_traceback\n";
    assert_member(source, "with_traceback", "BaseException", &method("with_traceback"));
}

#[test]
fn staticmethod_parameter_has_no_class_scope() {
    let source = "class A(Exception):\n    @staticmethod\n    def make(x):\n        return x.args\n";
    assert!(resolve_lines(source, "args").is_empty());
}

#[test]
fn keyword_argument_resolves_to_the_parameter_through_a_base_class() {
    let source = "class B:\n    def __init__(self, size):\n        pass\n\nclass C(B):\n    pass\n\nC(size=1)\n";
    assert_eq!(resolve(source, "size"), vec![Target::Local(1)]);
}
