# Changelog for tree-sitter-stack-graphs-python

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

- Identifiers in expressions no longer create a second reference node that has no edges.

- The rules define the loader's `<builtins>` file as the module `<builtins>`. Before this change, no reference reached the builtins file.

- `src/builtins.py` now holds a stub of the Python builtins: functions, classes with their methods, the exception hierarchy, and module constants such as `__name__`. The stub derives from typeshed `stdlib/builtins.pyi` (Apache-2.0 and MIT).

- A parameter, a function, a class, and a lambda parameter now shadow a binding of the same name in an outer scope or in the builtins.

- A member of a class resolves through every base class. A member of a subclass shadows a member of a base class with the same name.

- A literal behaves like an instance of its builtin class, so `"abc".join` and `[1].append` resolve. A parenthesized expression passes its inner expression through.

- A written annotation makes a name behave like an instance of the annotated class: typed parameters, annotated variables and attributes, return annotations, `Optional`, `Union`, `Foo | None`, and string annotations. The rules infer no type. A generic type follows the container only.

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
