# Changelog for tree-sitter-stack-graphs-python

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

- A method joins the member attributes of its class only if its body can store an attribute of its first parameter: `self.x = v`, `self.x += v`, `self.x: T = v`, a tuple target, or a `for` or `as` target. Before this change, a lookup of a member entered every method of the class, such as the 66 methods of `str` in the builtins, and went on in each one. One lookup made as many paths as the class has methods, and many routes into one class hit the queue limit of the stitcher. The results do not change, because a method that stores nothing defines no member. Measured against scip-python: on the standard library (160 files, 3 percent annotated), the references that a limit stops fall from 26 to 10, and the run takes 32 percent less time. On mypy the run takes 38 percent less time. No location goes from resolved to unresolved where scip-python agrees with the old result.

- The stitcher setting `StitcherConfig::with_max_scope_depth` bounds the growth of the call chain. A function that has two call sites at each of N levels gave 2^(N+1) paths. With depth 1, the queue of the same chain peaks at 9 paths for any N. On the standard library, depth 1 lowers the references that a limit stops from 10 to 2, with no other change. The rules do not set it. The caller of the stitcher does.

- A function inside another function sees the bindings of the enclosing function before the final bindings of the module and the builtins. The same holds for a lambda inside a function. Before this change, a name of the enclosing function resolved together with a module or builtin name.

- An `if` statement with an `else` clause no longer lets the bindings before the statement flow around its branches. After the statement, a name reaches the bindings of the branches, and an earlier binding only if a branch passes it through. The language configuration sets `shadow_aware_similar_paths` for this. Every stitcher that resolves Python graphs must set `StitcherConfig::with_shadow_aware_similar_paths(true)`. Without it, a name can lose a binding.

- The body of a lambda sees the bindings of the enclosing function, as the body of a nested function does. A parameter of the lambda shadows them.

- A read of a property, such as `obj.prop.attribute`, behaves like a call of the method. The return annotation or the returned value of the method gives the type of the read. This holds for `property`, `cached_property`, and `abstractproperty`.

- The first parameter of a classmethod, and of `__new__`, `__init_subclass__`, and `__class_getitem__`, behaves like the class. A call of it resolves the keyword arguments to the parameters of `__init__`, and `cls.attribute` resolves in the class.

- An annotation follows a dotted wrapper such as `typing.Optional[Foo]` and `typing.Union[Foo, Bar]`, a union type such as `list[Foo] | None`, and a string argument such as `typing.Optional["Foo"]`.

- A name in an annotation reaches the final bindings of the module. An annotation can name a class that the module defines later, or the class that holds the annotation.

- Identifiers in expressions no longer create a second reference node that has no edges.

- The rules define the loader's `<builtins>` file as the module `<builtins>`. Before this change, no reference reached the builtins file.

- `src/builtins.py` now holds a stub of the Python builtins: functions, classes with their methods, the exception hierarchy, and module constants such as `__name__`. The stub derives from typeshed `stdlib/builtins.pyi` (Apache-2.0 and MIT).

- A parameter, a function, a class, and a lambda parameter now shadow a binding of the same name in an outer scope or in the builtins.

- A member of a class resolves through every base class. A member of a subclass shadows a member of a base class with the same name.

- A literal behaves like an instance of its builtin class, so `"abc".join` and `[1].append` resolve. A parenthesized expression passes its inner expression through.

- A written annotation makes a name behave like an instance of the annotated class: typed parameters, annotated variables and attributes, return annotations, `Optional`, `Union`, `Foo | None`, and string annotations. The rules infer no type. A generic type follows the container only.

- The crate ships stub modules. `STUB_MODULES` lists them, and `language_configuration` adds them to the builtins graph. The first stub module is `typing`, derived from typeshed `stdlib/typing.pyi`. `List`, `Dict`, `Set`, `FrozenSet`, `Tuple`, `Type`, and `Text` are aliases of the builtin classes. `script/gen_stubs.py` generates the stubs.

- An annotation with a dotted container, like `typing.List[Foo]`, follows the container.

- A comprehension has a scope of its own. Its variables no longer share a symbol with a variable of the same name in the enclosing scope, and they no longer leak into that scope.

- The body of a top-level function sees the final bindings of the module with a higher precedence than the bindings before the function. A module-level definition after the function now shadows a builtin of the same name, and a rebinding after the function shadows the earlier binding.

- A parameter `*args: T` behaves like a tuple, and `**kwargs: T` behaves like a dict. The annotation `T` describes the elements, so the rules do not follow it.

- A `yield` expression no longer makes the build fail. Before, a statement like `x = yield value` stopped the build of the file.

- A decorated method gets the class scope, as a plain method does. `self` in a method with `@property` or a custom decorator reaches the members of the class. In a `@classmethod`, the first parameter behaves like the class. A `@staticmethod` has no `self`. The rules read the last decorator of the method only.

- The name in a keyword argument, as `mro` in `f(mro=1)`, is a reference. It resolves to the parameter of the called function. For a call of a class, it resolves to the parameter of `__init__`, also through a base class.

- The name in an attribute assignment target, as `x` in `obj.x = value`, is a reference. It resolves through the object as the name in a read of the attribute does. It also resolves to its own definition. The definition behaviour of `self.x = value` stays.

- `import a.b.c` binds the name `a` to the package `a`. A use of `a` alone, as in `a.foo` or `a.b.c.X`, resolves in the package and reaches the submodules `b` and `c` as members.

- The crate depends on `stack-graphs`, because `build_stub_module_into` takes a `StackGraph`.

- A call of a function with a return annotation follows the annotation only, and no longer the `return` statements of the body. This holds when the annotation names a class. The body stays visible to the caller when the annotation names no class: `Self`, `Any`, `Type[...]`, a `TypeVar`, `ParamSpec`, or type alias that the same file binds, a type parameter of a `def` or `class`, and a string annotation. Before this change, the value of the body and the annotation both reached the caller, and the caller found members that the annotation does not name.

- More `isinstance` guards narrow the type of a name:
  - `assert isinstance(x, A) and isinstance(x, B)`, with either operand, for the statements after the assert.
  - A condition in parentheses with `and`, in an `if` or an `elif`.
  - A conditional expression: the value before `if`.
  - `not isinstance(x, C) or ...`: the right operand, and the statements after an `if` of this condition when its body ends with `return`, `raise`, `continue`, or `break`.

- A string annotation that holds one plain name, such as `-> "Foo"` or `-> "mod.Foo"`, follows the annotation only, as a bare class name does. Before this change, the body stayed visible for every string. A string that holds more than a plain name, such as `"Foo | None"`, keeps the body.

- The annotation gate for a type alias accepts a name that ends in `TypeAlias`, such as `_TypeAlias`. A function that returns such an alias keeps its body visible.

- The reference at an attribute store, such as `o.x = x`, has the syntax type `store`. The path stitcher then keeps one more similar path for it. Before this change, the route of the return annotation could be lost when the field has the same name on both sides of the store. This restores the members that the previous change to return annotations had lost.

- The language builds graphs with contraction on. See `StackGraphLanguage::set_contract_graph` in `tree-sitter-stack-graphs`. The graphs are smaller, and the results do not change in the tests.

- The second stub module is `collections.abc`. `from collections.abc import Mapping`, `import collections.abc`, and `collections.abc.Mapping` in an annotation, a base class, or `isinstance` resolve to the class in the `typing` stub. `collections.abc.Set` is `typing.AbstractSet`. `StubModule::file_name()` turns each dot of a module name into a directory separator. Before this change these names had no path, so an annotation such as `-> Mapping[str, X]` lost its target. On mypy 1.19.1 this resolves 616 more reference positions and 6 listed rows. Precision falls by 0.01 point (95.60% to 95.59%). Rich and the standard library corpus keep their metrics. Four mypy references (`stubdoc.py`) become ambiguous, because `args: MutableSequence[str] = []` now also reaches the class of the annotation.

- `isinstance(x, NAMES)` narrows `x` when `NAMES` is a name bound to a tuple of classes, such as `SYMBOL_FUNCBASE_TYPES = (OverloadedFuncDef, FuncDef)`. The tuple can be in the same file or in an imported module. Each class of the tuple gives a candidate. Only an assignment to a single name has the new edge. A tuple in another position has none. On mypy 1.19.1 this resolves 13 references that had no path (`messages.py:2971:22` and `2973:22` among them) to the members of the classes, and makes 5 more ambiguous because the two classes inherit the member from different bases.

## v0.3.0 -- 2024-12-12

- The `tree-sitter-stack-graphs` dependency is updated to version 0.10.

- The `tree-sitter-python` dependency is updated to version 0.23.5.

## v0.2.0 -- 2024-07-09

### Added

- Added support for root paths. This fixes import problems when indexing using absolute directory paths.

### Fixed

- Fixed crash for lambdas with parameters.
- Fixed crash for nested functions definitions.

### Removed

- The `FILE_PATH_VAR` constant has been replaced in favor of `tree_sitter_stack_graphs::FILE_PATH_VAR`.

## v0.1.0 -- 2024-03-06

Initial release.
