# Changelog for tree-sitter-stack-graphs-python

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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
